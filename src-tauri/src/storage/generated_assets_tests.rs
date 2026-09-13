//! Generated-image migration, ownership, persistence, and path-redaction tests.

use std::fs;

use image::{ImageFormat, Rgba, RgbaImage};
use tauri::http::{Method, Request, StatusCode, header};

use super::tests::test_database_path;
use super::*;

/// Creates one conversation and exact hosted-image provenance for storage tests.
fn generated_fixture() -> (
    ConversationStore,
    StoredConversation,
    StoredMessage,
    GeneratedImageProvenance,
) {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Generated landscape")
        .expect("conversation should be created");
    let provenance = GeneratedImageProvenance::new(
        "qwen-image",
        "qwen-image-2.0-2026-03-03",
        GeneratedAssetExecution::Cloud,
        None,
    )
    .expect("provenance should be valid");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "A generated landscape".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("durable image prompt should be appended");
    (store, conversation, request, provenance)
}

/// Writes one deterministic metadata-free PNG into the generated temporary directory.
fn prepared_png(store: &ConversationStore, width: u32, height: u32) -> PreparedGeneratedImage {
    let directory = store.generated_asset_temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should exist");
    let path = directory.join(format!("{}.png.part", uuid::Uuid::new_v4()));
    RgbaImage::from_pixel(width, height, Rgba([20, 40, 60, 255]))
        .save_with_format(&path, ImageFormat::Png)
        .expect("PNG fixture should be written");
    PreparedGeneratedImage::from_validated_png(path, width, height)
        .expect("prepared PNG should be accepted")
}

/// Returns one accepted square request contract for storage tests.
fn request_options() -> GeneratedImageRequestOptions {
    GeneratedImageRequestOptions::new(1_024, 1_024, true).expect("request options should be valid")
}

#[test]
fn persists_pending_assistant_assets_without_serializing_hashes_or_paths() {
    let (store, conversation, request, provenance) = generated_fixture();

    let message = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            2,
            &provenance,
            &request_options(),
        )
        .expect("pending image message should start");
    let message = message.message;
    let serialized = serde_json::to_string(&message).expect("message should serialize");

    assert_eq!(message.role, StoredRole::Assistant);
    assert_eq!(message.state, MessageState::Partial);
    assert_eq!(message.generated_assets.len(), 2);
    assert!(
        message
            .generated_assets
            .iter()
            .all(|asset| asset.status == GeneratedAssetStatus::Pending)
    );
    assert!(serialized.contains("qwen-image-2.0-2026-03-03"));
    assert!(!serialized.contains("sha256"));
    assert!(!serialized.contains("generated-assets"));
    assert!(!serialized.contains(".png"));
}

#[test]
fn commits_exact_pngs_and_reopens_path_free_provenance() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Generated landscape")
        .expect("conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "A generated landscape".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("durable image prompt should be appended");
    let provenance = GeneratedImageProvenance::new(
        "qwen-image",
        "qwen-image-2.0-2026-03-03",
        GeneratedAssetExecution::Cloud,
        Some(42),
    )
    .expect("provenance should be valid");
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &request_options(),
        )
        .expect("pending image message should start");
    let pending = pending.message;
    let prepared = prepared_png(&store, 32, 16);
    let retained_path = store
        .generated_asset_blob_path(&prepared.sha256)
        .expect("content path should resolve");

    let completed = store
        .complete_generated_image_message(&pending.id, &[prepared.clone()])
        .expect("generated image should complete");

    assert_eq!(completed.state, MessageState::Final);
    assert_eq!(
        completed.generated_assets[0].status,
        GeneratedAssetStatus::Completed
    );
    assert_eq!(completed.generated_assets[0].width, Some(32));
    assert_eq!(completed.generated_assets[0].height, Some(16));
    assert_eq!(completed.generated_assets[0].seed, Some(42));
    assert!(retained_path.is_file());
    assert!(!prepared.temporary_path.exists());
    let preview = store
        .load_generated_asset_preview(&completed.generated_assets[0].id)
        .expect("preview should load")
        .expect("completed image should have a preview");
    assert_eq!(preview.mime_type, "image/png");
    assert!(preview.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "bottie-generated-asset://localhost/{}",
            completed.generated_assets[0].id
        ))
        .body(Vec::new())
        .expect("preview request should build");
    let response = crate::generated_asset_preview_protocol::response(&store, &request);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(!response.body().is_empty());
    drop(store);

    let reopened = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let asset = &reopened.messages[1].generated_assets[0];
    assert_eq!(asset.status, GeneratedAssetStatus::Completed);
    assert_eq!(asset.media_type.as_deref(), Some("image/png"));
    assert_eq!(asset.provider_id, "qwen-image");
    assert_eq!(asset.model_id, "qwen-image-2.0-2026-03-03");
    assert_eq!(asset.execution, GeneratedAssetExecution::Cloud);
}

#[test]
fn generated_preview_protocol_rejects_non_get_and_non_opaque_requests() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let asset_id = uuid::Uuid::new_v4();
    for (method, uri, expected) in [
        (
            Method::POST,
            format!("bottie-generated-asset://localhost/{asset_id}"),
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        (
            Method::GET,
            format!("bottie-generated-asset://localhost/{asset_id}?path=/private/image.png"),
            StatusCode::NOT_FOUND,
        ),
        (
            Method::GET,
            "bottie-generated-asset://localhost/not-a-uuid".into(),
            StatusCode::NOT_FOUND,
        ),
        (
            Method::GET,
            format!("bottie-generated-asset://localhost/{asset_id}/extra"),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Vec::new())
            .expect("adversarial preview request should build");
        let response = crate::generated_asset_preview_protocol::response(&store, &request);
        assert_eq!(response.status(), expected);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert!(response.body().is_empty());
    }
}

#[test]
fn rejects_changed_staged_bytes_without_finalizing_or_retaining_them() {
    let (store, conversation, request, provenance) = generated_fixture();
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &request_options(),
        )
        .expect("pending image message should start");
    let pending = pending.message;
    let prepared = prepared_png(&store, 16, 16);
    fs::write(&prepared.temporary_path, b"changed after validation")
        .expect("fixture should be changed");

    let error = store
        .complete_generated_image_message(&pending.id, &[prepared])
        .expect_err("changed bytes should fail closed");
    let reopened = store
        .load_conversation(&conversation.id)
        .expect("conversation should remain readable");

    assert_eq!(error.code, "internal");
    assert_eq!(reopened.messages[1].state, MessageState::Partial);
    assert_eq!(
        reopened.messages[1].generated_assets[0].status,
        GeneratedAssetStatus::Pending
    );
}

#[test]
fn finalizes_cancelled_and_failed_assets_without_content_metadata() {
    for (status, error_code, expected_state) in [
        (
            GeneratedAssetStatus::Cancelled,
            None,
            MessageState::Cancelled,
        ),
        (
            GeneratedAssetStatus::Failed,
            Some("download_failed"),
            MessageState::Failed,
        ),
    ] {
        let (store, conversation, request, provenance) = generated_fixture();
        let pending = store
            .start_generated_image_message(
                &conversation.id,
                &request.id,
                &request.text,
                1,
                &provenance,
                &request_options(),
            )
            .expect("pending image message should start");
        let pending = pending.message;

        let terminal = store
            .fail_generated_image_message(&pending.id, status, error_code)
            .expect("terminal state should persist");
        let asset = &terminal.generated_assets[0];

        assert_eq!(terminal.state, expected_state);
        assert_eq!(asset.status, status);
        assert_eq!(asset.error_code.as_deref(), error_code);
        assert!(asset.sha256.is_none());
        assert!(asset.media_type.is_none());
        assert!(asset.byte_size.is_none());
    }
}

#[test]
fn rejects_invalid_counts_and_overlapping_generation() {
    let (store, conversation, request, provenance) = generated_fixture();

    assert!(
        store
            .start_generated_image_message(
                &conversation.id,
                "missing-request",
                "missing prompt",
                0,
                &provenance,
                &request_options(),
            )
            .is_err()
    );
    let mismatch = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            "A different prompt",
            1,
            &provenance,
            &request_options(),
        )
        .expect_err("WebView prompt substitution should fail");
    assert_eq!(mismatch.code, "invalid_request");
    store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &request_options(),
        )
        .expect("first image generation should start");
    let error = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &request_options(),
        )
        .expect_err("overlapping generation should fail");

    assert_eq!(error.code, "invalid_request");
}

#[test]
fn retries_failed_and_cancelled_images_from_the_exact_durable_request() {
    for terminal_status in [
        GeneratedAssetStatus::Failed,
        GeneratedAssetStatus::Cancelled,
    ] {
        let (store, conversation, request, provenance) = generated_fixture();
        let options = GeneratedImageRequestOptions::new(1_536, 1_024, true)
            .expect("request options should be valid");
        let pending = store
            .start_generated_image_message(
                &conversation.id,
                &request.id,
                &request.text,
                2,
                &provenance,
                &options,
            )
            .expect("image generation should start");
        let pending = pending.message;
        store
            .fail_generated_image_message(
                &pending.id,
                terminal_status,
                (terminal_status == GeneratedAssetStatus::Failed).then_some("provider_failed"),
            )
            .expect("terminal image state should persist");

        let retry = store
            .retry_generated_image_message(&pending.id)
            .expect("terminal image generation should retry");

        assert_eq!(retry.prompt, "A generated landscape");
        assert_eq!(retry.options, options);
        assert_eq!(retry.provenance, provenance);
        assert_eq!(retry.message.state, MessageState::Partial);
        assert_eq!(retry.message.generated_assets.len(), 2);
        assert_eq!(retry.message.generated_assets[0].ordinal, 0);
        assert_ne!(retry.message.id, pending.id);
    }
}

#[test]
fn retry_rejects_completed_active_and_non_selected_image_messages() {
    let (store, conversation, request, provenance) = generated_fixture();
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &request_options(),
        )
        .expect("generation should start");
    let pending = pending.message;

    assert!(store.retry_generated_image_message(&pending.id).is_err());
    let prepared = prepared_png(&store, 32, 16);
    let completed = store
        .complete_generated_image_message(&pending.id, &[prepared])
        .expect("generation should complete");
    assert!(store.retry_generated_image_message(&completed.id).is_err());

    let (store, conversation, request, provenance) = generated_fixture();
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            1,
            &provenance,
            &request_options(),
        )
        .expect("generation should start")
        .message;
    store
        .fail_generated_image_message(
            &pending.id,
            GeneratedAssetStatus::Failed,
            Some("provider_failed"),
        )
        .expect("failure should persist");
    store
        .fork_from_user_message(&conversation.id, &request.id, "A different portrait")
        .expect("alternative branch should become selected");
    assert!(store.retry_generated_image_message(&pending.id).is_err());
}
