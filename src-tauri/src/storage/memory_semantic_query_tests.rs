//! Native semantic-memory KNN query contract tests.

use std::fs;

use rusqlite::params;

use super::{
    ConversationStore, MessageState, NewStoredMessage, StoredRole,
    memory_lexical::MemorySourceKind,
    memory_semantic::{EMBEDDING_DIMENSIONS, SemanticEmbedder},
    memory_semantic_query::{MemorySemanticFilters, QUERY_INPUT_PREFIX},
    tests::{process_pending_attachments, test_database_path},
};

/// Deterministic fixture embedder with visible query-prefix capture.
#[derive(Default)]
struct DirectionalEmbedder {
    inputs: Vec<String>,
}

impl SemanticEmbedder for DirectionalEmbedder {
    /// Maps fixture direction words onto orthogonal normalized vectors.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.inputs.extend(texts.iter().cloned());
        Ok(texts
            .iter()
            .map(|text| directional_embedding(text))
            .collect())
    }
}

/// Fixture embedder that violates the compiled dimensionality contract.
struct WrongDimensionsEmbedder;

impl SemanticEmbedder for WrongDimensionsEmbedder {
    /// Returns one deliberately malformed query vector.
    fn embed(&mut self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(vec![vec![1.0, 0.0]])
    }
}

/// Fixture embedder that violates the one-vector-per-query contract.
struct WrongCountEmbedder;

impl SemanticEmbedder for WrongCountEmbedder {
    /// Returns no query vector.
    fn embed(&mut self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(Vec::new())
    }
}

/// Fixture embedder that returns a non-finite value rejected by sqlite-vec policy.
struct NonFiniteEmbedder;

impl SemanticEmbedder for NonFiniteEmbedder {
    /// Returns one correctly sized vector containing NaN.
    fn embed(&mut self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let mut embedding = vec![0.0; EMBEDDING_DIMENSIONS];
        embedding[0] = f32::NAN;
        Ok(vec![embedding])
    }
}

/// Produces a stable 768-dimensional vector for one fixture string.
fn directional_embedding(text: &str) -> Vec<f32> {
    let mut embedding = vec![0.0; EMBEDDING_DIMENSIONS];
    if text.contains("north") {
        embedding[0] = 1.0;
    } else if text.contains("east") {
        embedding[1] = 1.0;
    } else {
        embedding[2] = 1.0;
    }
    embedding
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
        .expect("semantic fixture message should append")
        .id
}

/// Drains every deterministic chunk through the fixture embedder.
fn index_all(store: &ConversationStore, embedder: &mut DirectionalEmbedder) {
    while store
        .process_next_semantic_batch(embedder, 8)
        .expect("semantic fixture batch should succeed")
        .is_some()
    {}
}

#[test]
fn embeds_the_versioned_query_and_ranks_bounded_chunk_provenance() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let north = store
        .create_conversation("Northern memory")
        .expect("conversation should create");
    let north_id = append_message(&store, &north.id, "north lighthouse field notes");
    let east = store
        .create_conversation("Eastern memory")
        .expect("conversation should create");
    let east_id = append_message(&store, &east.id, "east orchard field notes");
    let mut embedder = DirectionalEmbedder::default();
    index_all(&store, &mut embedder);

    let hits = store
        .search_memory_semantically(
            "  north lighthouse  ",
            &mut embedder,
            MemorySemanticFilters::default(),
        )
        .expect("semantic query should succeed");

    assert_eq!(
        embedder.inputs.last().map(String::as_str),
        Some("task: search result | query: north lighthouse")
    );
    assert_eq!(QUERY_INPUT_PREFIX, "task: search result | query: ");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].source_kind, MemorySourceKind::Message);
    assert_eq!(hits[0].source_id, north_id);
    assert_eq!(hits[0].ordinal, 0);
    assert_eq!(hits[0].start_character, 0);
    assert_eq!(hits[0].end_character, hits[0].excerpt.chars().count());
    assert_eq!(hits[0].excerpt, "north lighthouse field notes");
    assert!(hits[0].distance < hits[1].distance);

    let connection = store.open().expect("store should open");
    connection
        .execute(
            "UPDATE memory_embedding_records SET index_generation = index_generation + 1
             WHERE chunk_id IN (
                 SELECT id FROM memory_chunks WHERE source_kind = 'message' AND source_id = ?1
             )",
            params![north_id],
        )
        .expect("fixture generation should change");
    drop(connection);
    let current_generation = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                limit: 1,
                ..MemorySemanticFilters::default()
            },
        )
        .expect("current-generation semantic query should succeed");
    assert_eq!(current_generation.len(), 1);
    assert_eq!(current_generation[0].source_id, east_id);
}

#[test]
fn prefilters_attachment_association_and_applies_source_conversation_and_trash_policy() {
    let path = test_database_path();
    let source = path.with_file_name("semantic-private.md");
    fs::write(&source, "north private attachment").expect("fixture should write");
    let store = ConversationStore::initialize(path).expect("storage should initialize");
    let attachment = store
        .ingest_attachment(&source)
        .expect("attachment should ingest");
    process_pending_attachments(&store);
    let active = store
        .create_conversation("Active semantic source")
        .expect("conversation should create");
    let active_message_id = append_message(&store, &active.id, "east active message");
    let retained = store
        .create_conversation("Retained semantic document")
        .expect("conversation should create");
    let mut embedder = DirectionalEmbedder::default();
    index_all(&store, &mut embedder);

    let before_association = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                limit: 1,
                ..MemorySemanticFilters::default()
            },
        )
        .expect("prefiltered semantic query should succeed");
    assert_eq!(before_association.len(), 1);
    assert_eq!(before_association[0].source_id, active_message_id);

    store
        .add_conversation_attachments(&retained.id, std::slice::from_ref(&attachment.id))
        .expect("attachment should gain conversation scope");
    let associated = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                source_kind: Some(MemorySourceKind::Attachment),
                conversation_id: Some(retained.id.clone()),
                ..MemorySemanticFilters::default()
            },
        )
        .expect("associated semantic query should succeed");
    assert_eq!(associated.len(), 1);
    assert_eq!(associated[0].source_id, attachment.id);

    store
        .set_conversation_archived(&retained.id, true)
        .expect("conversation should archive");
    let archived = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                source_kind: Some(MemorySourceKind::Attachment),
                ..MemorySemanticFilters::default()
            },
        )
        .expect("archived semantic query should succeed");
    assert_eq!(archived.len(), 1);

    store
        .delete_conversation(&retained.id)
        .expect("conversation should move to trash");
    let trashed = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                source_kind: Some(MemorySourceKind::Attachment),
                ..MemorySemanticFilters::default()
            },
        )
        .expect("trashed semantic query should succeed");
    assert!(trashed.is_empty());
}

#[test]
fn applies_inclusive_dates_and_rejects_malformed_native_filters() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Semantic date filter")
        .expect("conversation should create");
    append_message(&store, &conversation.id, "north dated memory");
    let mut embedder = DirectionalEmbedder::default();
    index_all(&store, &mut embedder);
    let initial = store
        .search_memory_semantically("north", &mut embedder, MemorySemanticFilters::default())
        .expect("initial semantic query should succeed");
    let created_at_ms = initial[0].created_at_ms;

    let inclusive = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                created_after_ms: Some(created_at_ms),
                created_before_ms: Some(created_at_ms),
                ..MemorySemanticFilters::default()
            },
        )
        .expect("inclusive date query should succeed");
    let empty_conversation = store.search_memory_semantically(
        "north",
        &mut embedder,
        MemorySemanticFilters {
            conversation_id: Some("  ".into()),
            ..MemorySemanticFilters::default()
        },
    );
    let reversed_dates = store.search_memory_semantically(
        "north",
        &mut embedder,
        MemorySemanticFilters {
            created_after_ms: Some(2),
            created_before_ms: Some(1),
            ..MemorySemanticFilters::default()
        },
    );

    assert_eq!(inclusive.len(), 1);
    assert_eq!(
        empty_conversation
            .expect_err("empty filter should fail")
            .code,
        "invalid_request"
    );
    assert_eq!(
        reversed_dates.expect_err("reversed dates should fail").code,
        "invalid_request"
    );
}

#[test]
fn enforces_query_vector_and_result_bounds_without_downloading_a_model() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    for index in 0..55 {
        let conversation = store
            .create_conversation(&format!("Bounded semantic memory {index}"))
            .expect("conversation should create");
        append_message(
            &store,
            &conversation.id,
            &format!("north bounded item {index}"),
        );
    }
    let mut embedder = DirectionalEmbedder::default();
    index_all(&store, &mut embedder);

    let indexed_input_count = embedder.inputs.len();
    let empty = store
        .search_memory_semantically(" \n ", &mut embedder, MemorySemanticFilters::default())
        .expect("empty semantic query should succeed");
    let zero_limit = store
        .search_memory_semantically(
            "north",
            &mut embedder,
            MemorySemanticFilters {
                limit: 0,
                ..MemorySemanticFilters::default()
            },
        )
        .expect("zero-limit semantic query should succeed");
    assert_eq!(embedder.inputs.len(), indexed_input_count);
    let bounded = store
        .search_memory_semantically("north", &mut embedder, MemorySemanticFilters::default())
        .expect("bounded semantic query should succeed");
    let too_long = store.search_memory_semantically(
        &"x".repeat(201),
        &mut embedder,
        MemorySemanticFilters::default(),
    );
    let mut wrong_dimensions = WrongDimensionsEmbedder;
    let invalid_vector = store.search_memory_semantically(
        "north",
        &mut wrong_dimensions,
        MemorySemanticFilters::default(),
    );
    let mut wrong_count = WrongCountEmbedder;
    let missing_vector = store.search_memory_semantically(
        "north",
        &mut wrong_count,
        MemorySemanticFilters::default(),
    );
    let mut non_finite = NonFiniteEmbedder;
    let non_finite_vector = store.search_memory_semantically(
        "north",
        &mut non_finite,
        MemorySemanticFilters::default(),
    );

    assert!(empty.is_empty());
    assert!(zero_limit.is_empty());
    assert_eq!(bounded.len(), 50);
    assert_eq!(
        too_long.expect_err("long query should fail").code,
        "invalid_request"
    );
    assert_eq!(
        invalid_vector.expect_err("invalid vector should fail").code,
        "internal"
    );
    assert_eq!(
        missing_vector.expect_err("missing vector should fail").code,
        "internal"
    );
    assert_eq!(
        non_finite_vector
            .expect_err("non-finite vector should fail")
            .code,
        "internal"
    );
}
