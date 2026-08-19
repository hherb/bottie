//! Rust-owned durable conversation storage.

use std::{fs, path::PathBuf, time::Duration};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

mod lifecycle;
mod migrations;
mod types;

use migrations::{MIGRATION_1, MIGRATION_2};
pub(crate) use types::{
    ConversationLifecycle, ConversationSummary, MessageState, NewStoredMessage, StorageError,
    StoredConversation, StoredMessage, StoredRole,
};

const CURRENT_SCHEMA_VERSION: i64 = 2;
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
}

impl ConversationStore {
    /// Creates the application directory, applies migrations, and verifies integrity.
    pub(crate) fn initialize(path: PathBuf) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let store = Self { path };
        let mut connection = store.open()?;
        store.migrate(&mut connection)?;
        if store.integrity_check(&connection)? != "ok" {
            return Err(StorageError::internal());
        }
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
            params![branch_id, conversation_id, DEFAULT_BRANCH_NAME, now],
        )?;
        transaction.commit()?;
        Ok(StoredConversation {
            id: conversation_id,
            title,
            messages: Vec::new(),
        })
    }

    /// Appends one immutable message and its ordered content blocks transactionally.
    pub(crate) fn append_message(
        &self,
        message: NewStoredMessage,
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
                "SELECT branches.id FROM branches
                 JOIN conversations ON conversations.id = branches.conversation_id
                 WHERE branches.conversation_id = ?1 AND conversations.profile_id = ?2
                   AND conversations.deleted_at_ms IS NULL
                 ORDER BY branches.created_at_ms, branches.id LIMIT 1",
                params![message.conversation_id, DEFAULT_PROFILE_ID],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))?;
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
        let stored = StoredMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: message.role,
            text,
            reasoning,
            state: message.state,
            provider_id: message.provider_id,
            model_id: message.model_id,
            created_at_ms: now_ms()?,
        };
        transaction.execute(
            "INSERT INTO messages
             (id, conversation_id, branch_id, parent_message_id, role, state, provider_id, model_id,
              created_at_ms, sequence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
        transaction.execute(
            "UPDATE conversations SET updated_at_ms = ?1, archived_at_ms = NULL WHERE id = ?2",
            params![stored.created_at_ms, message.conversation_id],
        )?;
        transaction.commit()?;
        Ok(stored)
    }

    /// Reconstructs one non-deleted conversation and its current main branch.
    pub(crate) fn load_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<StoredConversation, StorageError> {
        let connection = self.open()?;
        let title: String = connection
            .query_row(
                "SELECT title FROM conversations WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
                params![conversation_id, DEFAULT_PROFILE_ID],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))?;
        let branch_id: String = connection.query_row(
            "SELECT id FROM branches WHERE conversation_id = ?1 ORDER BY created_at_ms, id LIMIT 1",
            [conversation_id],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT id, role, state, provider_id, model_id, created_at_ms
             FROM messages WHERE branch_id = ?1 ORDER BY sequence",
        )?;
        let rows = statement.query_map([branch_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        let mut messages = Vec::new();
        for row in rows {
            let (id, role, state, provider_id, model_id, created_at_ms) = row?;
            let (text, reasoning) = load_blocks(&connection, &id)?;
            messages.push(StoredMessage {
                id,
                role: StoredRole::from_database(&role)?,
                text,
                reasoning,
                state: MessageState::from_database(&state)?,
                provider_id,
                model_id,
                created_at_ms,
            });
        }
        Ok(StoredConversation {
            id: conversation_id.into(),
            title,
            messages,
        })
    }

    /// Opens one connection with Bottie's durability and integrity policy enabled.
    fn open(&self) -> Result<Connection, StorageError> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(connection)
    }

    /// Applies each pending migration exactly once and ensures the built-in profile exists.
    fn migrate(&self, connection: &mut Connection) -> Result<(), StorageError> {
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > CURRENT_SCHEMA_VERSION {
            return Err(StorageError::internal());
        }
        if version < 1 {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            transaction.execute_batch(MIGRATION_1)?;
            let now = now_ms()?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (1, 'storage foundation', ?1)",
                [now],
            )?;
            transaction.execute(
                "INSERT INTO profiles (id, name, created_at_ms) VALUES (?1, ?2, ?3)",
                params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME, now],
            )?;
            transaction.pragma_update(None, "user_version", 1)?;
            transaction.commit()?;
        }
        if version < 2 {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            transaction.execute_batch(MIGRATION_2)?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, name, applied_at_ms)
                 VALUES (2, 'branch-local message order', ?1)",
                [now_ms()?],
            )?;
            transaction.pragma_update(None, "user_version", 2)?;
            transaction.commit()?;
        }
        Ok(())
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
