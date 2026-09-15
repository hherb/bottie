//! Portable attachment bundle export contract tests.

use std::io::Read;

use image::{ImageFormat, Rgba, RgbaImage};
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
    assert_eq!(value["version"], 7);
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
    assert_eq!(value["version"], 7);
    assert_eq!(
        value["conversations"][0]["attachments"][0]["displayName"],
        "batch.txt"
    );
    assert_eq!(archive.len(), 2);
}

#[test]
fn bundles_selected_generated_pngs_with_path_free_portable_metadata() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Generated export")
        .expect("conversation should be created");
    let completed = complete_generated_export_fixture(&store, &conversation, 2);

    let export = store
        .prepare_json_export(&conversation.id)
        .expect("generated-image JSON bundle should prepare");
    let path = store.path.with_file_name("generated-export.zip");
    export
        .write_to(&path)
        .expect("generated-image JSON bundle should write");
    let file = std::fs::File::open(path).expect("portable ZIP should open");
    let mut archive = ZipArchive::new(file).expect("portable ZIP should parse");
    let mut document = String::new();
    archive
        .by_name("bottie-generated-export.json")
        .expect("JSON document should be present")
        .read_to_string(&mut document)
        .expect("JSON document should be readable");
    let value: serde_json::Value = serde_json::from_str(&document).expect("JSON should parse");
    let assets = value["messages"][1]["generatedAssets"]
        .as_array()
        .expect("generated assets should be portable");

    assert_eq!(value["version"], 7);
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0]["providerId"], "qwen-image");
    assert_eq!(assets[0]["modelId"], "qwen-image-2.0-2026-03-03");
    assert_eq!(assets[0]["execution"], "cloud");
    assert_eq!(assets[0]["width"], 32);
    assert_eq!(assets[0]["height"], 16);
    assert!(assets[0].get("id").is_none());
    assert_eq!(assets[0]["file"], assets[1]["file"]);
    let archive_path = assets[0]["file"]
        .as_str()
        .expect("generated file should be named");
    assert!(archive_path.starts_with("generated-images/"));
    assert!(archive_bytes(&mut archive, archive_path).starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(archive.len(), 2);
    assert_eq!(
        completed.generated_assets[0].sha256,
        completed.generated_assets[1].sha256
    );
}

#[test]
fn excludes_generated_pngs_owned_only_by_a_superseded_branch() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Generated branch export")
        .expect("conversation should be created");
    complete_generated_export_fixture(&store, &conversation, 1);
    let original_request = store
        .load_conversation(&conversation.id)
        .expect("conversation should load")
        .messages[0]
        .id
        .clone();
    store
        .fork_from_user_message(&conversation.id, &original_request, "A different square")
        .expect("alternative branch should become selected");

    let export = store
        .prepare_json_export(&conversation.id)
        .expect("selected branch should export");
    let value: serde_json::Value =
        serde_json::from_str(&export.contents).expect("plain selected JSON should parse");

    assert!(!export.is_bundle());
    assert_eq!(
        value["messages"]
            .as_array()
            .expect("messages should exist")
            .len(),
        1
    );
    assert_eq!(value["messages"][0]["text"], "A different square");
}

#[test]
fn rejects_changed_generated_png_bytes_without_leaving_a_partial_export() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Changed generated export")
        .expect("conversation should be created");
    let completed = complete_generated_export_fixture(&store, &conversation, 1);
    let sha256 = completed.generated_assets[0]
        .sha256
        .as_deref()
        .expect("completed hash should exist");
    let export = store
        .prepare_json_export(&conversation.id)
        .expect("generated-image export should prepare");
    let blob = store
        .generated_asset_blob_path(sha256)
        .expect("generated blob should resolve");
    std::fs::write(&blob, b"changed after export preparation")
        .expect("generated blob should be changed");
    let destination = store.path.with_file_name("changed-generated-export.zip");

    let error = export
        .write_to(&destination)
        .expect_err("changed generated bytes must fail closed");

    assert_eq!(error.code, "internal");
    assert!(!destination.exists());
}

#[test]
fn rejects_missing_generated_png_without_leaving_a_partial_export() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Missing generated export")
        .expect("conversation should be created");
    let completed = complete_generated_export_fixture(&store, &conversation, 1);
    let sha256 = completed.generated_assets[0]
        .sha256
        .as_deref()
        .expect("completed hash should exist");
    let export = store
        .prepare_json_export(&conversation.id)
        .expect("generated-image export should prepare");
    let blob = store
        .generated_asset_blob_path(sha256)
        .expect("generated blob should resolve");
    std::fs::remove_file(blob).expect("generated blob should be removed");
    let destination = store.path.with_file_name("missing-generated-export.zip");

    let error = export
        .write_to(&destination)
        .expect_err("missing generated bytes must fail closed");

    assert_eq!(error.code, "internal");
    assert!(!destination.exists());
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

/// Creates one selected completed generated-image message for portable export coverage.
fn complete_generated_export_fixture(
    store: &ConversationStore,
    conversation: &StoredConversation,
    output_count: u8,
) -> StoredMessage {
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "A quiet blue square".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("prompt should persist");
    let provenance = GeneratedImageProvenance::new(
        "qwen-image",
        "qwen-image-2.0-2026-03-03",
        GeneratedAssetExecution::Cloud,
        Some(42),
    )
    .expect("provenance should be valid");
    let options =
        GeneratedImageRequestOptions::new(1_024, 1_024, true).expect("options should be valid");
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            output_count,
            &provenance,
            &options,
        )
        .expect("image generation should start")
        .message;
    let images = (0..output_count)
        .map(|_| {
            let directory = store.generated_asset_temporary_directory();
            std::fs::create_dir_all(&directory).expect("temporary directory should exist");
            let path = directory.join(format!("{}.png.part", uuid::Uuid::new_v4()));
            RgbaImage::from_pixel(32, 16, Rgba([20, 40, 60, 255]))
                .save_with_format(&path, ImageFormat::Png)
                .expect("PNG fixture should be written");
            PreparedGeneratedImage::from_validated_png(path, 32, 16)
                .expect("prepared PNG should be accepted")
        })
        .collect::<Vec<_>>();
    store
        .complete_generated_image_message(&pending.id, &images)
        .expect("generated image should complete")
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
