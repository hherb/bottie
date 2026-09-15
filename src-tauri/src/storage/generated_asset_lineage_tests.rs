//! Generated-image edit-lineage migration, persistence, and ownership tests.

use std::fs;

use image::{ImageFormat, Rgba, RgbaImage};

use super::tests::{completed_ingestion, test_database_path};
use super::*;

mod lifecycle;

/// Writes and fully normalizes one retained PNG fixture.
fn image_attachment(store: &ConversationStore, name: &str, color: [u8; 4]) -> IngestedAttachment {
    let source = store.path.with_file_name(name);
    RgbaImage::from_pixel(8, 8, Rgba(color))
        .save_with_format(&source, ImageFormat::Png)
        .expect("source PNG should be written");
    let ingested = store
        .ingest_attachment(&source)
        .expect("source PNG should ingest");
    completed_ingestion(store, ingested)
}

/// Appends one final user request, optionally with ordered retained attachments.
fn user_request(
    store: &ConversationStore,
    conversation_id: &str,
    text: &str,
    attachments: &[String],
) -> StoredMessage {
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
            attachments,
        )
        .expect("user request should append")
}

/// Returns exact hosted Qwen-Image-2.0 provenance and accepted output options.
fn hosted_contract() -> (GeneratedImageProvenance, GeneratedImageRequestOptions) {
    (
        GeneratedImageProvenance::new(
            "qwen-image",
            "qwen-image-2.0-2026-03-03",
            GeneratedAssetExecution::Cloud,
            None,
        )
        .expect("hosted provenance should be valid"),
        GeneratedImageRequestOptions::new(1_024, 1_024, true)
            .expect("hosted options should be valid"),
    )
}

/// Completes one generated image and returns its path-free durable asset.
fn generated_source(
    store: &ConversationStore,
    conversation_id: &str,
    color: [u8; 4],
) -> StoredGeneratedAsset {
    let request = user_request(store, conversation_id, "Create a source image", &[]);
    let (provenance, options) = hosted_contract();
    let pending = store
        .start_generated_image_message(
            conversation_id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &options,
        )
        .expect("source generation should start")
        .message;
    let directory = store.generated_asset_temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should exist");
    let path = directory.join(format!("{}.png.part", uuid::Uuid::new_v4()));
    RgbaImage::from_pixel(8, 8, Rgba(color))
        .save_with_format(&path, ImageFormat::Png)
        .expect("generated PNG should be written");
    let prepared = PreparedGeneratedImage::from_validated_png(path, 8, 8)
        .expect("generated PNG should prepare");
    store
        .complete_generated_image_message(&pending.id, &[prepared])
        .expect("source generation should complete")
        .generated_assets
        .remove(0)
}

/// Starts one edit using an earlier generated image plus two current-turn attachments.
fn started_edit(
    store: &ConversationStore,
    conversation_id: &str,
) -> (
    StartedGeneratedImageEdit,
    StoredGeneratedAsset,
    [IngestedAttachment; 2],
) {
    let generated = generated_source(store, conversation_id, [10, 20, 30, 255]);
    let first = image_attachment(store, "first-source.png", [40, 50, 60, 255]);
    let second = image_attachment(store, "second-source.png", [70, 80, 90, 255]);
    let request = user_request(
        store,
        conversation_id,
        "Combine these sources",
        &[first.id.clone(), second.id.clone()],
    );
    let sources = [
        GeneratedImageSourceReference::GeneratedAsset(generated.id.clone()),
        GeneratedImageSourceReference::Attachment(second.id.clone()),
        GeneratedImageSourceReference::Attachment(first.id.clone()),
    ];
    let (provenance, options) = hosted_contract();
    let started = store
        .start_generated_image_edit_message(
            conversation_id,
            &request.id,
            &request.text,
            2,
            &provenance,
            &options,
            &sources,
        )
        .expect("edit generation should start");
    (started, generated, [first, second])
}

#[test]
fn upgrades_version_twenty_four_with_an_empty_lineage_table() {
    let path = test_database_path();
    let store =
        ConversationStore::initialize(path.clone()).expect("current store should initialize");
    let connection = store.open().expect("store should open");
    connection
        .execute_batch(
            "DROP TABLE generated_asset_sources;
             DELETE FROM schema_migrations WHERE version = 25;
             PRAGMA user_version = 24;",
        )
        .expect("fixture should become a version 24 store");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version 24 should upgrade");
    let connection = upgraded.open().expect("upgraded store should open");
    let source_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM generated_asset_sources", [], |row| {
            row.get(0)
        })
        .expect("lineage table should exist");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        25
    );
    assert_eq!(source_count, 0);
}

#[test]
fn persists_ordered_mixed_sources_and_reopens_without_hashes_or_paths() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("store should initialize");
    let conversation = store
        .create_conversation("Image edit lineage")
        .expect("conversation should create");
    let (started, generated, attachments) = started_edit(&store, &conversation.id);

    assert_eq!(started.sources.len(), 3);
    assert_eq!(started.message.generated_assets.len(), 2);
    for asset in &started.message.generated_assets {
        assert_eq!(asset.sources.len(), 3);
        assert_eq!(asset.sources[0].source_id, generated.id);
        assert_eq!(
            asset.sources[0].source_type,
            GeneratedImageSourceType::GeneratedAsset
        );
        assert_eq!(asset.sources[1].source_id, attachments[1].id);
        assert_eq!(
            asset.sources[1].source_type,
            GeneratedImageSourceType::Attachment
        );
        assert_eq!(asset.sources[2].source_id, attachments[0].id);
    }
    let serialized = serde_json::to_string(&started.message).expect("message should serialize");
    assert!(serialized.contains("generated_asset"));
    assert!(serialized.contains("attachment"));
    assert!(!serialized.contains("sha256"));
    assert!(!serialized.contains("normalized-images"));
    assert!(!serialized.contains("generated-assets"));
    drop(store);

    let reopened = ConversationStore::initialize(path)
        .expect("store should reopen")
        .load_conversation(&conversation.id)
        .expect("conversation should reopen");
    assert_eq!(
        reopened.messages.last().unwrap().generated_assets[0]
            .sources
            .len(),
        3
    );
}

#[test]
fn rejects_empty_excess_duplicate_malformed_and_unowned_sources_without_mutation() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let conversation = store
        .create_conversation("Invalid edit")
        .expect("conversation should create");
    let attachment = image_attachment(&store, "owned.png", [1, 2, 3, 255]);
    let other = store
        .create_conversation("Other")
        .expect("other conversation should create");
    let foreign = image_attachment(&store, "foreign.png", [4, 5, 6, 255]);
    let foreign_request = user_request(&store, &other.id, "Foreign request", &[foreign.id.clone()]);
    let request = user_request(
        &store,
        &conversation.id,
        "Edit this",
        &[attachment.id.clone()],
    );
    let (provenance, options) = hosted_contract();

    let cases = vec![
        Vec::new(),
        vec![GeneratedImageSourceReference::Attachment(attachment.id.clone()); 4],
        vec![
            GeneratedImageSourceReference::Attachment(attachment.id.clone()),
            GeneratedImageSourceReference::Attachment(attachment.id.clone()),
        ],
        vec![GeneratedImageSourceReference::Attachment(
            "not-a-uuid".into(),
        )],
        vec![GeneratedImageSourceReference::Attachment(
            foreign.id.clone(),
        )],
    ];
    for sources in cases {
        let error = store
            .start_generated_image_edit_message(
                &conversation.id,
                &request.id,
                &request.text,
                1,
                &provenance,
                &options,
                &sources,
            )
            .expect_err("invalid sources must fail closed");
        assert_eq!(error.code, "invalid_request");
    }
    let local_provenance = GeneratedImageProvenance::new(
        "qwen-image-local",
        "Qwen/Qwen-Image-2512",
        GeneratedAssetExecution::Local,
        Some(42),
    )
    .expect("local provenance should be structurally valid");
    let provenance_error = store
        .start_generated_image_edit_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &local_provenance,
            &options,
            &[GeneratedImageSourceReference::Attachment(
                attachment.id.clone(),
            )],
        )
        .expect_err("local 2512 must not be accepted as hosted editing");
    assert_eq!(provenance_error.code, "invalid_request");
    assert_eq!(
        store
            .load_conversation(&conversation.id)
            .expect("conversation should remain readable")
            .messages
            .len(),
        1
    );
    assert_eq!(foreign_request.text, "Foreign request");
}

#[test]
fn rejects_distinct_source_ids_that_resolve_to_duplicate_content() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let conversation = store
        .create_conversation("Duplicate source bytes")
        .expect("conversation should create");
    let first = generated_source(&store, &conversation.id, [9, 8, 7, 255]);
    let second = generated_source(&store, &conversation.id, [9, 8, 7, 255]);
    let request = user_request(&store, &conversation.id, "Combine distinct sources", &[]);
    let (provenance, options) = hosted_contract();

    let error = store
        .start_generated_image_edit_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &options,
            &[
                GeneratedImageSourceReference::GeneratedAsset(first.id),
                GeneratedImageSourceReference::GeneratedAsset(second.id),
            ],
        )
        .expect_err("duplicate exact bytes must not be sent twice");

    assert_eq!(error.code, "invalid_request");
}

#[test]
fn exact_retry_preserves_edit_sources_and_source_deletion_is_blocked() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let conversation = store
        .create_conversation("Retry edit")
        .expect("conversation should create");
    let (started, generated, _) = started_edit(&store, &conversation.id);
    let source_snapshot = started.message.generated_assets[0].sources.clone();
    store
        .fail_generated_image_message(
            &started.message.id,
            GeneratedAssetStatus::Failed,
            Some("provider_failed"),
        )
        .expect("edit should fail durably");

    let retry = store
        .retry_generated_image_message(&started.message.id)
        .expect("edit should retry exactly");
    assert_eq!(retry.message.generated_assets[0].sources, source_snapshot);
    let error = store
        .delete_generated_asset(&generated.id)
        .expect_err("a retained edit source must not be deleted");
    assert_eq!(error.code, "invalid_request");
}
