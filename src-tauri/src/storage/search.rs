//! Bounded native conversation search before the later FTS-backed memory milestone.

use std::collections::HashMap;

use rusqlite::params;

use super::{
    ConversationLifecycle, ConversationSearchResult, ConversationStore, DEFAULT_PROFILE_ID,
    StorageError,
};

const MAX_SEARCH_QUERY_CHARACTERS: usize = 200;
const MAX_SEARCH_RESULTS: usize = 50;
const SEARCH_SNIPPET_CONTEXT_CHARACTERS: usize = 44;
const SEARCH_SNIPPET_MAX_CHARACTERS: usize = 120;

/// One non-deleted conversation considered in native recency order.
struct SearchableConversation {
    id: String,
    title: String,
    updated_at_ms: i64,
    lifecycle: ConversationLifecycle,
    current_branch_id: String,
}

/// First matching visible-text block selected for one conversation.
struct MessageMatch {
    message_id: String,
    branch_id: String,
    snippet: String,
}

impl ConversationStore {
    /// Searches titles and user-visible message text across active and archived conversations.
    pub(crate) fn search_conversations(
        &self,
        query: &str,
    ) -> Result<Vec<ConversationSearchResult>, StorageError> {
        let query = normalized_search_text(query);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if query.chars().count() > MAX_SEARCH_QUERY_CHARACTERS {
            return Err(StorageError::invalid(format!(
                "Conversation search is limited to {MAX_SEARCH_QUERY_CHARACTERS} characters."
            )));
        }
        let folded_query = query.to_lowercase();
        let connection = self.open()?;
        let conversations = load_searchable_conversations(&connection)?;
        let message_matches = load_message_matches(&connection, &folded_query)?;
        let mut results = Vec::new();

        for conversation in conversations {
            let title_matches = conversation.title.to_lowercase().contains(&folded_query);
            let message_match = message_matches.get(&conversation.id);
            if !title_matches && message_match.is_none() {
                continue;
            }
            let (snippet, branch_id) = if title_matches {
                (conversation.title.clone(), conversation.current_branch_id)
            } else {
                let matched = message_match.expect("a non-title result must have message content");
                let branch_id = if message_is_visible_on_branch(
                    &connection,
                    &conversation.current_branch_id,
                    &matched.message_id,
                )? {
                    conversation.current_branch_id
                } else {
                    matched.branch_id.clone()
                };
                (matched.snippet.clone(), branch_id)
            };
            results.push(ConversationSearchResult {
                conversation_id: conversation.id,
                title: conversation.title,
                snippet,
                branch_id,
                updated_at_ms: conversation.updated_at_ms,
                lifecycle: conversation.lifecycle,
            });
            if results.len() == MAX_SEARCH_RESULTS {
                break;
            }
        }
        Ok(results)
    }
}

/// Loads active and archived conversation metadata in the stable navigation order.
fn load_searchable_conversations(
    connection: &rusqlite::Connection,
) -> Result<Vec<SearchableConversation>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, title, updated_at_ms,
                CASE WHEN archived_at_ms IS NOT NULL THEN 'archived' ELSE 'active' END,
                current_branch_id
         FROM conversations
         WHERE profile_id = ?1 AND deleted_at_ms IS NULL
         ORDER BY updated_at_ms DESC, id DESC",
    )?;
    let rows = statement.query_map([DEFAULT_PROFILE_ID], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, title, updated_at_ms, lifecycle, current_branch_id) = row?;
        Ok(SearchableConversation {
            id,
            title,
            updated_at_ms,
            lifecycle: ConversationLifecycle::from_database(&lifecycle)?,
            current_branch_id,
        })
    })
    .collect()
}

/// Selects the newest matching visible-text block for each searchable conversation.
fn load_message_matches(
    connection: &rusqlite::Connection,
    folded_query: &str,
) -> Result<HashMap<String, MessageMatch>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT conversations.id, messages.id, messages.branch_id, message_blocks.text_content
         FROM conversations
         JOIN messages ON messages.conversation_id = conversations.id
         JOIN message_blocks ON message_blocks.message_id = messages.id
         WHERE conversations.profile_id = ?1 AND conversations.deleted_at_ms IS NULL
           AND message_blocks.block_type = 'text'
         ORDER BY conversations.updated_at_ms DESC, messages.created_at_ms DESC,
                  messages.id DESC, message_blocks.ordinal",
    )?;
    let rows = statement.query_map([DEFAULT_PROFILE_ID], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut matches = HashMap::new();
    for row in rows {
        let (conversation_id, message_id, branch_id, content) = row?;
        if matches.contains_key(&conversation_id) {
            continue;
        }
        let normalized = normalized_search_text(&content);
        if normalized.to_lowercase().contains(folded_query) {
            matches.insert(
                conversation_id,
                MessageMatch {
                    message_id,
                    branch_id,
                    snippet: matching_snippet(&normalized, folded_query),
                },
            );
        }
    }
    Ok(matches)
}

/// Reports whether a matching message is already visible on the selected branch lineage.
fn message_is_visible_on_branch(
    connection: &rusqlite::Connection,
    branch_id: &str,
    message_id: &str,
) -> Result<bool, StorageError> {
    connection
        .query_row(
            "WITH RECURSIVE lineage(id, parent_message_id) AS (
                 SELECT id, parent_message_id FROM messages
                 WHERE id = (
                     SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1
                 )
                 UNION ALL
                 SELECT messages.id, messages.parent_message_id
                 FROM messages JOIN lineage ON messages.id = lineage.parent_message_id
             )
             SELECT EXISTS (SELECT 1 FROM lineage WHERE id = ?2)",
            params![branch_id, message_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Collapses whitespace so queries and stored blocks use the same phrase-search shape.
fn normalized_search_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extracts a bounded Unicode-safe excerpt around the first case-insensitive match.
fn matching_snippet(content: &str, folded_query: &str) -> String {
    let mut folded_content = String::new();
    let mut source_characters = Vec::new();
    for (source_index, character) in content.chars().enumerate() {
        for folded_character in character.to_lowercase() {
            folded_content.push(folded_character);
            source_characters.push(source_index);
        }
    }
    let match_byte = folded_content.find(folded_query).unwrap_or_default();
    let folded_match_start = folded_content[..match_byte].chars().count();
    let folded_match_end = folded_match_start + folded_query.chars().count().saturating_sub(1);
    let characters = content.chars().collect::<Vec<_>>();
    let match_start = source_characters
        .get(folded_match_start)
        .copied()
        .unwrap_or_default();
    let match_end = source_characters
        .get(folded_match_end)
        .map_or(match_start, |index| index + 1);
    let start = match_start.saturating_sub(SEARCH_SNIPPET_CONTEXT_CHARACTERS);
    let desired_end = match_end + SEARCH_SNIPPET_CONTEXT_CHARACTERS;
    let end = desired_end
        .min(characters.len())
        .min(start + SEARCH_SNIPPET_MAX_CHARACTERS);
    let excerpt = characters[start..end].iter().collect::<String>();
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        excerpt,
        if end < characters.len() { "…" } else { "" }
    )
}
