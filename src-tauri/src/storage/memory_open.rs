//! Rust-owned bounded `open_memory` arguments, lineage reconstruction, and path-free results.

#![allow(dead_code)]

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredRole,
    memory_filters::MemorySourceKind,
};

/// Stable native tool name reserved for opening surrounding conversation-memory turns.
pub(crate) const OPEN_MEMORY_TOOL_NAME: &str = "open_memory";
/// Default number of retained turns included before and after the matched message.
const DEFAULT_OPEN_MEMORY_SURROUNDING_TURNS: usize = 2;
/// Maximum number of retained turns accepted on either side of the matched message.
pub(crate) const MAX_OPEN_MEMORY_SURROUNDING_TURNS: usize = 3;
/// Maximum Unicode-scalar length of one returned turn.
pub(crate) const MAX_OPEN_MEMORY_TURN_CHARACTERS: usize = 2_000;
/// Maximum Unicode-scalar length accepted for either opaque durable identity.
pub(crate) const MAX_OPEN_MEMORY_ID_CHARACTERS: usize = 128;

/// Typed provenance accepted by Bottie's future provider-independent `open_memory` executor.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OpenMemoryArguments {
    /// Durable conversation identity returned by `search_memory`.
    pub(crate) conversation_id: String,
    /// Exact durable message identity returned by `search_memory`.
    pub(crate) message_id: String,
    /// Optional number of retained turns requested before the match.
    pub(crate) before: Option<usize>,
    /// Optional number of retained turns requested after the match.
    pub(crate) after: Option<usize>,
}

/// Path-free retained-turn window returned by the native `open_memory` contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenMemoryResult {
    /// Exact source provenance used to resolve this window.
    pub(crate) provenance: OpenMemoryProvenance,
    /// Ordered final message answers from the matched message's own branch lineage.
    pub(crate) turns: Vec<OpenMemoryTurn>,
}

/// Durable path-free provenance for one opened conversation-memory source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenMemoryProvenance {
    /// Stable source category shared with `search_memory`.
    pub(crate) source_kind: &'static str,
    /// Conversation containing the matched message.
    pub(crate) conversation_id: String,
    /// Bounded durable conversation title for inspectable attribution.
    pub(crate) conversation_title: String,
    /// Exact matched durable message identity.
    pub(crate) message_id: String,
}

/// One bounded final message answer in an opened memory window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenMemoryTurn {
    /// Stable message identity for later citation and inspection.
    pub(crate) message_id: String,
    /// Author role of the retained turn.
    pub(crate) role: StoredRole,
    /// Bounded answer text; separate reasoning is deliberately omitted.
    pub(crate) text: String,
    /// Durable creation time in Unix milliseconds.
    pub(crate) created_at_ms: i64,
    /// Whether this is the exact message supplied by `search_memory` provenance.
    pub(crate) is_match: bool,
}

/// Internal complete retained turn before the output window and text caps are applied.
struct RetainedTurn {
    message_id: String,
    role: StoredRole,
    text: String,
    created_at_ms: i64,
}

/// Raw retained-turn metadata loaded before answer-only content reconstruction.
type RawTurn = (String, String, i64);

impl ConversationStore {
    /// Opens bounded final turns around exact conversation-message provenance without changing selection.
    pub(crate) fn execute_open_memory(
        &self,
        arguments: OpenMemoryArguments,
    ) -> Result<OpenMemoryResult, StorageError> {
        validate_identity("conversation", &arguments.conversation_id)?;
        validate_identity("message", &arguments.message_id)?;
        let before = arguments
            .before
            .unwrap_or(DEFAULT_OPEN_MEMORY_SURROUNDING_TURNS)
            .min(MAX_OPEN_MEMORY_SURROUNDING_TURNS);
        let after = arguments
            .after
            .unwrap_or(DEFAULT_OPEN_MEMORY_SURROUNDING_TURNS)
            .min(MAX_OPEN_MEMORY_SURROUNDING_TURNS);
        let connection = self.open()?;
        let target = connection
            .query_row(
                "SELECT conversations.title, messages.branch_id, messages.sequence,
                        messages.role, messages.created_at_ms
                 FROM messages
                 JOIN conversations ON conversations.id = messages.conversation_id
                 WHERE conversations.id = ?1 AND messages.id = ?2
                   AND conversations.profile_id = ?3
                   AND conversations.deleted_at_ms IS NULL
                   AND messages.state = 'final'
                   AND EXISTS (
                       SELECT 1 FROM message_blocks
                       WHERE message_blocks.message_id = messages.id
                         AND message_blocks.block_type = 'text'
                         AND length(message_blocks.text_content) > 0
                   )",
                params![
                    &arguments.conversation_id,
                    &arguments.message_id,
                    DEFAULT_PROFILE_ID
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(unavailable_memory)?;
        let turns = load_retained_window(
            &connection,
            &target.1,
            target.2,
            (&arguments.message_id, &target.3, target.4),
            before,
            after,
        )?;
        let turns = turns
            .iter()
            .map(|turn| OpenMemoryTurn {
                message_id: turn.message_id.clone(),
                role: turn.role,
                text: bounded_turn_text(&turn.text),
                created_at_ms: turn.created_at_ms,
                is_match: turn.message_id == arguments.message_id,
            })
            .collect();
        Ok(OpenMemoryResult {
            provenance: OpenMemoryProvenance {
                source_kind: MemorySourceKind::Message.as_str(),
                conversation_id: arguments.conversation_id,
                conversation_title: target.0,
                message_id: arguments.message_id,
            },
            turns,
        })
    }
}

/// Loads only the requested final text turns around the target's immutable owning-branch lineage.
fn load_retained_window(
    connection: &rusqlite::Connection,
    branch_id: &str,
    target_sequence: i64,
    target: (&str, &str, i64),
    before: usize,
    after: usize,
) -> Result<Vec<RetainedTurn>, StorageError> {
    let before_limit = i64::try_from(before).map_err(|_| StorageError::internal())?;
    let after_limit = i64::try_from(after).map_err(|_| StorageError::internal())?;
    let mut before_statement = connection.prepare(
        "WITH RECURSIVE ancestors(id, parent_message_id, depth) AS (
             SELECT id, parent_message_id, 0 FROM messages WHERE id = ?1
             UNION ALL
             SELECT messages.id, messages.parent_message_id, ancestors.depth + 1
             FROM messages JOIN ancestors ON messages.id = ancestors.parent_message_id
         )
         SELECT messages.id, messages.role, messages.created_at_ms
         FROM ancestors JOIN messages ON messages.id = ancestors.id
         WHERE ancestors.depth > 0 AND messages.state = 'final'
           AND EXISTS (
               SELECT 1 FROM message_blocks
               WHERE message_blocks.message_id = messages.id
                 AND message_blocks.block_type = 'text'
                 AND length(message_blocks.text_content) > 0
           )
         ORDER BY ancestors.depth ASC LIMIT ?2",
    )?;
    let mut before_rows = before_statement
        .query_map(params![target.0, before_limit], raw_turn_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    before_rows.reverse();

    let mut after_statement = connection.prepare(
        "SELECT messages.id, messages.role, messages.created_at_ms
         FROM messages
         WHERE messages.branch_id = ?1 AND messages.sequence > ?2
           AND messages.state = 'final'
           AND EXISTS (
               SELECT 1 FROM message_blocks
               WHERE message_blocks.message_id = messages.id
                 AND message_blocks.block_type = 'text'
                 AND length(message_blocks.text_content) > 0
           )
         ORDER BY messages.sequence ASC LIMIT ?3",
    )?;
    let after_rows = after_statement
        .query_map(
            params![branch_id, target_sequence, after_limit],
            raw_turn_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    before_rows.push((target.0.to_owned(), target.1.to_owned(), target.2));
    before_rows.extend(after_rows);
    load_retained_turns(connection, before_rows)
}

/// Decodes one trusted final-turn row without mixing storage policy into rusqlite callbacks.
fn raw_turn_from_row(row: &Row<'_>) -> rusqlite::Result<RawTurn> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

/// Reconstructs answer-only text for the already bounded set of retained turn identities.
fn load_retained_turns(
    connection: &rusqlite::Connection,
    rows: Vec<RawTurn>,
) -> Result<Vec<RetainedTurn>, StorageError> {
    rows.into_iter()
        .map(|(message_id, role, created_at_ms)| {
            let (text, _) = super::load_blocks(connection, &message_id)?;
            Ok(RetainedTurn {
                message_id,
                role: StoredRole::from_database(&role)?,
                text,
                created_at_ms,
            })
        })
        .collect()
}

/// Rejects missing or unbounded opaque provenance before any database lookup.
fn validate_identity(kind: &str, value: &str) -> Result<(), StorageError> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > MAX_OPEN_MEMORY_ID_CHARACTERS {
        return Err(StorageError::invalid(format!(
            "The open_memory {kind} identity is invalid."
        )));
    }
    Ok(())
}

/// Caps one turn without splitting a Unicode scalar and preserves a visible truncation marker.
fn bounded_turn_text(value: &str) -> String {
    if value.chars().count() <= MAX_OPEN_MEMORY_TURN_CHARACTERS {
        return value.to_owned();
    }
    value
        .chars()
        .take(MAX_OPEN_MEMORY_TURN_CHARACTERS - 1)
        .chain(std::iter::once('…'))
        .collect()
}

/// Hides whether unavailable provenance belonged to another profile, conversation, state, or lifecycle.
fn unavailable_memory() -> StorageError {
    StorageError::not_found("That retained conversation memory is unavailable.")
}
