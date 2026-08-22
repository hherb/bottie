//! Native reciprocal-rank-fusion memory retrieval contract tests.

use super::{
    ConversationStore, MessageState, NewStoredMessage, StoredRole,
    memory_hybrid::{RECIPROCAL_RANK_CONSTANT, fuse_ranked_hits},
    memory_lexical::{MemoryLexicalHit, MemorySourceKind},
    memory_semantic::{EMBEDDING_DIMENSIONS, SemanticEmbedder},
    memory_semantic_query::MemorySemanticHit,
    tests::test_database_path,
};

/// Deterministic query embedder with visible invocation capture.
#[derive(Default)]
struct HybridEmbedder {
    inputs: Vec<String>,
}

impl SemanticEmbedder for HybridEmbedder {
    /// Maps north-facing fixture text onto one stable unit vector.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.inputs.extend(texts.iter().cloned());
        Ok(texts
            .iter()
            .map(|text| {
                let mut embedding = vec![0.0; EMBEDDING_DIMENSIONS];
                embedding[usize::from(!text.contains("north"))] = 1.0;
                embedding
            })
            .collect())
    }
}

/// Creates one lexical fixture hit.
fn lexical_hit(source_id: &str, created_at_ms: i64) -> MemoryLexicalHit {
    MemoryLexicalHit {
        source_kind: MemorySourceKind::Message,
        source_id: source_id.into(),
        snippet: format!("lexical {source_id}"),
        rank: -1.0,
        created_at_ms,
    }
}

/// Creates one semantic fixture hit for a source chunk.
fn semantic_hit(source_id: &str, ordinal: usize, created_at_ms: i64) -> MemorySemanticHit {
    MemorySemanticHit {
        source_kind: MemorySourceKind::Message,
        source_id: source_id.into(),
        ordinal,
        start_character: ordinal * 10,
        end_character: ordinal * 10 + 9,
        excerpt: format!("semantic {source_id} chunk {ordinal}"),
        distance: ordinal as f64 / 10.0,
        created_at_ms,
    }
}

/// Appends one final user message and returns its durable identity.
fn append_message(store: &ConversationStore, conversation_id: &str, text: &str) -> String {
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation_id.into(),
                role: StoredRole::User,
                text: text.into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("hybrid fixture message should append")
        .id
}

/// Drains every deterministic chunk through the fixture embedder.
fn index_all(store: &ConversationStore, embedder: &mut HybridEmbedder) {
    while store
        .process_next_semantic_batch(embedder, 8)
        .expect("hybrid fixture batch should succeed")
        .is_some()
    {}
}

#[test]
fn fuses_source_ranks_once_and_prefers_exact_semantic_provenance() {
    let lexical = vec![lexical_hit("shared", 20), lexical_hit("lexical-only", 10)];
    let semantic = vec![
        semantic_hit("shared", 2, 20),
        semantic_hit("shared", 3, 20),
        semantic_hit("semantic-only", 0, 30),
    ];

    let hits = fuse_ranked_hits(lexical, semantic, 10);

    assert_eq!(RECIPROCAL_RANK_CONSTANT, 60);
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].source_id, "shared");
    assert_eq!(hits[0].lexical_rank, Some(1));
    assert_eq!(hits[0].semantic_rank, Some(1));
    assert_eq!(hits[0].ordinal, Some(2));
    assert_eq!(hits[0].excerpt, "semantic shared chunk 2");
    assert_eq!(hits[1].source_id, "lexical-only");
    assert_eq!(hits[2].source_id, "semantic-only");
    let expected_shared_score = 2.0 / (RECIPROCAL_RANK_CONSTANT + 1) as f64;
    assert!((hits[0].score - expected_shared_score).abs() < f64::EPSILON);
}

#[test]
fn caps_fused_results_and_breaks_equal_scores_deterministically() {
    let lexical = (0..55)
        .map(|index| lexical_hit(&format!("source-{index:02}"), index))
        .collect();
    let bounded = fuse_ranked_hits(lexical, Vec::new(), usize::MAX);
    let tied = fuse_ranked_hits(
        vec![lexical_hit("older-lexical", 10)],
        vec![semantic_hit("newer-semantic", 0, 20)],
        2,
    );

    assert_eq!(bounded.len(), 50);
    assert_eq!(tied[0].source_id, "newer-semantic");
    assert_eq!(tied[1].source_id, "older-lexical");
}

#[test]
fn hybrid_query_applies_one_filter_contract_and_excludes_trash() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let retained = store
        .create_conversation("Retained hybrid memory")
        .expect("conversation should create");
    let retained_id = append_message(&store, &retained.id, "north orchard retained memory");
    let trashed = store
        .create_conversation("Trashed hybrid memory")
        .expect("conversation should create");
    append_message(&store, &trashed.id, "north orchard trashed memory");
    store
        .delete_conversation(&trashed.id)
        .expect("conversation should move to trash");
    let mut embedder = HybridEmbedder::default();
    index_all(&store, &mut embedder);

    let retained_across_lifecycle = store
        .search_memory_hybrid(
            "north orchard",
            &mut embedder,
            super::memory_filters::MemorySearchFilters {
                source_kind: Some(MemorySourceKind::Message),
                created_after_ms: Some(0),
                created_before_ms: Some(i64::MAX),
                limit: 5,
                ..super::memory_filters::MemorySearchFilters::default()
            },
        )
        .expect("hybrid query should succeed");
    let conversation_scoped = store
        .search_memory_hybrid(
            "north orchard",
            &mut embedder,
            super::memory_filters::MemorySearchFilters {
                source_kind: Some(MemorySourceKind::Message),
                conversation_id: Some(retained.id),
                limit: 5,
                ..super::memory_filters::MemorySearchFilters::default()
            },
        )
        .expect("conversation-scoped hybrid query should succeed");

    assert_eq!(retained_across_lifecycle.len(), 1);
    assert_eq!(retained_across_lifecycle[0].source_id, retained_id);
    assert_eq!(conversation_scoped.len(), 1);
    assert_eq!(conversation_scoped[0].source_id, retained_id);
    assert!(conversation_scoped[0].lexical_rank.is_some());
    assert!(conversation_scoped[0].semantic_rank.is_some());
}

#[test]
fn hybrid_query_enforces_bounds_before_embedding() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let mut embedder = HybridEmbedder::default();

    let empty = store
        .search_memory_hybrid(
            " \n ",
            &mut embedder,
            super::memory_filters::MemorySearchFilters::default(),
        )
        .expect("empty hybrid query should succeed");
    let zero = store
        .search_memory_hybrid(
            "north",
            &mut embedder,
            super::memory_filters::MemorySearchFilters {
                limit: 0,
                ..super::memory_filters::MemorySearchFilters::default()
            },
        )
        .expect("zero-limit hybrid query should succeed");
    let invalid_filter = store.search_memory_hybrid(
        "north",
        &mut embedder,
        super::memory_filters::MemorySearchFilters {
            conversation_id: Some("  ".into()),
            ..super::memory_filters::MemorySearchFilters::default()
        },
    );
    let too_long = store.search_memory_hybrid(
        &"x".repeat(201),
        &mut embedder,
        super::memory_filters::MemorySearchFilters::default(),
    );

    assert!(empty.is_empty());
    assert!(zero.is_empty());
    assert!(embedder.inputs.is_empty());
    assert_eq!(
        invalid_filter.expect_err("empty filter should fail").code,
        "invalid_request"
    );
    assert_eq!(
        too_long.expect_err("long query should fail").code,
        "invalid_request"
    );
}
