//! Durable per-conversation memory-exclusion contract tests.

use std::fs;

use super::{
    ConversationStore, MessageState, NewProviderRun, NewStoredMessage, ProviderRunState,
    StoredReasoningEffort, StoredRole,
    memory_chunks::MemoryChunkSourceKind,
    memory_file_tool::SearchAttachedFilesArguments,
    memory_lexical::{MemoryLexicalFilters, MemorySourceKind},
    memory_open::OpenMemoryArguments,
    memory_semantic::{EMBEDDING_DIMENSIONS, SemanticEmbedder},
    memory_semantic_query::MemorySemanticFilters,
    memory_tool::SearchMemoryArguments,
    tests::{process_pending_attachments, test_database_path},
};

/// Deterministic embedder used to exercise semantic policy without model downloads.
#[derive(Default)]
struct FixtureEmbedder;

impl SemanticEmbedder for FixtureEmbedder {
    /// Maps every fixture string to one valid stable vector.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts
            .iter()
            .map(|_| {
                let mut embedding = vec![0.0; EMBEDDING_DIMENSIONS];
                embedding[0] = 1.0;
                embedding
            })
            .collect())
    }
}

/// Appends one final user message and returns its opaque identity.
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
        .expect("memory-exclusion fixture should append")
        .id
}

/// Drains every pending deterministic chunk through the fixture embedder.
fn index_all(store: &ConversationStore, embedder: &mut FixtureEmbedder) {
    while store
        .process_next_semantic_batch(embedder, 8)
        .expect("semantic fixture batch should succeed")
        .is_some()
    {}
}

#[test]
fn rejects_exclusion_changes_while_native_generation_owns_the_conversation() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Active generation")
        .expect("conversation should create");
    let request_message_id =
        append_message(&store, &conversation.id, "keep current context stable");
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id,
            provider_id: "ollama".into(),
            model_id: "fixture".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: None,
        })
        .expect("provider run should start");

    let error = store
        .set_conversation_memory_excluded(&conversation.id, true)
        .expect_err("active generation should block memory changes");
    assert_eq!(error.code, "invalid_request");
    store
        .finish_provider_run(&run_id, ProviderRunState::Cancelled, None, None)
        .expect("provider run should finish");
}

#[test]
fn persists_reversible_exclusion_and_enforces_every_message_memory_path() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Private planning")
        .expect("conversation should create");
    let message_id = append_message(&store, &conversation.id, "violet excluded memory phrase");
    let mut embedder = FixtureEmbedder;
    index_all(&store, &mut embedder);

    let excluded = store
        .set_conversation_memory_excluded(&conversation.id, true)
        .expect("conversation should be excluded");
    assert!(excluded.memory_excluded);
    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &message_id)
            .expect("message chunks should load")
            .is_empty()
    );
    assert!(
        store
            .search_memory_lexically("violet", MemoryLexicalFilters::default())
            .expect("lexical search should succeed")
            .is_empty()
    );
    assert!(
        store
            .search_memory_semantically("violet", &mut embedder, MemorySemanticFilters::default())
            .expect("semantic search should succeed")
            .is_empty()
    );
    assert!(
        store
            .execute_search_memory(
                SearchMemoryArguments {
                    query: "violet".into(),
                    ..SearchMemoryArguments::default()
                },
                &mut embedder,
            )
            .expect("message tool should succeed")
            .matches
            .is_empty()
    );
    assert_eq!(
        store
            .execute_open_memory(OpenMemoryArguments {
                conversation_id: conversation.id.clone(),
                message_id: message_id.clone(),
                before: None,
                after: None,
            })
            .expect_err("excluded provenance should not open")
            .code,
        "not_found"
    );
    assert_eq!(
        store
            .load_conversation(&conversation.id)
            .expect("source conversation should remain")
            .messages[0]
            .text,
        "violet excluded memory phrase"
    );
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    let listed = reopened
        .list_conversations()
        .expect("conversations should list");
    assert!(listed[0].memory_excluded);
    let included = reopened
        .set_conversation_memory_excluded(&conversation.id, false)
        .expect("conversation should return to memory");
    assert!(!included.memory_excluded);
    assert!(
        !reopened
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &message_id)
            .expect("rebuilt chunks should load")
            .is_empty()
    );
    assert_eq!(
        reopened
            .search_memory_lexically("violet", MemoryLexicalFilters::default())
            .expect("re-enabled lexical search should succeed")
            .len(),
        1
    );
}

#[test]
fn excludes_documents_only_through_the_target_conversation_association() {
    let path = test_database_path();
    let source = path.with_file_name("shared-memory.txt");
    fs::write(&source, "violet shared document memory").expect("fixture should write");
    let store = ConversationStore::initialize(path).expect("storage should initialize");
    let attachment = store
        .ingest_attachment(&source)
        .expect("attachment should ingest");
    process_pending_attachments(&store);
    let excluded = store
        .create_conversation("Excluded document scope")
        .expect("excluded conversation should create");
    let included = store
        .create_conversation("Included document scope")
        .expect("included conversation should create");
    store
        .add_conversation_attachments(&excluded.id, std::slice::from_ref(&attachment.id))
        .expect("excluded association should persist");
    store
        .add_conversation_attachments(&included.id, std::slice::from_ref(&attachment.id))
        .expect("included association should persist");
    store
        .set_conversation_memory_excluded(&excluded.id, true)
        .expect("conversation should be excluded");
    let mut embedder = FixtureEmbedder;
    index_all(&store, &mut embedder);

    let excluded_result = store
        .execute_search_attached_files(
            SearchAttachedFilesArguments {
                query: "violet".into(),
                conversation_id: Some(excluded.id),
                ..SearchAttachedFilesArguments::default()
            },
            &mut embedder,
        )
        .expect("excluded file search should succeed");
    let included_result = store
        .execute_search_attached_files(
            SearchAttachedFilesArguments {
                query: "violet".into(),
                conversation_id: Some(included.id),
                ..SearchAttachedFilesArguments::default()
            },
            &mut embedder,
        )
        .expect("included file search should succeed");
    let global_lexical = store
        .search_memory_lexically(
            "violet",
            MemoryLexicalFilters {
                source_kind: Some(MemorySourceKind::Attachment),
                ..MemoryLexicalFilters::default()
            },
        )
        .expect("global lexical search should succeed");

    assert!(excluded_result.matches.is_empty());
    assert_eq!(included_result.matches.len(), 1);
    assert_eq!(global_lexical.len(), 1);
}
