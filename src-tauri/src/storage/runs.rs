//! Provider-run provenance, response checkpoints, usage, and interruption recovery.

use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, MessageState, NewProviderRun, ProviderRunState,
    RunBlockKind, StorageError, StoredProviderRun, StoredReasoningEffort, StoredUsage, now_ms,
};

const INTERRUPTED_ERROR_CODE: &str = "interrupted";

impl ConversationStore {
    /// Records one accepted native provider run and its empty response checkpoint atomically.
    pub(crate) fn start_provider_run(&self, run: NewProviderRun) -> Result<(), StorageError> {
        validate_new_run(&run)?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let branch_id: String = transaction
            .query_row(
                "SELECT messages.branch_id FROM messages
                 JOIN conversations ON conversations.id = messages.conversation_id
                 WHERE messages.id = ?1 AND messages.conversation_id = ?2
                   AND messages.role = 'user' AND conversations.profile_id = ?3
                   AND conversations.deleted_at_ms IS NULL
                   AND conversations.current_branch_id = messages.branch_id",
                params![
                    run.request_message_id,
                    run.conversation_id,
                    DEFAULT_PROFILE_ID
                ],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::not_found(
                    "The request message for that provider run no longer exists.",
                )
            })?;
        let latest_message_id: String = transaction.query_row(
            "SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [&branch_id],
            |row| row.get(0),
        )?;
        if latest_message_id != run.request_message_id {
            return Err(StorageError::invalid(
                "A provider run must start from the latest user request.",
            ));
        }
        let started_at_ms = now_ms()?;
        transaction.execute(
            "INSERT INTO provider_runs
             (id, conversation_id, branch_id, request_message_id, provider_id, model_id, state,
              reasoning_effort, temperature, max_output_tokens, started_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'running', ?7, ?8, ?9, ?10)",
            params![
                run.id,
                run.conversation_id,
                branch_id,
                run.request_message_id,
                run.provider_id,
                run.model_id,
                run.reasoning_effort.as_str(),
                run.temperature,
                run.max_output_tokens,
                started_at_ms
            ],
        )?;
        transaction.execute(
            "INSERT INTO messages
             (id, conversation_id, branch_id, parent_message_id, role, state, provider_id, model_id,
              created_at_ms, sequence, provider_run_id)
             VALUES (?1, ?2, ?3, ?4, 'assistant', 'partial', ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                run.conversation_id,
                branch_id,
                run.request_message_id,
                run.provider_id,
                run.model_id,
                started_at_ms,
                next_message_sequence(&transaction, &branch_id)?,
                run.id
            ],
        )?;
        transaction.execute(
            "UPDATE conversations SET updated_at_ms = ?1, archived_at_ms = NULL WHERE id = ?2",
            params![started_at_ms, run.conversation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Appends one provider delta to the native-owned partial response before IPC delivery.
    pub(crate) fn checkpoint_provider_delta(
        &self,
        run_id: &str,
        kind: RunBlockKind,
        delta: &str,
    ) -> Result<(), StorageError> {
        if delta.is_empty() {
            return Ok(());
        }
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let message_id = active_response_message_id(&transaction, run_id)?;
        let ordinal: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM message_blocks WHERE message_id = ?1",
            [&message_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO message_blocks (id, message_id, ordinal, block_type, text_content)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                message_id,
                ordinal,
                kind.as_str(),
                delta
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Appends one provider-reported cumulative usage checkpoint while a run is active.
    pub(crate) fn checkpoint_provider_usage(
        &self,
        run_id: &str,
        usage: StoredUsage,
    ) -> Result<(), StorageError> {
        if !usage.is_valid() {
            return Err(StorageError::invalid("Provider usage values are invalid."));
        }
        if !usage.has_value() {
            return Ok(());
        }
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        active_response_message_id(&transaction, run_id)?;
        append_usage_if_changed(&transaction, run_id, now_ms()?, usage)?;
        transaction.commit()?;
        Ok(())
    }

    /// Closes one running provider record and appends its final usage snapshot.
    pub(crate) fn finish_provider_run(
        &self,
        run_id: &str,
        state: ProviderRunState,
        error_code: Option<&str>,
        usage: Option<StoredUsage>,
    ) -> Result<(), StorageError> {
        let error_code = validate_run_completion(state, error_code, usage.as_ref())?;
        let usage = usage.filter(StoredUsage::has_value);
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let completed_at_ms = now_ms()?;
        let updated = transaction.execute(
            "UPDATE provider_runs SET state = ?1, error_code = ?2, completed_at_ms = ?3
             WHERE id = ?4 AND state = 'running'",
            params![state.as_str(), error_code, completed_at_ms, run_id],
        )?;
        if updated != 1 {
            return Err(StorageError::not_found("That provider run is not active."));
        }
        let message_state = match state {
            ProviderRunState::Completed => MessageState::Final,
            ProviderRunState::Cancelled => MessageState::Cancelled,
            ProviderRunState::Failed => MessageState::Failed,
            ProviderRunState::Running => unreachable!("validated terminal state"),
        };
        transaction.execute(
            "UPDATE messages SET state = ?1 WHERE provider_run_id = ?2 AND state = 'partial'",
            params![message_state.as_str(), run_id],
        )?;
        if let Some(usage) = usage {
            append_usage_if_changed(&transaction, run_id, completed_at_ms, usage)?;
        }
        transaction.execute(
            "UPDATE conversations SET updated_at_ms = ?1
             WHERE id = (SELECT conversation_id FROM provider_runs WHERE id = ?2)",
            params![completed_at_ms, run_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Converts provider work left running by an earlier process into retained partial responses.
    pub(crate) fn recover_interrupted_runs(&self) -> Result<usize, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        create_missing_partial_responses(&transaction)?;
        let completed_at_ms = now_ms()?;
        let recovered = transaction.execute(
            "UPDATE provider_runs
             SET state = 'failed', error_code = ?1, completed_at_ms = ?2
             WHERE state = 'running'",
            params![INTERRUPTED_ERROR_CODE, completed_at_ms],
        )?;
        transaction.commit()?;
        Ok(recovered)
    }
}

/// Loads provider provenance and the latest cumulative usage for one response.
pub(super) fn load_provider_run(
    transaction: &rusqlite::Connection,
    run_id: &str,
) -> Result<StoredProviderRun, StorageError> {
    let values = transaction.query_row(
        "SELECT provider_runs.state, provider_runs.reasoning_effort,
                provider_runs.started_at_ms, provider_runs.completed_at_ms, provider_runs.error_code,
                usage_records.input_tokens, usage_records.output_tokens, usage_records.cost_usd
         FROM provider_runs
         LEFT JOIN usage_records ON usage_records.provider_run_id = provider_runs.id
           AND usage_records.ordinal = (
               SELECT MAX(latest.ordinal) FROM usage_records AS latest
               WHERE latest.provider_run_id = provider_runs.id
           )
         WHERE provider_runs.id = ?1",
        [run_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<f64>>(7)?,
            ))
        },
    )?;
    let usage = usage_from_database(values.5, values.6, values.7)?;
    Ok(StoredProviderRun {
        id: run_id.into(),
        state: ProviderRunState::from_database(&values.0)?,
        reasoning_effort: StoredReasoningEffort::from_database(&values.1)?,
        started_at_ms: values.2,
        completed_at_ms: values.3,
        error_code: values.4,
        usage,
        tool_invocations: super::tools::load_tool_invocations(transaction, run_id)?,
    })
}

/// Returns the partial assistant message associated with one active run.
fn active_response_message_id(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<String, StorageError> {
    transaction
        .query_row(
            "SELECT messages.id FROM messages
             JOIN provider_runs ON provider_runs.id = messages.provider_run_id
             WHERE provider_runs.id = ?1 AND provider_runs.state = 'running'
               AND messages.role = 'assistant' AND messages.state = 'partial'",
            [run_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::not_found("That provider run is not active."))
}

/// Returns the next append sequence for one branch inside the caller's transaction.
fn next_message_sequence(
    transaction: &Transaction<'_>,
    branch_id: &str,
) -> Result<i64, StorageError> {
    transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence), -1) + 1 FROM messages WHERE branch_id = ?1",
            [branch_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Creates empty checkpoints for running version-three records produced before native checkpointing.
fn create_missing_partial_responses(transaction: &Transaction<'_>) -> Result<(), StorageError> {
    let mut statement = transaction.prepare(
        "SELECT provider_runs.id, provider_runs.conversation_id, provider_runs.branch_id,
                provider_runs.request_message_id, provider_runs.provider_id, provider_runs.model_id,
                provider_runs.started_at_ms
         FROM provider_runs
         WHERE provider_runs.state = 'running'
           AND NOT EXISTS (SELECT 1 FROM messages WHERE messages.provider_run_id = provider_runs.id)
         ORDER BY provider_runs.started_at_ms, provider_runs.id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;
    let runs = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (run_id, conversation_id, branch_id, request_id, provider_id, model_id, started_at_ms) in
        runs
    {
        transaction.execute(
            "INSERT INTO messages
             (id, conversation_id, branch_id, parent_message_id, role, state, provider_id, model_id,
              created_at_ms, sequence, provider_run_id)
             VALUES (?1, ?2, ?3, ?4, 'assistant', 'partial', ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                conversation_id,
                branch_id,
                request_id,
                provider_id,
                model_id,
                started_at_ms,
                next_message_sequence(transaction, &branch_id)?,
                run_id
            ],
        )?;
    }
    Ok(())
}

/// Validates provider-run identifiers and settings before insertion.
fn validate_new_run(run: &NewProviderRun) -> Result<(), StorageError> {
    if run.id.trim().is_empty()
        || run.provider_id.trim().is_empty()
        || run.model_id.trim().is_empty()
    {
        return Err(StorageError::invalid(
            "A provider run requires run, provider, and model identities.",
        ));
    }
    if run.temperature.is_some_and(|value| !value.is_finite()) || run.max_output_tokens == Some(0) {
        return Err(StorageError::invalid(
            "Provider-run generation settings are invalid.",
        ));
    }
    Ok(())
}

/// Validates a terminal transition and returns its normalized failure code.
fn validate_run_completion<'a>(
    state: ProviderRunState,
    error_code: Option<&'a str>,
    usage: Option<&StoredUsage>,
) -> Result<Option<&'a str>, StorageError> {
    if !state.is_terminal() {
        return Err(StorageError::invalid(
            "A provider run requires a terminal state.",
        ));
    }
    if usage.is_some_and(|value| !value.is_valid()) {
        return Err(StorageError::invalid("Provider usage values are invalid."));
    }
    let error_code = error_code.map(str::trim).filter(|value| !value.is_empty());
    if (state == ProviderRunState::Failed) != error_code.is_some() {
        return Err(StorageError::invalid(
            "Only failed provider runs require an error code.",
        ));
    }
    Ok(error_code)
}

/// Appends one cumulative usage snapshot only when it differs from the latest retained total.
fn append_usage_if_changed(
    transaction: &Transaction<'_>,
    run_id: &str,
    recorded_at_ms: i64,
    usage: StoredUsage,
) -> Result<(), StorageError> {
    let latest = transaction
        .query_row(
            "SELECT input_tokens, output_tokens, cost_usd FROM usage_records
             WHERE provider_run_id = ?1 ORDER BY ordinal DESC LIMIT 1",
            [run_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                ))
            },
        )
        .optional()?;
    let input_tokens = usage
        .input_tokens
        .map(i64::try_from)
        .transpose()
        .map_err(|_| StorageError::invalid("Provider usage exceeds the supported token range."))?;
    let output_tokens = usage
        .output_tokens
        .map(i64::try_from)
        .transpose()
        .map_err(|_| StorageError::invalid("Provider usage exceeds the supported token range."))?;
    if latest == Some((input_tokens, output_tokens, usage.cost_usd)) {
        return Ok(());
    }
    let ordinal: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM usage_records WHERE provider_run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT INTO usage_records
         (id, provider_run_id, ordinal, input_tokens, output_tokens, cost_usd, recorded_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            uuid::Uuid::new_v4().to_string(),
            run_id,
            ordinal,
            input_tokens,
            output_tokens,
            usage.cost_usd,
            recorded_at_ms
        ],
    )?;
    Ok(())
}

/// Reconstructs optional usage while checking SQLite integer conversions.
fn usage_from_database(
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cost_usd: Option<f64>,
) -> Result<Option<StoredUsage>, StorageError> {
    if input_tokens.is_none() && output_tokens.is_none() && cost_usd.is_none() {
        return Ok(None);
    }
    Ok(Some(StoredUsage {
        input_tokens: input_tokens
            .map(u64::try_from)
            .transpose()
            .map_err(|_| StorageError::internal())?,
        output_tokens: output_tokens
            .map(u64::try_from)
            .transpose()
            .map_err(|_| StorageError::internal())?,
        cost_usd,
    }))
}
