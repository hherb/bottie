//! Native-only reciprocal-rank fusion over Bottie's lexical and semantic memory retrieval.

#![allow(dead_code)]

use std::collections::HashMap;

use super::{
    ConversationStore, StorageError,
    memory_filters::{
        MAX_MEMORY_RESULTS, MemorySearchFilters, MemorySourceKind, normalized_memory_query,
    },
    memory_lexical::MemoryLexicalHit,
    memory_semantic::SemanticEmbedder,
    memory_semantic_query::MemorySemanticHit,
};

/// Conventional RRF rank offset that limits outsized influence from list leaders.
pub(super) const RECIPROCAL_RANK_CONSTANT: usize = 60;

/// One source-level fused memory result with optional exact chunk provenance.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemoryHybridHit {
    /// Indexed source category.
    pub(crate) source_kind: MemorySourceKind,
    /// Opaque native message or attachment identity.
    pub(crate) source_id: String,
    /// Bounded lexical snippet or preferred exact semantic chunk.
    pub(crate) excerpt: String,
    /// Zero-based semantic chunk order when semantic retrieval found the source.
    pub(crate) ordinal: Option<usize>,
    /// Inclusive Unicode-scalar source offset when exact chunk provenance exists.
    pub(crate) start_character: Option<usize>,
    /// Exclusive Unicode-scalar source offset when exact chunk provenance exists.
    pub(crate) end_character: Option<usize>,
    /// One-based lexical source rank, if present.
    pub(crate) lexical_rank: Option<usize>,
    /// One-based rank of the source's strongest semantic chunk, if present.
    pub(crate) semantic_rank: Option<usize>,
    /// Sum of reciprocal contributions from the source's distinct result lists.
    pub(crate) score: f64,
    /// Durable source creation time used only for deterministic tie-breaking.
    pub(crate) created_at_ms: i64,
}

impl ConversationStore {
    /// Retrieves bounded candidates under one filter contract and fuses their source ranks.
    pub(crate) fn search_memory_hybrid(
        &self,
        query: &str,
        embedder: &mut impl SemanticEmbedder,
        filters: MemorySearchFilters,
    ) -> Result<Vec<MemoryHybridHit>, StorageError> {
        let query = normalized_memory_query(query)?;
        filters.validate()?;
        if query.is_empty() || filters.limit == 0 {
            return Ok(Vec::new());
        }
        let candidate_filters = filters.with_candidate_limit();
        let lexical = self.search_memory_lexically(&query, candidate_filters.clone())?;
        let semantic = self
            .search_memory_semantically(&query, embedder, candidate_filters)
            .unwrap_or_default();
        Ok(fuse_ranked_hits(lexical, semantic, filters.limit))
    }
}

/// Fuses source-level ranks while counting at most one contribution from each result list.
pub(super) fn fuse_ranked_hits(
    lexical: Vec<MemoryLexicalHit>,
    semantic: Vec<MemorySemanticHit>,
    limit: usize,
) -> Vec<MemoryHybridHit> {
    let mut hits = HashMap::<(MemorySourceKind, String), MemoryHybridHit>::new();
    for (index, hit) in lexical.into_iter().enumerate() {
        let rank = index + 1;
        let key = (hit.source_kind, hit.source_id.clone());
        hits.entry(key).or_insert_with(|| MemoryHybridHit {
            source_kind: hit.source_kind,
            source_id: hit.source_id,
            excerpt: hit.snippet,
            ordinal: None,
            start_character: None,
            end_character: None,
            lexical_rank: Some(rank),
            semantic_rank: None,
            score: reciprocal_rank_score(rank),
            created_at_ms: hit.created_at_ms,
        });
    }
    for (index, hit) in semantic.into_iter().enumerate() {
        add_semantic_hit(&mut hits, hit, index + 1);
    }
    let mut hits = hits.into_values().collect::<Vec<_>>();
    hits.sort_by(compare_fused_hits);
    hits.truncate(limit.min(MAX_MEMORY_RESULTS));
    hits
}

/// Adds only the strongest chunk rank for a source and prefers its exact excerpt provenance.
fn add_semantic_hit(
    hits: &mut HashMap<(MemorySourceKind, String), MemoryHybridHit>,
    hit: MemorySemanticHit,
    rank: usize,
) {
    let key = (hit.source_kind, hit.source_id.clone());
    let entry = hits.entry(key).or_insert_with(|| MemoryHybridHit {
        source_kind: hit.source_kind,
        source_id: hit.source_id.clone(),
        excerpt: hit.excerpt.clone(),
        ordinal: Some(hit.ordinal),
        start_character: Some(hit.start_character),
        end_character: Some(hit.end_character),
        lexical_rank: None,
        semantic_rank: Some(rank),
        score: reciprocal_rank_score(rank),
        created_at_ms: hit.created_at_ms,
    });
    if entry.semantic_rank.is_some() {
        return;
    }
    entry.excerpt = hit.excerpt;
    entry.ordinal = Some(hit.ordinal);
    entry.start_character = Some(hit.start_character);
    entry.end_character = Some(hit.end_character);
    entry.semantic_rank = Some(rank);
    entry.score += reciprocal_rank_score(rank);
    entry.created_at_ms = entry.created_at_ms.max(hit.created_at_ms);
}

/// Calculates one reciprocal contribution from a one-based rank.
fn reciprocal_rank_score(rank: usize) -> f64 {
    1.0 / (RECIPROCAL_RANK_CONSTANT + rank) as f64
}

/// Orders by fused relevance and stable native provenance without raw-score assumptions.
fn compare_fused_hits(left: &MemoryHybridHit, right: &MemoryHybridHit) -> std::cmp::Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| best_rank(left).cmp(&best_rank(right)))
        .then_with(|| right.created_at_ms.cmp(&left.created_at_ms))
        .then_with(|| left.source_kind.as_str().cmp(right.source_kind.as_str()))
        .then_with(|| left.source_id.cmp(&right.source_id))
}

/// Returns the strongest available source rank for deterministic equal-score ordering.
fn best_rank(hit: &MemoryHybridHit) -> usize {
    hit.lexical_rank
        .into_iter()
        .chain(hit.semantic_rank)
        .min()
        .unwrap_or(usize::MAX)
}
