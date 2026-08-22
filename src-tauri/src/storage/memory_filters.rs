//! Shared native filter and bound policy for lexical, semantic, and hybrid memory retrieval.

use super::StorageError;

pub(super) const MAX_MEMORY_QUERY_CHARACTERS: usize = 200;
pub(super) const MAX_MEMORY_RESULTS: usize = 50;
const DEFAULT_MEMORY_RESULTS: usize = MAX_MEMORY_RESULTS;

/// Native memory source category that never crosses IPC.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum MemorySourceKind {
    /// One complete final user or assistant message answer, excluding reasoning.
    Message,
    /// One complete ready extracted attachment document.
    Attachment,
}

impl MemorySourceKind {
    /// Returns the stable derived-index representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Attachment => "attachment",
        }
    }

    /// Parses a trusted source kind constrained by migration-owned triggers.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "message" => Ok(Self::Message),
            "attachment" => Ok(Self::Attachment),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Shared native filters applied by every memory retrieval strategy.
#[derive(Clone, Debug)]
pub(crate) struct MemorySearchFilters {
    /// Optional source-category restriction.
    pub(crate) source_kind: Option<MemorySourceKind>,
    /// Optional conversation scope resolved through native durable associations.
    pub(crate) conversation_id: Option<String>,
    /// Optional inclusive source creation-time floor.
    pub(crate) created_after_ms: Option<i64>,
    /// Optional inclusive source creation-time ceiling.
    pub(crate) created_before_ms: Option<i64>,
    /// Requested result count, capped by native policy.
    pub(crate) limit: usize,
}

impl MemorySearchFilters {
    /// Rejects contradictory or malformed values before retrieval work begins.
    pub(super) fn validate(&self) -> Result<(), StorageError> {
        if self
            .conversation_id
            .as_deref()
            .is_some_and(|conversation_id| conversation_id.trim().is_empty())
        {
            return Err(StorageError::invalid(
                "A memory conversation filter cannot be empty.",
            ));
        }
        if self
            .created_after_ms
            .zip(self.created_before_ms)
            .is_some_and(|(after, before)| after > before)
        {
            return Err(StorageError::invalid(
                "The memory date filter has an invalid range.",
            ));
        }
        Ok(())
    }

    /// Returns a copy with a bounded internal candidate count.
    pub(super) fn with_candidate_limit(&self) -> Self {
        Self {
            limit: MAX_MEMORY_RESULTS,
            ..self.clone()
        }
    }
}

impl Default for MemorySearchFilters {
    fn default() -> Self {
        Self {
            source_kind: None,
            conversation_id: None,
            created_after_ms: None,
            created_before_ms: None,
            limit: DEFAULT_MEMORY_RESULTS,
        }
    }
}

/// Normalizes bounded user query text consistently across retrieval strategies.
pub(super) fn normalized_memory_query(value: &str) -> Result<String, StorageError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() > MAX_MEMORY_QUERY_CHARACTERS {
        return Err(StorageError::invalid(format!(
            "Memory search is limited to {MAX_MEMORY_QUERY_CHARACTERS} characters."
        )));
    }
    Ok(normalized)
}
