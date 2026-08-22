//! Rust-owned bounded `search_attached_files` arguments, execution, and path-free results.

#![allow(dead_code)]

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{
    AttachmentExtractionFormat, ConversationStore, DEFAULT_PROFILE_ID, StorageError,
    memory_filters::{MemorySearchFilters, MemorySourceKind},
    memory_hybrid::MemoryHybridHit,
    memory_semantic::SemanticEmbedder,
};

/// Stable native tool name reserved for hybrid retained-document retrieval.
pub(crate) const SEARCH_ATTACHED_FILES_TOOL_NAME: &str = "search_attached_files";
/// Default number of file excerpts returned when a caller omits the limit.
const DEFAULT_SEARCH_ATTACHED_FILE_RESULTS: usize = 5;
/// Tool-context result ceiling, intentionally narrower than the internal retrieval ceiling.
pub(crate) const MAX_SEARCH_ATTACHED_FILE_RESULTS: usize = 10;
/// Maximum Unicode-scalar length of any excerpt returned by the tool contract.
pub(crate) const MAX_SEARCH_ATTACHED_FILE_EXCERPT_CHARACTERS: usize = 1_200;

/// Typed arguments accepted by Bottie's future provider-independent file-search executor.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SearchAttachedFilesArguments {
    /// Natural-language retrieval query, normalized and capped by native search policy.
    pub(crate) query: String,
    /// Optional durable conversation scope resolved through native attachment associations.
    pub(crate) conversation_id: Option<String>,
    /// Optional inclusive attachment creation-time floor.
    pub(crate) created_after_ms: Option<i64>,
    /// Optional inclusive attachment creation-time ceiling.
    pub(crate) created_before_ms: Option<i64>,
    /// Optional requested match count, capped by the narrower tool-context policy.
    pub(crate) limit: Option<usize>,
}

/// Path-free result returned by the native `search_attached_files` contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchAttachedFilesResult {
    /// Ranked retained-document matches.
    pub(crate) matches: Vec<SearchAttachedFileMatch>,
}

/// One ranked, bounded file excerpt and its durable native provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchAttachedFileMatch {
    /// One-based fused result order without exposing engine-specific scores.
    pub(crate) rank: usize,
    /// Bounded lexical snippet or exact semantic chunk.
    pub(crate) excerpt: String,
    /// Path-free retained-file metadata needed by later citation/open tooling.
    pub(crate) provenance: SearchAttachedFileProvenance,
}

/// Durable path-free provenance for one retained extracted document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchAttachedFileProvenance {
    /// Stable source category for forward-compatible tool consumers.
    pub(crate) source_kind: &'static str,
    /// Exact opaque attachment identity.
    pub(crate) attachment_id: String,
    /// Sanitized leaf name retained for inert presentation.
    pub(crate) display_name: String,
    /// MIME type inferred from the retained bytes.
    pub(crate) mime_type: String,
    /// Exact retained original byte count.
    pub(crate) byte_size: u64,
    /// Native extraction representation searched by this contract.
    pub(crate) extraction_format: AttachmentExtractionFormat,
    /// Unicode-scalar count of the complete extracted document.
    pub(crate) character_count: u64,
    /// Page count for ready PDF documents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) page_count: Option<u64>,
    /// Durable attachment creation time in Unix milliseconds.
    pub(crate) created_at_ms: i64,
    /// Exact semantic-chunk location when fusion found a current indexed chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chunk: Option<SearchAttachedFileChunk>,
}

/// Exact Unicode-scalar position of one semantic excerpt in its complete extracted document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchAttachedFileChunk {
    /// Zero-based deterministic chunk order.
    pub(crate) ordinal: usize,
    /// Inclusive Unicode-scalar source offset.
    pub(crate) start_character: usize,
    /// Exclusive Unicode-scalar source offset.
    pub(crate) end_character: usize,
}

/// Raw path-free attachment metadata loaded after hybrid retrieval.
type RawAttachmentProvenance = (String, String, i64, i64, String, i64, Option<i64>);

impl ConversationStore {
    /// Executes the native-only retained-document tool over the shared hybrid retrieval contract.
    pub(crate) fn execute_search_attached_files(
        &self,
        arguments: SearchAttachedFilesArguments,
        embedder: &mut impl SemanticEmbedder,
    ) -> Result<SearchAttachedFilesResult, StorageError> {
        if arguments.limit == Some(0) {
            return Err(StorageError::invalid(
                "The search_attached_files result limit must be greater than zero.",
            ));
        }
        let conversation_id = arguments.conversation_id.clone();
        let filters = MemorySearchFilters {
            source_kind: Some(MemorySourceKind::Attachment),
            conversation_id: arguments.conversation_id,
            created_after_ms: arguments.created_after_ms,
            created_before_ms: arguments.created_before_ms,
            limit: arguments
                .limit
                .unwrap_or(DEFAULT_SEARCH_ATTACHED_FILE_RESULTS)
                .min(MAX_SEARCH_ATTACHED_FILE_RESULTS),
        };
        let hits = self.search_memory_hybrid(&arguments.query, embedder, filters)?;
        self.load_search_attached_file_results(hits, conversation_id.as_deref())
    }

    /// Resolves ranked native identities into bounded file provenance after retrieval.
    fn load_search_attached_file_results(
        &self,
        hits: Vec<MemoryHybridHit>,
        conversation_id: Option<&str>,
    ) -> Result<SearchAttachedFilesResult, StorageError> {
        let connection = self.open()?;
        let mut matches = Vec::with_capacity(hits.len());
        for hit in hits {
            if hit.source_kind != MemorySourceKind::Attachment {
                continue;
            }
            let provenance = connection
                .query_row(
                    "SELECT attachments.display_name, attachments.mime_type, attachments.byte_size,
                            attachments.created_at_ms, attachment_extractions.format,
                            attachment_extractions.character_count, attachment_extractions.page_count
                     FROM attachments
                     JOIN attachment_extractions
                       ON attachment_extractions.attachment_id = attachments.id
                     WHERE attachments.id = ?1 AND attachment_extractions.state = 'ready'
                       AND (
                           EXISTS (
                               SELECT 1 FROM conversation_attachments
                               JOIN conversations
                                 ON conversations.id = conversation_attachments.conversation_id
                               WHERE conversation_attachments.attachment_id = attachments.id
                                 AND conversations.profile_id = ?2
                                 AND conversations.deleted_at_ms IS NULL
                                 AND NOT EXISTS (
                                     SELECT 1 FROM conversation_memory_preferences
                                     WHERE conversation_id = conversations.id AND excluded = 1
                                 )
                                 AND (?3 IS NULL OR conversations.id = ?3)
                           )
                           OR EXISTS (
                               SELECT 1 FROM message_attachments
                               JOIN messages ON messages.id = message_attachments.message_id
                               JOIN conversations ON conversations.id = messages.conversation_id
                               WHERE message_attachments.attachment_id = attachments.id
                                 AND conversations.profile_id = ?2
                                 AND conversations.deleted_at_ms IS NULL
                                 AND NOT EXISTS (
                                     SELECT 1 FROM conversation_memory_preferences
                                     WHERE conversation_id = conversations.id AND excluded = 1
                                 )
                                 AND (?3 IS NULL OR conversations.id = ?3)
                           )
                       )",
                    params![&hit.source_id, DEFAULT_PROFILE_ID, conversation_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, Option<i64>>(6)?,
                        ))
                    },
                )
                .optional()?;
            let Some(provenance) = provenance else {
                continue;
            };
            matches.push(search_attached_file_match(
                hit,
                provenance,
                matches.len() + 1,
            )?);
        }
        Ok(SearchAttachedFilesResult { matches })
    }
}

/// Converts one hybrid identity plus trusted metadata into a path-free tool match.
fn search_attached_file_match(
    hit: MemoryHybridHit,
    provenance: RawAttachmentProvenance,
    rank: usize,
) -> Result<SearchAttachedFileMatch, StorageError> {
    let (display_name, mime_type, byte_size, created_at_ms, format, character_count, page_count) =
        provenance;
    let chunk = exact_chunk(&hit)?;
    Ok(SearchAttachedFileMatch {
        rank,
        excerpt: bounded_excerpt(&hit.excerpt),
        provenance: SearchAttachedFileProvenance {
            source_kind: MemorySourceKind::Attachment.as_str(),
            attachment_id: hit.source_id,
            display_name,
            mime_type,
            byte_size: u64::try_from(byte_size).map_err(|_| StorageError::internal())?,
            extraction_format: AttachmentExtractionFormat::from_database(&format)?,
            character_count: u64::try_from(character_count)
                .map_err(|_| StorageError::internal())?,
            page_count: page_count
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StorageError::internal())?,
            created_at_ms,
            chunk,
        },
    })
}

/// Converts complete hybrid chunk fields into one all-or-nothing provenance object.
fn exact_chunk(hit: &MemoryHybridHit) -> Result<Option<SearchAttachedFileChunk>, StorageError> {
    match (hit.ordinal, hit.start_character, hit.end_character) {
        (Some(ordinal), Some(start_character), Some(end_character)) => {
            Ok(Some(SearchAttachedFileChunk {
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
    if value.chars().count() <= MAX_SEARCH_ATTACHED_FILE_EXCERPT_CHARACTERS {
        return value.to_owned();
    }
    value
        .chars()
        .take(MAX_SEARCH_ATTACHED_FILE_EXCERPT_CHARACTERS - 1)
        .chain(std::iter::once('…'))
        .collect()
}
