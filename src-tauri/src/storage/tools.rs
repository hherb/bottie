//! Append-oriented tool invocation and result persistence under native provider runs.

#![allow(
    dead_code,
    reason = "native tool orchestration is intentionally deferred to a later bounded slice"
)]

use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::Value;

use super::{ConversationStore, StorageError, now_ms};

const MAX_TOOL_NAME_CHARACTERS: usize = 128;
const MAX_PROVIDER_CALL_ID_CHARACTERS: usize = 512;
const MAX_TOOL_JSON_BYTES: usize = 1_048_576;

/// One provider-emitted tool call accepted for durable append.
#[derive(Clone, Debug)]
pub(crate) struct NewToolInvocation {
    /// Native provider run that owns the call.
    pub(crate) provider_run_id: String,
    /// Provider-scoped call identity used to correlate one result.
    pub(crate) provider_call_id: String,
    /// Stable tool definition name requested by the provider.
    pub(crate) tool_name: String,
    /// Validated JSON object containing provider-supplied arguments.
    pub(crate) arguments: Value,
}

/// One tool outcome accepted for durable append.
#[derive(Clone, Debug)]
pub(crate) struct NewToolResult {
    /// Native provider run that owns the matching call.
    pub(crate) provider_run_id: String,
    /// Provider-scoped call identity previously retained for the run.
    pub(crate) provider_call_id: String,
    /// Structured result value retained without lossy string conversion.
    pub(crate) output: Value,
    /// Whether the output represents a tool failure rather than a normal result.
    pub(crate) is_error: bool,
}

/// One reconstructed tool call returned without native or provider call identities.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredToolInvocation {
    /// Append order within the owning provider run.
    pub(crate) ordinal: u32,
    /// Stable tool definition name requested by the provider.
    pub(crate) tool_name: String,
    /// Exact structured arguments supplied for the call.
    pub(crate) arguments: Value,
    /// Appended tool outcome, absent while the call remains unresolved.
    pub(crate) result: Option<StoredToolResult>,
    /// Native wall-clock time when the call was retained.
    pub(crate) created_at_ms: i64,
}

/// One reconstructed result linked to a retained tool call.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredToolResult {
    /// Exact structured output produced by the tool boundary.
    pub(crate) output: Value,
    /// Whether the output records a tool failure.
    pub(crate) is_error: bool,
    /// Native wall-clock time when the result was retained.
    pub(crate) created_at_ms: i64,
}

impl ConversationStore {
    /// Appends one validated tool call while its provider run remains active.
    pub(crate) fn checkpoint_tool_invocation(
        &self,
        invocation: NewToolInvocation,
    ) -> Result<(), StorageError> {
        let call_id = normalized_identity(
            &invocation.provider_call_id,
            MAX_PROVIDER_CALL_ID_CHARACTERS,
            "A tool call requires a bounded provider call identity.",
        )?;
        let tool_name = normalized_identity(
            &invocation.tool_name,
            MAX_TOOL_NAME_CHARACTERS,
            "A tool call requires a bounded tool name.",
        )?;
        if !invocation.arguments.is_object() {
            return Err(StorageError::invalid(
                "Tool-call arguments must be a JSON object.",
            ));
        }
        let arguments_json =
            bounded_json(&invocation.arguments, "Tool-call arguments are too large.")?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        require_active_run(&transaction, &invocation.provider_run_id)?;
        let duplicate: bool = transaction.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM tool_invocations
                 WHERE provider_run_id = ?1 AND provider_call_id = ?2
             )",
            params![invocation.provider_run_id, call_id],
            |row| row.get(0),
        )?;
        if duplicate {
            return Err(StorageError::invalid(
                "That provider tool call is already retained.",
            ));
        }
        let ordinal: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM tool_invocations
             WHERE provider_run_id = ?1",
            [&invocation.provider_run_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO tool_invocations
             (id, provider_run_id, ordinal, provider_call_id, tool_name, arguments_json, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                uuid::Uuid::new_v4().to_string(),
                invocation.provider_run_id,
                ordinal,
                call_id,
                tool_name,
                arguments_json,
                now_ms()?
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Appends the single validated outcome for a retained tool call while its run is active.
    pub(crate) fn checkpoint_tool_result(&self, result: NewToolResult) -> Result<(), StorageError> {
        let call_id = normalized_identity(
            &result.provider_call_id,
            MAX_PROVIDER_CALL_ID_CHARACTERS,
            "A tool result requires a bounded provider call identity.",
        )?;
        let output_json = bounded_json(&result.output, "Tool-result output is too large.")?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        require_active_run(&transaction, &result.provider_run_id)?;
        let invocation_id = transaction
            .query_row(
                "SELECT id FROM tool_invocations
                 WHERE provider_run_id = ?1 AND provider_call_id = ?2",
                params![result.provider_run_id, call_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::not_found("That tool invocation is not retained."))?;
        let has_result: bool = transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM tool_results WHERE tool_invocation_id = ?1)",
            [&invocation_id],
            |row| row.get(0),
        )?;
        if has_result {
            return Err(StorageError::invalid(
                "That tool invocation already has a retained result.",
            ));
        }
        transaction.execute(
            "INSERT INTO tool_results
             (id, tool_invocation_id, output_json, is_error, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                invocation_id,
                output_json,
                result.is_error,
                now_ms()?
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

/// Reconstructs ordered tool activity for one retained provider run.
pub(super) fn load_tool_invocations(
    connection: &rusqlite::Connection,
    run_id: &str,
) -> Result<Vec<StoredToolInvocation>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT tool_invocations.ordinal, tool_invocations.tool_name,
                tool_invocations.arguments_json, tool_invocations.created_at_ms,
                tool_results.output_json, tool_results.is_error, tool_results.created_at_ms
         FROM tool_invocations
         LEFT JOIN tool_results ON tool_results.tool_invocation_id = tool_invocations.id
         WHERE tool_invocations.provider_run_id = ?1
         ORDER BY tool_invocations.ordinal",
    )?;
    let rows = statement.query_map([run_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<bool>>(5)?,
            row.get::<_, Option<i64>>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (ordinal, tool_name, arguments, created_at_ms, output, is_error, result_at_ms) = row?;
        let result = match (output, is_error, result_at_ms) {
            (Some(output), Some(is_error), Some(created_at_ms)) => Some(StoredToolResult {
                output: serde_json::from_str(&output).map_err(|_| StorageError::internal())?,
                is_error,
                created_at_ms,
            }),
            (None, None, None) => None,
            _ => return Err(StorageError::internal()),
        };
        Ok(StoredToolInvocation {
            ordinal: u32::try_from(ordinal).map_err(|_| StorageError::internal())?,
            tool_name,
            arguments: serde_json::from_str(&arguments).map_err(|_| StorageError::internal())?,
            result,
            created_at_ms,
        })
    })
    .collect()
}

/// Requires a provider run that can still accept crash-safe checkpoints.
fn require_active_run(transaction: &Transaction<'_>, run_id: &str) -> Result<(), StorageError> {
    let active: bool = transaction.query_row(
        "SELECT EXISTS (SELECT 1 FROM provider_runs WHERE id = ?1 AND state = 'running')",
        [run_id],
        |row| row.get(0),
    )?;
    if active {
        Ok(())
    } else {
        Err(StorageError::not_found("That provider run is not active."))
    }
}

/// Trims and bounds one provider-controlled identifier.
fn normalized_identity<'a>(
    value: &'a str,
    maximum_characters: usize,
    error_message: &'static str,
) -> Result<&'a str, StorageError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum_characters {
        return Err(StorageError::invalid(error_message));
    }
    Ok(value)
}

/// Serializes one structured payload and enforces the native checkpoint ceiling.
fn bounded_json(value: &Value, error_message: &'static str) -> Result<String, StorageError> {
    let json = serde_json::to_string(value).map_err(|_| StorageError::invalid(error_message))?;
    if json.len() > MAX_TOOL_JSON_BYTES {
        return Err(StorageError::invalid(error_message));
    }
    Ok(json)
}
