//! Rust-owned durable conversation storage.

use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

mod attachment_policy;
pub(crate) mod attachments;
mod backup;
mod branching;
mod error;
mod export;
pub(crate) mod extraction;
mod lifecycle;
mod migrate;
mod migrations;
mod ratings;
mod recovery;
mod runs;
mod search;
mod selection;
mod tools;
mod types;

pub(crate) use attachments::{IngestedAttachment, MAX_ATTACHMENT_SELECTION_COUNT};
pub(crate) use error::StorageError;
pub(crate) use export::ConversationFileExport;
pub(crate) use extraction::{
    AttachmentExtractionFormat, AttachmentExtractionState, StoredAttachmentExtraction,
};
#[cfg(test)]
use migrations::{MIGRATION_1, MIGRATION_2, MIGRATION_3, MIGRATION_4};
pub(crate) use recovery::StorageRecoveryStatus;
pub(crate) use types::{
    ConversationBranch, ConversationLifecycle, ConversationSearchResult, ConversationSummary,
    ForkedConversation, MessageState, NewProviderRun, NewStoredMessage, ProviderRunContext,
    ProviderRunState, ResponseRating, RunBlockKind, StoredAttachment, StoredConversation,
    StoredMessage, StoredProviderRun, StoredReasoningEffort, StoredRole, StoredUsage,
};

const CURRENT_SCHEMA_VERSION: i64 = 11;
const DEFAULT_PROFILE_ID: &str = "local";
const DEFAULT_PROFILE_NAME: &str = "Local profile";
const DEFAULT_BRANCH_NAME: &str = "Main";
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONVERSATION_TITLE_CHARACTERS: usize = 80;

/// Diagnostic storage policy status used by tests and future recovery UI.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct StorageStatus {
    schema_version: i64,
    profile_name: String,
    integrity_check: String,
    foreign_keys_enabled: bool,
    journal_mode: String,
}

/// Path-backed SQLite store opened through short-lived configured connections.
#[derive(Clone)]
pub(crate) struct ConversationStore {
    path: PathBuf,
    recovery_required: Arc<AtomicBool>,
}

impl ConversationStore {
    /// Creates the application directory, applies migrations, and verifies integrity.
    pub(crate) fn initialize(path: PathBuf) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let store = Self {
            path,
            recovery_required: Arc::new(AtomicBool::new(false)),
        };
        let mut connection = store.open()?;
        store.migrate(&mut connection)?;
        if store.integrity_check(&connection)? != "ok" {
            return Err(StorageError::internal());
        }
        drop(connection);
        store.process_pending_attachment_extractions()?;
        store.recover_interrupted_runs()?;
        Ok(store)
    }

    /// Creates a conversation and its main branch in one immediate transaction.
    pub(crate) fn create_conversation(
        &self,
        title: &str,
    ) -> Result<StoredConversation, StorageError> {
        let title = normalized_title(title)?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let branch_id = uuid::Uuid::new_v4().to_string();
        let now = now_ms()?;
        transaction.execute(
            "INSERT INTO conversations (id, profile_id, title, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![conversation_id, DEFAULT_PROFILE_ID, title, now],
        )?;
        transaction.execute(
            "INSERT INTO branches (id, conversation_id, name, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
            params![&branch_id, &conversation_id, DEFAULT_BRANCH_NAME, now],
        )?;
        transaction.execute(
            "UPDATE conversations SET current_branch_id = ?1 WHERE id = ?2",
            params![&branch_id, &conversation_id],
        )?;
        transaction.execute(
            "UPDATE profiles SET last_open_conversation_id = ?1 WHERE id = ?2",
            params![conversation_id, DEFAULT_PROFILE_ID],
        )?;
        transaction.commit()?;
        Ok(StoredConversation {
            id: conversation_id,
            title,
            current_branch_id: branch_id.clone(),
            branches: vec![ConversationBranch {
                id: branch_id,
                name: DEFAULT_BRANCH_NAME.into(),
            }],
            messages: Vec::new(),
        })
    }

    /// Appends one message and its ordered retained attachment associations atomically.
    pub(crate) fn append_message_with_attachments(
        &self,
        message: NewStoredMessage,
        attachment_ids: &[String],
    ) -> Result<StoredMessage, StorageError> {
        let text = message.text.trim().to_owned();
        let reasoning = message
            .reasoning
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if text.is_empty() && reasoning.is_none() {
            return Err(StorageError::invalid(
                "A stored message must contain text or reasoning.",
            ));
        }
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let branch_id: String = transaction
            .query_row(
                "SELECT conversations.current_branch_id FROM conversations
                 WHERE conversations.id = ?1 AND conversations.profile_id = ?2
                   AND conversations.deleted_at_ms IS NULL",
                params![message.conversation_id, DEFAULT_PROFILE_ID],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))?;
        if message.role == StoredRole::User {
            let has_active_run: bool = transaction.query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
                 )",
                [&message.conversation_id],
                |row| row.get(0),
            )?;
            if has_active_run {
                return Err(StorageError::invalid(
                    "Wait for the active response to finish before sending another message.",
                ));
            }
        }
        let parent_message_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [&branch_id],
                |row| row.get(0),
            )
            .optional()?;
        let sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), -1) + 1 FROM messages WHERE branch_id = ?1",
            [&branch_id],
            |row| row.get(0),
        )?;
        let mut stored = StoredMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: message.role,
            text,
            reasoning,
            state: message.state,
            provider_id: message.provider_id,
            model_id: message.model_id,
            provider_run: None,
            rating: None,
            attachments: Vec::new(),
            created_at_ms: now_ms()?,
        };
        transaction.execute(
            "INSERT INTO messages
             (id, conversation_id, branch_id, parent_message_id, role, state, provider_id, model_id,
              created_at_ms, sequence, provider_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
            params![
                stored.id,
                message.conversation_id,
                branch_id,
                parent_message_id,
                stored.role.as_str(),
                stored.state.as_str(),
                stored.provider_id,
                stored.model_id,
                stored.created_at_ms,
                sequence
            ],
        )?;
        insert_blocks(&transaction, &stored)?;
        stored.attachments = attachments::associate_message_attachments(
            &transaction,
            &stored.id,
            stored.role,
            attachment_ids,
        )?;
        transaction.execute(
            "UPDATE conversations SET updated_at_ms = ?1, archived_at_ms = NULL WHERE id = ?2",
            params![stored.created_at_ms, message.conversation_id],
        )?;
        transaction.commit()?;
        Ok(stored)
    }

    /// Reconstructs one non-deleted conversation and its current main branch.
    #[cfg(test)]
    pub(crate) fn load_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<StoredConversation, StorageError> {
        let connection = self.open()?;
        load_conversation_from_connection(&connection, conversation_id)
    }

    /// Opens one connection with Bottie's durability and integrity policy enabled.
    fn open(&self) -> Result<Connection, StorageError> {
        if self.recovery_required.load(Ordering::Acquire) {
            return Err(StorageError::recovery_required());
        }
        self.open_unchecked()
    }

    /// Opens one configured connection while startup recovery is inspecting or replacing the store.
    fn open_unchecked(&self) -> Result<Connection, StorageError> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(connection)
    }

    /// Returns current migration and SQLite policy state.
    #[cfg(test)]
    fn status(&self) -> Result<StorageStatus, StorageError> {
        let connection = self.open()?;
        Ok(StorageStatus {
            schema_version: connection
                .pragma_query_value(None, "user_version", |row| row.get(0))?,
            profile_name: connection.query_row(
                "SELECT name FROM profiles WHERE id = ?1",
                [DEFAULT_PROFILE_ID],
                |row| row.get(0),
            )?,
            integrity_check: self.integrity_check(&connection)?,
            foreign_keys_enabled: connection
                .pragma_query_value(None, "foreign_keys", |row| row.get(0))?,
            journal_mode: connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?,
        })
    }

    /// Runs SQLite's bounded quick integrity check.
    fn integrity_check(&self, connection: &Connection) -> Result<String, StorageError> {
        connection
            .pragma_query_value(None, "quick_check", |row| row.get(0))
            .map_err(Into::into)
    }
}

/// Reconstructs one non-deleted conversation through an existing connection or transaction.
fn load_conversation_from_connection(
    connection: &Connection,
    conversation_id: &str,
) -> Result<StoredConversation, StorageError> {
    let (title, branch_id): (String, String) = connection
        .query_row(
            "SELECT title, current_branch_id FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))?;
    let mut statement = connection.prepare(
        "WITH RECURSIVE lineage(id, parent_message_id, depth) AS (
             SELECT id, parent_message_id, 0 FROM messages
             WHERE id = (
                 SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1
             )
             UNION ALL
             SELECT messages.id, messages.parent_message_id, lineage.depth + 1
             FROM messages JOIN lineage ON messages.id = lineage.parent_message_id
         )
         SELECT messages.id, messages.role, messages.state, messages.provider_id, messages.model_id,
                messages.provider_run_id, response_ratings.rating, messages.created_at_ms
         FROM lineage JOIN messages ON messages.id = lineage.id
         LEFT JOIN response_ratings ON response_ratings.message_id = messages.id
         ORDER BY lineage.depth DESC",
    )?;
    let rows = statement.query_map([&branch_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, i64>(7)?,
        ))
    })?;
    let mut messages = Vec::new();
    for row in rows {
        let (id, role, state, provider_id, model_id, provider_run_id, rating, created_at_ms) = row?;
        let (text, reasoning) = load_blocks(connection, &id)?;
        let provider_run = provider_run_id
            .as_deref()
            .map(|run_id| runs::load_provider_run(connection, run_id))
            .transpose()?;
        let attachments = attachments::load_message_attachments(connection, &id)?;
        messages.push(StoredMessage {
            id,
            role: StoredRole::from_database(&role)?,
            text,
            reasoning,
            state: MessageState::from_database(&state)?,
            provider_id,
            model_id,
            provider_run,
            rating: rating
                .as_deref()
                .map(ResponseRating::from_database)
                .transpose()?,
            attachments,
            created_at_ms,
        });
    }
    let mut branch_statement = connection.prepare(
        "SELECT id, name FROM branches WHERE conversation_id = ?1 ORDER BY created_at_ms, id",
    )?;
    let branches = branch_statement
        .query_map([conversation_id], |row| {
            Ok(ConversationBranch {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StoredConversation {
        id: conversation_id.into(),
        title,
        current_branch_id: branch_id,
        branches,
        messages,
    })
}

/// Inserts non-empty text and reasoning as independently ordered content blocks.
fn insert_blocks(
    transaction: &Transaction<'_>,
    message: &StoredMessage,
) -> Result<(), StorageError> {
    let mut ordinal = 0_i64;
    for (block_type, content) in [
        ("text", Some(&message.text)),
        ("reasoning", message.reasoning.as_ref()),
    ] {
        if let Some(content) = content.filter(|content| !content.is_empty()) {
            transaction.execute(
                "INSERT INTO message_blocks (id, message_id, ordinal, block_type, text_content)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    message.id,
                    ordinal,
                    block_type,
                    content
                ],
            )?;
            ordinal += 1;
        }
    }
    Ok(())
}

/// Reconstructs text and optional reasoning from ordered content blocks.
fn load_blocks(
    connection: &Connection,
    message_id: &str,
) -> Result<(String, Option<String>), StorageError> {
    let mut statement = connection.prepare(
        "SELECT block_type, text_content FROM message_blocks WHERE message_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map([message_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut text = String::new();
    let mut reasoning = None;
    for row in rows {
        let (block_type, content) = row?;
        match block_type.as_str() {
            "text" => text.push_str(&content),
            "reasoning" => reasoning.get_or_insert_with(String::new).push_str(&content),
            _ => return Err(StorageError::internal()),
        }
    }
    Ok((text, reasoning))
}

/// Normalizes and bounds a conversation title without cutting inside a Unicode scalar value.
fn normalized_title(title: &str) -> Result<String, StorageError> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return Err(StorageError::invalid(
            "A conversation title cannot be empty.",
        ));
    }
    Ok(title
        .chars()
        .take(MAX_CONVERSATION_TITLE_CHARACTERS)
        .collect())
}

/// Returns the current Unix epoch timestamp in milliseconds.
fn now_ms() -> Result<i64, StorageError> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StorageError::internal())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| StorageError::internal())
}

#[cfg(test)]
#[path = "storage/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "storage/run_tests.rs"]
mod run_tests;

#[cfg(test)]
#[path = "storage/selection_tests.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "storage/branch_tests.rs"]
mod branch_tests;

#[cfg(test)]
#[path = "storage/search_tests.rs"]
mod search_tests;

#[cfg(test)]
#[path = "storage/rating_tests.rs"]
mod rating_tests;

#[cfg(test)]
#[path = "storage/export_tests.rs"]
mod export_tests;

#[cfg(test)]
#[path = "storage/tool_tests.rs"]
mod tool_tests;

#[cfg(test)]
#[path = "storage/attachment_tests.rs"]
mod attachment_tests;
#[cfg(test)]
#[path = "storage/backup_tests.rs"]
mod backup_tests;
#[cfg(test)]
#[path = "storage/extraction_tests.rs"]
mod extraction_tests;
#[cfg(test)]
#[path = "storage/recovery_tests.rs"]
mod recovery_tests;
