//! Native resumable semantic-index contract tests.

use rusqlite::params;

use super::{
    ConversationStore, MessageState, NewStoredMessage, StoredRole,
    memory_semantic::{
        EMBEDDING_DIMENSIONS, EMBEDDING_MODEL_CODE, EMBEDDING_MODEL_VARIANT,
        EMBEDDING_RUNTIME_VERSION, EMBEDDING_VERSION, INDEX_GENERATION,
        REMOVE_MEMORY_SEMANTIC_SCHEMA_FOR_TEST, SemanticEmbedder, SemanticIndexState,
    },
    tests::test_database_path,
};

/// Deterministic test embedder that never downloads or initializes a model runtime.
struct FixtureEmbedder {
    dimensions: usize,
}

impl FixtureEmbedder {
    /// Creates a fixture returning the requested number of dimensions.
    fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

impl SemanticEmbedder for FixtureEmbedder {
    /// Converts each input into a stable vector whose first value records its length.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut embedding = vec![0.0; self.dimensions];
                if let Some(first) = embedding.first_mut() {
                    *first = text.chars().count() as f32;
                }
                embedding
            })
            .collect())
    }
}

/// Appends one final message large enough to exercise multiple chunks.
fn append_chunked_message(store: &ConversationStore, conversation_id: &str) {
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation_id.into(),
                role: StoredRole::User,
                text: format!("semantic kingfisher {}", "bounded words ".repeat(180)),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("semantic fixture should append");
}

#[test]
fn migration_registers_static_vec_and_records_versioned_empty_index() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let connection = store.open().expect("store should open");
    connection
        .execute_batch(REMOVE_MEMORY_SEMANTIC_SCHEMA_FOR_TEST)
        .expect("semantic schema should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version = 18", [])
        .expect("semantic migration record should be removable");
    connection
        .pragma_update(None, "user_version", 17)
        .expect("fixture version should rewind");
    drop(connection);
    drop(store);

    let upgraded =
        ConversationStore::initialize(path).expect("version seventeen store should upgrade");
    let connection = upgraded.open().expect("upgraded store should open");
    let vec_version: String = connection
        .query_row("SELECT vec_version()", [], |row| row.get(0))
        .expect("sqlite-vec should be statically registered");
    let metadata: (i64, String, String, String, i64, i64, i64, String) = connection
        .query_row(
            "SELECT embedding_version, model_code, model_variant, runtime_version,
                    dimensions, chunking_version, index_generation, state
             FROM memory_semantic_metadata WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .expect("semantic metadata should load");

    assert!(vec_version.starts_with('v'));
    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        18
    );
    assert_eq!(
        metadata,
        (
            EMBEDDING_VERSION,
            EMBEDDING_MODEL_CODE.into(),
            EMBEDDING_MODEL_VARIANT.into(),
            EMBEDDING_RUNTIME_VERSION.into(),
            EMBEDDING_DIMENSIONS as i64,
            super::memory_chunks::CHUNKING_VERSION,
            INDEX_GENERATION,
            "ready".into(),
        )
    );
}

#[test]
fn semantic_batches_persist_and_resume_without_duplicate_vectors() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Semantic resume")
        .expect("conversation should create");
    append_chunked_message(&store, &conversation.id);
    let initial = store
        .semantic_index_status_for_test()
        .expect("semantic status should load");
    assert_eq!(initial.state, SemanticIndexState::Pending);
    assert!(initial.total_chunks > 1);
    assert_eq!(initial.completed_chunks, 0);

    let mut embedder = FixtureEmbedder::new(EMBEDDING_DIMENSIONS);
    let first = store
        .process_next_semantic_batch(&mut embedder, 1)
        .expect("first semantic batch should succeed")
        .expect("one batch should be available");
    assert_eq!(first.completed_chunks, 1);
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    let resumed = reopened
        .semantic_index_status_for_test()
        .expect("resumed status should load");
    assert_eq!(resumed.completed_chunks, 1);
    while reopened
        .process_next_semantic_batch(&mut embedder, 2)
        .expect("resumed semantic batch should succeed")
        .is_some()
    {}
    let completed = reopened
        .semantic_index_status_for_test()
        .expect("completed status should load");
    let connection = reopened.open().expect("store should open");
    let record_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM memory_embedding_records", [], |row| {
            row.get(0)
        })
        .expect("embedding records should count");
    let vector_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM memory_vector_index", [], |row| {
            row.get(0)
        })
        .expect("semantic vectors should count");
    let query = vec![1.0_f32; EMBEDDING_DIMENSIONS]
        .into_iter()
        .flat_map(f32::to_ne_bytes)
        .collect::<Vec<_>>();
    let nearest_distance: f64 = connection
        .query_row(
            "SELECT distance FROM memory_vector_index
             WHERE embedding MATCH ?1 AND k = 1",
            [query],
            |row| row.get(0),
        )
        .expect("sqlite-vec should execute a native cosine query");

    assert_eq!(completed.state, SemanticIndexState::Ready);
    assert_eq!(completed.completed_chunks, completed.total_chunks);
    assert_eq!(record_count, completed.total_chunks as i64);
    assert_eq!(vector_count, record_count);
    assert!(nearest_distance.is_finite());
}

#[test]
fn invalid_embedding_dimensions_fail_without_partial_vector_rows() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Semantic dimensions")
        .expect("conversation should create");
    append_chunked_message(&store, &conversation.id);
    let mut embedder = FixtureEmbedder::new(2);

    let error = store
        .process_next_semantic_batch(&mut embedder, 2)
        .expect_err("wrong dimensions should fail");
    let status = store
        .semantic_index_status_for_test()
        .expect("failed status should load");
    let connection = store.open().expect("store should open");
    let counts: (i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM memory_embedding_records),
                (SELECT COUNT(*) FROM memory_vector_index)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("semantic rows should count");

    assert_eq!(error.code, "internal");
    assert_eq!(status.state, SemanticIndexState::Failed);
    assert_eq!(status.error_code.as_deref(), Some("embedding_dimensions"));
    assert_eq!(counts, (0, 0));
}

#[test]
fn deleting_a_chunk_cascades_its_semantic_vector() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Semantic cleanup")
        .expect("conversation should create");
    append_chunked_message(&store, &conversation.id);
    let mut embedder = FixtureEmbedder::new(EMBEDDING_DIMENSIONS);
    store
        .process_next_semantic_batch(&mut embedder, 1)
        .expect("semantic batch should succeed")
        .expect("one semantic batch should exist");
    let connection = store.open().expect("store should open");
    let chunk_id: String = connection
        .query_row(
            "SELECT chunk_id FROM memory_embedding_records LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("embedded chunk should load");
    connection
        .execute("DELETE FROM memory_chunks WHERE id = ?1", params![chunk_id])
        .expect("chunk should delete");
    let counts: (i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM memory_embedding_records),
                (SELECT COUNT(*) FROM memory_vector_index)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("semantic rows should count");

    assert_eq!(counts, (0, 0));
}

#[test]
fn explicit_reindex_clears_only_derived_vectors_and_resets_durable_progress() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Semantic rebuild")
        .expect("conversation should create");
    append_chunked_message(&store, &conversation.id);
    let mut embedder = FixtureEmbedder::new(EMBEDDING_DIMENSIONS);
    store
        .process_next_semantic_batch(&mut embedder, 1)
        .expect("semantic batch should succeed")
        .expect("one semantic batch should exist");

    let reset = store
        .reset_semantic_index()
        .expect("semantic reindex should reset derived rows");
    let connection = store.open().expect("store should open");
    let counts: (i64, i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM memory_chunks),
                (SELECT COUNT(*) FROM memory_embedding_records),
                (SELECT COUNT(*) FROM memory_vector_index)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("semantic rows should count");

    assert_eq!(reset.state, SemanticIndexState::Pending);
    assert_eq!(reset.completed_chunks, 0);
    assert_eq!(reset.total_chunks as i64, counts.0);
    assert_eq!(reset.error_code, None);
    assert!(counts.0 > 0);
    assert_eq!((counts.1, counts.2), (0, 0));
    assert_eq!(
        serde_json::to_value(&reset).expect("path-free progress should serialize"),
        serde_json::json!({
            "state": "pending",
            "completedChunks": 0,
            "totalChunks": counts.0,
            "errorCode": null,
        })
    );
    drop(connection);
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    assert_eq!(
        reopened
            .semantic_index_status_for_test()
            .expect("reset progress should survive reopen"),
        reset
    );
}

#[test]
fn explicit_reindex_keeps_an_empty_index_ready() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");

    let reset = store
        .reset_semantic_index()
        .expect("empty semantic reindex should succeed");

    assert_eq!(reset.state, SemanticIndexState::Ready);
    assert_eq!(reset.completed_chunks, 0);
    assert_eq!(reset.total_chunks, 0);
    assert_eq!(reset.error_code, None);
}
