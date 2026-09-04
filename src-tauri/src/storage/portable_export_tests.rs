//! Portable attachment bundle export contract tests.

use std::io::Read;

use zip::ZipArchive;

use super::*;

#[test]
fn bundles_selected_message_and_conversation_attachments_with_portable_metadata() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Portable context")
        .expect("conversation should be created");
    let conversation_attachment =
        ingest_export_fixture(&store, "shared notes.txt", b"conversation attachment");
    let message_attachment = ingest_export_fixture(&store, "request.md", b"# Request attachment");
    store
        .add_conversation_attachments(&conversation.id, &[conversation_attachment.id.clone()])
        .expect("conversation attachment should associate");
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Use the retained files".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[message_attachment.id.clone()],
        )
        .expect("message attachment should associate");

    let export = store
        .prepare_json_export(&conversation.id)
        .expect("portable JSON bundle should prepare");
    let path = store.path.with_file_name("portable-context.zip");
    export
        .write_to(&path)
        .expect("portable JSON bundle should write");

    let file = std::fs::File::open(path).expect("portable ZIP should open");
    let mut archive = ZipArchive::new(file).expect("portable ZIP should parse");
    let mut document = String::new();
    archive
        .by_name("bottie-portable-context.json")
        .expect("JSON document should be present")
        .read_to_string(&mut document)
        .expect("JSON document should be readable");
    let value: serde_json::Value = serde_json::from_str(&document).expect("JSON should parse");
    let conversation_file = value["attachments"][0]["file"]
        .as_str()
        .expect("conversation attachment file should be named")
        .to_owned();
    let message_file = value["messages"][0]["attachments"][0]["file"]
        .as_str()
        .expect("message attachment file should be named")
        .to_owned();
    let conversation_bytes = archive_bytes(&mut archive, &conversation_file);
    let message_bytes = archive_bytes(&mut archive, &message_file);

    assert_eq!(export.file_name, "bottie-portable-context.zip");
    assert_eq!(value["version"], 5);
    assert_eq!(value["attachments"][0]["displayName"], "shared notes.txt");
    assert_eq!(
        value["messages"][0]["attachments"][0]["displayName"],
        "request.md"
    );
    assert!(value["attachments"][0].get("id").is_none());
    assert_eq!(conversation_bytes, b"conversation attachment");
    assert_eq!(message_bytes, b"# Request attachment");
}

#[test]
fn writes_markdown_links_and_deduplicates_one_attachment_across_scopes() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Shared attachment")
        .expect("conversation should be created");
    let attachment = ingest_export_fixture(&store, "shared.md", b"# Shared attachment");
    store
        .add_conversation_attachments(&conversation.id, &[attachment.id.clone()])
        .expect("conversation attachment should associate");
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Use the same attachment".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[attachment.id.clone()],
        )
        .expect("message attachment should associate");

    let export = store
        .prepare_markdown_export(&conversation.id)
        .expect("portable Markdown bundle should prepare");
    let path = store.path.with_file_name("shared-attachment.zip");
    export
        .write_to(&path)
        .expect("portable Markdown bundle should write");
    let file = std::fs::File::open(path).expect("portable ZIP should open");
    let mut archive = ZipArchive::new(file).expect("portable ZIP should parse");
    let mut markdown = String::new();
    archive
        .by_name("bottie-shared-attachment.md")
        .expect("Markdown document should be present")
        .read_to_string(&mut markdown)
        .expect("Markdown document should be readable");

    assert_eq!(export.file_name, "bottie-shared-attachment.zip");
    assert_eq!(archive.len(), 2);
    assert!(markdown.contains("## Conversation attachments"));
    assert!(markdown.contains("## Attachments"));
    assert_eq!(markdown.matches(&attachment.sha256).count(), 4);
}

#[test]
fn bundles_referenced_files_with_the_non_trashed_batch_document() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Batch attachment")
        .expect("conversation should be created");
    let attachment = ingest_export_fixture(&store, "batch.txt", b"batch attachment bytes");
    store
        .add_conversation_attachments(&conversation.id, &[attachment.id.clone()])
        .expect("batch attachment should enter conversation scope");

    let export = store
        .prepare_batch_json_export()
        .expect("portable batch bundle should prepare");
    let path = store.path.with_file_name("batch-export.zip");
    export
        .write_to(&path)
        .expect("portable batch bundle should write");
    let file = std::fs::File::open(path).expect("portable ZIP should open");
    let mut archive = ZipArchive::new(file).expect("portable ZIP should parse");
    let mut document = String::new();
    archive
        .by_name("bottie-conversations.json")
        .expect("batch JSON document should be present")
        .read_to_string(&mut document)
        .expect("batch JSON document should be readable");
    let value: serde_json::Value =
        serde_json::from_str(&document).expect("batch JSON should parse");

    assert_eq!(export.file_name, "bottie-conversations.zip");
    assert_eq!(value["version"], 5);
    assert_eq!(
        value["conversations"][0]["attachments"][0]["displayName"],
        "batch.txt"
    );
    assert_eq!(archive.len(), 2);
}

/// Writes and completes one retained attachment for portable export coverage.
fn ingest_export_fixture(
    store: &ConversationStore,
    name: &str,
    bytes: &[u8],
) -> IngestedAttachment {
    let source = store.path.with_file_name(name);
    std::fs::write(&source, bytes).expect("attachment fixture should be written");
    let ingested = store
        .ingest_attachment(&source)
        .expect("attachment fixture should ingest");
    tests::completed_ingestion(store, ingested)
}

/// Reads one exact ZIP member into bytes for archive contract assertions.
fn archive_bytes(archive: &mut ZipArchive<std::fs::File>, name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("attachment ZIP member should exist")
        .read_to_end(&mut bytes)
        .expect("attachment ZIP member should be readable");
    bytes
}
