//! Native `search_memory` tool-contract tests.

use serde_json::json;

use super::{
    ConversationStore, MessageState, NewStoredMessage, StoredRole,
    memory_semantic::{EMBEDDING_DIMENSIONS, SemanticEmbedder},
    memory_tool::{
        MAX_SEARCH_MEMORY_EXCERPT_CHARACTERS, MAX_SEARCH_MEMORY_RESULTS, SEARCH_MEMORY_TOOL_NAME,
        SearchMemoryArguments,
    },
    tests::test_database_path,
};

/// Deterministic embedder that records whether invalid requests reached model work.
#[derive(Default)]
struct ToolEmbedder {
    inputs: Vec<String>,
}

impl SemanticEmbedder for ToolEmbedder {
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

/// Appends one final message and returns its durable identity.
fn append_message(
    store: &ConversationStore,
    conversation_id: &str,
    role: StoredRole,
    text: &str,
) -> String {
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation_id.into(),
                role,
                text: text.into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("memory-tool fixture message should append")
        .id
}

/// Drains every deterministic chunk through the fixture embedder.
fn index_all(store: &ConversationStore, embedder: &mut ToolEmbedder) {
    while store
        .process_next_semantic_batch(embedder, 8)
        .expect("memory-tool fixture batch should succeed")
        .is_some()
    {}
}

#[test]
fn returns_ranked_path_free_message_provenance() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Architecture discussion")
        .expect("conversation should create");
    let message_id = append_message(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Keep the north boundary inside the Rust core.",
    );
    let outside_conversation = store
        .create_conversation("Outside scope")
        .expect("second conversation should create");
    append_message(
        &store,
        &outside_conversation.id,
        StoredRole::User,
        "The north boundary appears outside the requested conversation.",
    );
    let mut embedder = ToolEmbedder::default();
    index_all(&store, &mut embedder);

    let result = store
        .execute_search_memory(
            SearchMemoryArguments {
                query: "north boundary".into(),
                conversation_id: Some(conversation.id.clone()),
                created_after_ms: Some(0),
                created_before_ms: Some(i64::MAX),
                limit: Some(5),
            },
            &mut embedder,
        )
        .expect("search_memory should succeed");

    assert_eq!(SEARCH_MEMORY_TOOL_NAME, "search_memory");
    assert_eq!(result.matches.len(), 1);
    let matched = &result.matches[0];
    assert_eq!(matched.rank, 1);
    assert_eq!(
        matched.excerpt,
        "Keep the north boundary inside the Rust core."
    );
    assert_eq!(matched.provenance.source_kind, "message");
    assert_eq!(matched.provenance.conversation_id, conversation.id);
    assert_eq!(
        matched.provenance.conversation_title,
        "Architecture discussion"
    );
    assert_eq!(matched.provenance.message_id, message_id);
    assert_eq!(matched.provenance.role, StoredRole::Assistant);
    assert_eq!(
        matched.provenance.chunk.as_ref().map(|chunk| chunk.ordinal),
        Some(0)
    );
    assert_eq!(
        matched
            .provenance
            .chunk
            .as_ref()
            .map(|chunk| chunk.start_character),
        Some(0)
    );
    assert_eq!(
        matched
            .provenance
            .chunk
            .as_ref()
            .map(|chunk| chunk.end_character),
        Some(matched.excerpt.chars().count())
    );

    let serialized = serde_json::to_value(&result).expect("tool result should serialize");
    assert_eq!(
        serialized["matches"][0]["provenance"]["sourceKind"],
        json!("message")
    );
    assert!(serialized["matches"][0].get("score").is_none());
    assert!(serialized["matches"][0].get("lexicalRank").is_none());
    assert!(serialized["matches"][0].get("semanticRank").is_none());
    let serialized_text = serialized.to_string();
    assert!(!serialized_text.contains("databasePath"));
    assert!(!serialized_text.contains("filePath"));
    assert!(!serialized_text.contains("embedding"));
    assert!(!serialized_text.contains("distance"));
}

#[test]
fn caps_results_and_excerpt_text_at_tool_specific_limits() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Bounded memories")
        .expect("conversation should create");
    for index in 0..(MAX_SEARCH_MEMORY_RESULTS + 3) {
        append_message(
            &store,
            &conversation.id,
            StoredRole::User,
            &format!("north memory {index} {}", "x".repeat(1_400)),
        );
    }
    let mut embedder = ToolEmbedder::default();
    index_all(&store, &mut embedder);

    let result = store
        .execute_search_memory(
            SearchMemoryArguments {
                query: "north memory".into(),
                limit: Some(usize::MAX),
                ..SearchMemoryArguments::default()
            },
            &mut embedder,
        )
        .expect("bounded search should succeed");

    assert_eq!(result.matches.len(), MAX_SEARCH_MEMORY_RESULTS);
    assert!(result.matches.iter().all(|matched| {
        matched.excerpt.chars().count() <= MAX_SEARCH_MEMORY_EXCERPT_CHARACTERS
    }));
}

#[test]
fn rejects_invalid_arguments_before_embedding_and_denies_unknown_fields() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let mut embedder = ToolEmbedder::default();
    let invalid_requests = [
        SearchMemoryArguments {
            query: "x".repeat(201),
            ..SearchMemoryArguments::default()
        },
        SearchMemoryArguments {
            query: "north".into(),
            conversation_id: Some("  ".into()),
            ..SearchMemoryArguments::default()
        },
        SearchMemoryArguments {
            query: "north".into(),
            created_after_ms: Some(2),
            created_before_ms: Some(1),
            ..SearchMemoryArguments::default()
        },
        SearchMemoryArguments {
            query: "north".into(),
            limit: Some(0),
            ..SearchMemoryArguments::default()
        },
    ];

    for arguments in invalid_requests {
        assert_eq!(
            store
                .execute_search_memory(arguments, &mut embedder)
                .expect_err("invalid tool arguments should fail")
                .code,
            "invalid_request"
        );
    }
    assert!(embedder.inputs.is_empty());

    let unknown = serde_json::from_value::<SearchMemoryArguments>(json!({
        "query": "north",
        "includePaths": true
    }));
    assert!(unknown.is_err());
}

#[test]
fn empty_query_returns_no_matches_without_embedding() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let mut embedder = ToolEmbedder::default();
    let result = store
        .execute_search_memory(SearchMemoryArguments::default(), &mut embedder)
        .expect("empty query should return an empty result");

    assert!(result.matches.is_empty());
    assert!(embedder.inputs.is_empty());
}

#[test]
fn keeps_archived_messages_excludes_trash_and_omits_chunk_for_lexical_fallback() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let archived = store
        .create_conversation("Archived memory")
        .expect("archived conversation should create");
    append_message(
        &store,
        &archived.id,
        StoredRole::User,
        "violet archive phrase",
    );
    store
        .set_conversation_archived(&archived.id, true)
        .expect("conversation should archive");
    let trashed = store
        .create_conversation("Trashed memory")
        .expect("trashed conversation should create");
    append_message(&store, &trashed.id, StoredRole::User, "violet trash phrase");
    store
        .delete_conversation(&trashed.id)
        .expect("conversation should move to trash");
    let mut embedder = ToolEmbedder::default();

    let result = store
        .execute_search_memory(
            SearchMemoryArguments {
                query: "violet phrase".into(),
                ..SearchMemoryArguments::default()
            },
            &mut embedder,
        )
        .expect("lexical fallback should succeed without indexed vectors");

    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].provenance.conversation_id, archived.id);
    assert!(result.matches[0].provenance.chunk.is_none());
    assert_eq!(embedder.inputs.len(), 1);
    let serialized = serde_json::to_value(&result).expect("fallback result should serialize");
    assert!(
        serialized["matches"][0]["provenance"]
            .get("chunk")
            .is_none()
    );
}
