//! Rust-owned bounded `search_memory` tool arguments, execution, and path-free results.

#![allow(dead_code)]

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredRole,
    memory_filters::{MemorySearchFilters, MemorySourceKind},
    memory_hybrid::MemoryHybridHit,
    memory_semantic::SemanticEmbedder,
};

/// Stable native tool name reserved for hybrid conversation-memory retrieval.
pub(crate) const SEARCH_MEMORY_TOOL_NAME: &str = "search_memory";
/// Default number of excerpts returned when a caller omits the limit.
const DEFAULT_SEARCH_MEMORY_RESULTS: usize = 5;
/// Tool-context result ceiling, intentionally narrower than the internal retrieval ceiling.
pub(crate) const MAX_SEARCH_MEMORY_RESULTS: usize = 10;
/// Maximum Unicode-scalar length of any excerpt returned by the tool contract.
pub(crate) const MAX_SEARCH_MEMORY_EXCERPT_CHARACTERS: usize = 1_200;

/// Typed arguments accepted by Bottie's future provider-independent `search_memory` executor.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SearchMemoryArguments {
    /// Natural-language retrieval query, normalized and capped by native search policy.
    pub(crate) query: String,
    /// Optional durable conversation scope.
    pub(crate) conversation_id: Option<String>,
    /// Optional inclusive source creation-time floor.
    pub(crate) created_after_ms: Option<i64>,
    /// Optional inclusive source creation-time ceiling.
    pub(crate) created_before_ms: Option<i64>,
    /// Optional requested match count, capped by the narrower tool-context policy.
    pub(crate) limit: Option<usize>,
}

/// Path-free result returned by the native `search_memory` contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchMemoryResult {
    /// Ranked conversation-message matches.
    pub(crate) matches: Vec<SearchMemoryMatch>,
}

/// One ranked, bounded memory excerpt and its durable native provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchMemoryMatch {
    /// One-based fused result order without exposing engine-specific scores.
    pub(crate) rank: usize,
    /// Bounded lexical snippet or exact semantic chunk.
    pub(crate) excerpt: String,
    /// Path-free conversation and message metadata needed by later citation/open tooling.
    pub(crate) provenance: SearchMemoryProvenance,
}

/// Durable path-free provenance for one conversation-message match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchMemoryProvenance {
    /// Stable source category for forward-compatible tool consumers.
    pub(crate) source_kind: &'static str,
    /// Conversation containing the matched message.
    pub(crate) conversation_id: String,
    /// Bounded durable conversation title for inspectable attribution.
    pub(crate) conversation_title: String,
    /// Exact matched durable message identity.
    pub(crate) message_id: String,
    /// Author role of the matched final message.
    pub(crate) role: StoredRole,
    /// Durable message creation time in Unix milliseconds.
    pub(crate) created_at_ms: i64,
    /// Exact semantic-chunk location when fusion found a current indexed chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chunk: Option<SearchMemoryChunk>,
}

/// Exact Unicode-scalar position of one semantic excerpt in its complete message answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchMemoryChunk {
    /// Zero-based deterministic chunk order.
    pub(crate) ordinal: usize,
    /// Inclusive Unicode-scalar source offset.
    pub(crate) start_character: usize,
    /// Exclusive Unicode-scalar source offset.
    pub(crate) end_character: usize,
}

impl ConversationStore {
    /// Executes the native-only message-memory tool over the shared hybrid retrieval contract.
    pub(crate) fn execute_search_memory(
        &self,
        arguments: SearchMemoryArguments,
        embedder: &mut impl SemanticEmbedder,
    ) -> Result<SearchMemoryResult, StorageError> {
        if arguments.limit == Some(0) {
            return Err(StorageError::invalid(
                "The search_memory result limit must be greater than zero.",
            ));
        }
        let filters = MemorySearchFilters {
            source_kind: Some(MemorySourceKind::Message),
            conversation_id: arguments.conversation_id,
            created_after_ms: arguments.created_after_ms,
            created_before_ms: arguments.created_before_ms,
            limit: arguments
                .limit
                .unwrap_or(DEFAULT_SEARCH_MEMORY_RESULTS)
                .min(MAX_SEARCH_MEMORY_RESULTS),
        };
        let hits = self.search_memory_hybrid(&arguments.query, embedder, filters)?;
        self.load_search_memory_results(hits)
    }

    /// Resolves ranked native identities into bounded message provenance after retrieval.
    fn load_search_memory_results(
        &self,
        hits: Vec<MemoryHybridHit>,
    ) -> Result<SearchMemoryResult, StorageError> {
        let connection = self.open()?;
        let mut matches = Vec::with_capacity(hits.len());
        for hit in hits {
            if hit.source_kind != MemorySourceKind::Message {
                continue;
            }
            let provenance = connection
                .query_row(
                    "SELECT conversations.id, conversations.title, messages.role,
                            messages.created_at_ms
                     FROM messages
                     JOIN conversations ON conversations.id = messages.conversation_id
                     WHERE messages.id = ?1 AND messages.state = 'final'
                       AND conversations.profile_id = ?2
                       AND conversations.deleted_at_ms IS NULL
                       AND NOT EXISTS (
                           SELECT 1 FROM conversation_memory_preferences
                           WHERE conversation_id = conversations.id AND excluded = 1
                       )",
                    params![&hit.source_id, DEFAULT_PROFILE_ID],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()?;
            let Some((conversation_id, conversation_title, role, created_at_ms)) = provenance
            else {
                continue;
            };
            let chunk = exact_chunk(&hit)?;
            matches.push(SearchMemoryMatch {
                rank: matches.len() + 1,
                excerpt: bounded_excerpt(&hit.excerpt),
                provenance: SearchMemoryProvenance {
                    source_kind: MemorySourceKind::Message.as_str(),
                    conversation_id,
                    conversation_title,
                    message_id: hit.source_id,
                    role: StoredRole::from_database(&role)?,
                    created_at_ms,
                    chunk,
                },
            });
        }
        Ok(SearchMemoryResult { matches })
    }
}

/// Converts complete hybrid chunk fields into one all-or-nothing provenance object.
fn exact_chunk(hit: &MemoryHybridHit) -> Result<Option<SearchMemoryChunk>, StorageError> {
    match (hit.ordinal, hit.start_character, hit.end_character) {
        (Some(ordinal), Some(start_character), Some(end_character)) => {
            Ok(Some(SearchMemoryChunk {
                ordinal,
                start_character,
                end_character,
            }))
        }
        (None, None, None) => Ok(None),
        _ => Err(StorageError::internal()),
    }
}

/// Caps an excerpt without splitting a Unicode scalar and preserves a visible truncation marker.
fn bounded_excerpt(value: &str) -> String {
    if value.chars().count() <= MAX_SEARCH_MEMORY_EXCERPT_CHARACTERS {
        return value.to_owned();
    }
    value
        .chars()
        .take(MAX_SEARCH_MEMORY_EXCERPT_CHARACTERS - 1)
        .chain(std::iter::once('…'))
        .collect()
}
