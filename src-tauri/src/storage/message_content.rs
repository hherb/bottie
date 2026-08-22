//! Shared insertion and reconstruction of ordered durable message content blocks.

use rusqlite::{Connection, Transaction, params};

use super::{StorageError, StoredMessage};

/// Inserts non-empty text and reasoning as independently ordered content blocks.
pub(super) fn insert_blocks(
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
pub(super) fn load_blocks(
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
