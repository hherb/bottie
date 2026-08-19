//! Provider-run provenance and append-only usage persistence.

use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, NewProviderRun, ProviderRunState, StorageError,
    StoredProviderRun, StoredReasoningEffort, StoredUsage, now_ms,
};

impl ConversationStore {
    /// Records one accepted native provider run before network work begins.
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
                   AND conversations.deleted_at_ms IS NULL",
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
                now_ms()?
            ],
        )?;
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
        if let Some(usage) = usage {
            append_usage(&transaction, run_id, completed_at_ms, usage)?;
        }
        transaction.commit()?;
        Ok(())
    }
}

/// Loads provider provenance and the latest cumulative usage for one response.
pub(super) fn load_provider_run(
    transaction: &rusqlite::Connection,
    run_id: &str,
) -> Result<StoredProviderRun, StorageError> {
    let values = transaction.query_row(
        "SELECT provider_runs.state, provider_runs.reasoning_effort,
                provider_runs.started_at_ms, provider_runs.completed_at_ms,
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
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<f64>>(6)?,
            ))
        },
    )?;
    let usage = usage_from_database(values.4, values.5, values.6)?;
    Ok(StoredProviderRun {
        id: run_id.into(),
        state: ProviderRunState::from_database(&values.0)?,
        reasoning_effort: StoredReasoningEffort::from_database(&values.1)?,
        started_at_ms: values.2,
        completed_at_ms: values.3,
        usage,
    })
}

/// Ensures an assistant response links only to its matching native provider run.
pub(super) fn validate_provider_run_link(
    transaction: &Transaction<'_>,
    run_id: &str,
    conversation_id: &str,
    branch_id: &str,
    provider_id: Option<&str>,
    model_id: Option<&str>,
) -> Result<(), StorageError> {
    let Some((provider_id, model_id)) = provider_id.zip(model_id) else {
        return Err(StorageError::invalid(
            "A provider-linked response requires its provider and model.",
        ));
    };
    let exists = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM provider_runs
             WHERE id = ?1 AND conversation_id = ?2 AND branch_id = ?3
               AND provider_id = ?4 AND model_id = ?5
         )",
        params![run_id, conversation_id, branch_id, provider_id, model_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(StorageError::invalid(
            "That provider run does not match the assistant response.",
        ));
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

/// Appends one cumulative usage snapshot in provider-run order.
fn append_usage(
    transaction: &Transaction<'_>,
    run_id: &str,
    recorded_at_ms: i64,
    usage: StoredUsage,
) -> Result<(), StorageError> {
    let ordinal: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM usage_records WHERE provider_run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
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
