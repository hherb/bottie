//! Native `search_attached_files` tool-contract tests.

use std::{fs, path::Path};

use serde_json::json;

use super::{
    AttachmentExtractionFormat, ConversationStore, MessageState, NewStoredMessage, StoredRole,
    memory_file_tool::{
        MAX_SEARCH_ATTACHED_FILE_EXCERPT_CHARACTERS, MAX_SEARCH_ATTACHED_FILE_RESULTS,
        SEARCH_ATTACHED_FILES_TOOL_NAME, SearchAttachedFilesArguments,
    },
    memory_semantic::{EMBEDDING_DIMENSIONS, SemanticEmbedder},
    tests::{process_pending_attachments, test_database_path},
};

/// Deterministic embedder that records whether invalid requests reached model work.
#[derive(Default)]
struct FileToolEmbedder {
    inputs: Vec<String>,
}

impl SemanticEmbedder for FileToolEmbedder {
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

/// Ingests and completes one extracted-text fixture attachment.
fn ready_attachment(store: &ConversationStore, path: &Path, name: &str, text: &str) -> String {
    let source = path.with_file_name(name);
    fs::write(&source, text).expect("attachment fixture should write");
    let attachment_id = store
        .ingest_attachment(&source)
        .expect("attachment fixture should ingest")
        .id;
    process_pending_attachments(store);
    attachment_id
}

/// Associates one fixture attachment with one final user request.
fn associate_attachment(store: &ConversationStore, conversation_id: &str, attachment_id: &str) {
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation_id.into(),
                role: StoredRole::User,
                text: "Retain the attached field notes.".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[attachment_id.into()],
        )
        .expect("attachment fixture should associate");
}

/// Drains every deterministic chunk through the fixture embedder.
fn index_all(store: &ConversationStore, embedder: &mut FileToolEmbedder) {
    while store
        .process_next_semantic_batch(embedder, 8)
        .expect("file-tool fixture batch should succeed")
        .is_some()
    {}
}

#[test]
fn returns_ranked_path_free_attachment_provenance() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Northern field research")
        .expect("conversation should create");
    let attachment_id = ready_attachment(
        &store,
        &path,
        "field-notes.md",
        "# Survey\nKeep the north boundary inside the Rust core.",
    );
    associate_attachment(&store, &conversation.id, &attachment_id);
    let outside = store
        .create_conversation("Outside scope")
        .expect("outside conversation should create");
    let outside_id = ready_attachment(
        &store,
        &path,
        "outside.txt",
        "The north boundary appears in unrelated notes.",
    );
    associate_attachment(&store, &outside.id, &outside_id);
    ready_attachment(
        &store,
        &path,
        "unassociated.txt",
        "The north boundary appears in an unassociated draft.",
    );
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::Assistant,
            text: "The north boundary also appears in a message.".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("message fixture should append");
    let mut embedder = FileToolEmbedder::default();
    index_all(&store, &mut embedder);

    let result = store
        .execute_search_attached_files(
            SearchAttachedFilesArguments {
                query: "north boundary".into(),
                conversation_id: Some(conversation.id),
                created_after_ms: Some(0),
                created_before_ms: Some(i64::MAX),
                limit: Some(5),
            },
            &mut embedder,
        )
        .expect("search_attached_files should succeed");

    assert_eq!(SEARCH_ATTACHED_FILES_TOOL_NAME, "search_attached_files");
    assert_eq!(result.matches.len(), 1);
    let matched = &result.matches[0];
    assert_eq!(matched.rank, 1);
    assert_eq!(
        matched.excerpt,
        "# Survey\nKeep the north boundary inside the Rust core."
    );
    assert_eq!(matched.provenance.source_kind, "attachment");
    assert_eq!(matched.provenance.attachment_id, attachment_id);
    assert_eq!(matched.provenance.display_name, "field-notes.md");
    assert_eq!(matched.provenance.mime_type, "text/plain");
    assert_eq!(
        matched.provenance.extraction_format,
        AttachmentExtractionFormat::Markdown
    );
    assert_eq!(
        matched.provenance.character_count,
        matched.excerpt.chars().count() as u64
    );
    assert_eq!(matched.provenance.page_count, None);
    assert!(matched.provenance.byte_size > 0);
    assert!(matched.provenance.created_at_ms > 0);
    assert_eq!(
        matched.provenance.chunk.as_ref().map(|chunk| chunk.ordinal),
        Some(0)
    );

    let serialized = serde_json::to_value(&result).expect("tool result should serialize");
    assert_eq!(
        serialized["matches"][0]["provenance"]["sourceKind"],
        json!("attachment")
    );
    assert_eq!(
        serialized["matches"][0]["provenance"]["extractionFormat"],
        json!("markdown")
    );
    assert!(serialized["matches"][0].get("score").is_none());
    assert!(serialized["matches"][0].get("lexicalRank").is_none());
    assert!(serialized["matches"][0].get("semanticRank").is_none());
    let serialized_text = serialized.to_string();
    for forbidden in [
        "sha256",
        "databasePath",
        "filePath",
        "textContent",
        "embedding",
        "distance",
        "conversationId",
        "messageId",
    ] {
        assert!(!serialized_text.contains(forbidden));
    }
}

#[test]
fn caps_results_and_excerpt_text_at_tool_specific_limits() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let first = store
        .create_conversation("Bounded files one")
        .expect("first conversation should create");
    let second = store
        .create_conversation("Bounded files two")
        .expect("second conversation should create");
    for index in 0..(MAX_SEARCH_ATTACHED_FILE_RESULTS + 3) {
        let attachment_id = ready_attachment(
            &store,
            &path,
            &format!("bounded-{index}.txt"),
            &format!("north file {index} {}", "x".repeat(1_400)),
        );
        let conversation_id = if index < 8 { &first.id } else { &second.id };
        associate_attachment(&store, conversation_id, &attachment_id);
    }
    let mut embedder = FileToolEmbedder::default();
    index_all(&store, &mut embedder);

    let result = store
        .execute_search_attached_files(
            SearchAttachedFilesArguments {
                query: "north file".into(),
                limit: Some(usize::MAX),
                ..SearchAttachedFilesArguments::default()
            },
            &mut embedder,
        )
        .expect("bounded file search should succeed");

    assert_eq!(result.matches.len(), MAX_SEARCH_ATTACHED_FILE_RESULTS);
    assert!(result.matches.iter().all(|matched| {
        matched.excerpt.chars().count() <= MAX_SEARCH_ATTACHED_FILE_EXCERPT_CHARACTERS
    }));
}

#[test]
fn rejects_invalid_arguments_before_embedding_and_denies_unknown_fields() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let mut embedder = FileToolEmbedder::default();
    let invalid_requests = [
        SearchAttachedFilesArguments {
            query: "x".repeat(201),
            ..SearchAttachedFilesArguments::default()
        },
        SearchAttachedFilesArguments {
            query: "north".into(),
            conversation_id: Some("  ".into()),
            ..SearchAttachedFilesArguments::default()
        },
        SearchAttachedFilesArguments {
            query: "north".into(),
            created_after_ms: Some(2),
            created_before_ms: Some(1),
            ..SearchAttachedFilesArguments::default()
        },
        SearchAttachedFilesArguments {
            query: "north".into(),
            limit: Some(0),
            ..SearchAttachedFilesArguments::default()
        },
    ];

    for arguments in invalid_requests {
        assert_eq!(
            store
                .execute_search_attached_files(arguments, &mut embedder)
                .expect_err("invalid tool arguments should fail")
                .code,
            "invalid_request"
        );
    }
    assert!(embedder.inputs.is_empty());

    let unknown = serde_json::from_value::<SearchAttachedFilesArguments>(json!({
        "query": "north",
        "includePaths": true
    }));
    assert!(unknown.is_err());
}

#[test]
fn empty_query_returns_no_matches_without_embedding() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let mut embedder = FileToolEmbedder::default();
    let result = store
        .execute_search_attached_files(SearchAttachedFilesArguments::default(), &mut embedder)
        .expect("empty query should return an empty result");

    assert!(result.matches.is_empty());
    assert!(embedder.inputs.is_empty());
}

#[test]
fn keeps_archived_files_excludes_trash_and_omits_chunk_for_lexical_fallback() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let archived = store
        .create_conversation("Archived field notes")
        .expect("archived conversation should create");
    let archived_id = ready_attachment(&store, &path, "archived.txt", "violet archive file phrase");
    associate_attachment(&store, &archived.id, &archived_id);
    store
        .set_conversation_archived(&archived.id, true)
        .expect("conversation should archive");
    let trashed = store
        .create_conversation("Trashed field notes")
        .expect("trashed conversation should create");
    let trashed_id = ready_attachment(&store, &path, "trashed.txt", "violet trash file phrase");
    associate_attachment(&store, &trashed.id, &trashed_id);
    store
        .delete_conversation(&trashed.id)
        .expect("conversation should move to trash");
    let mut embedder = FileToolEmbedder::default();

    let result = store
        .execute_search_attached_files(
            SearchAttachedFilesArguments {
                query: "violet phrase".into(),
                ..SearchAttachedFilesArguments::default()
            },
            &mut embedder,
        )
        .expect("lexical fallback should succeed without indexed vectors");

    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].provenance.attachment_id, archived_id);
    assert!(result.matches[0].provenance.chunk.is_none());
    assert_eq!(embedder.inputs.len(), 1);
    let serialized = serde_json::to_value(&result).expect("fallback result should serialize");
    assert!(
        serialized["matches"][0]["provenance"]
            .get("chunk")
            .is_none()
    );
}
