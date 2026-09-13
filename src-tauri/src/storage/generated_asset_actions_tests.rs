//! Generated-image open/export resolution and deletion ownership tests.

use std::fs;

use image::{ImageFormat, Rgba, RgbaImage};

use super::tests::test_database_path;
use super::*;

/// Creates one completed two-output image message whose bytes deduplicate to one blob.
fn completed_images() -> (ConversationStore, StoredConversation, StoredMessage) {
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let conversation = store
        .create_conversation("Generated images")
        .expect("conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Two quiet blue squares".into(),
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
        None,
    )
    .expect("provenance should be valid");
    let options =
        GeneratedImageRequestOptions::new(1_024, 1_024, true).expect("options should be valid");
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &request.id,
            &request.text,
            2,
            &provenance,
            &options,
        )
        .expect("image generation should start")
        .message;
    let images = [prepared_png(&store), prepared_png(&store)];
    let completed = store
        .complete_generated_image_message(&pending.id, &images)
        .expect("images should complete");
    (store, conversation, completed)
}

/// Writes one deterministic generated PNG staging fixture.
fn prepared_png(store: &ConversationStore) -> PreparedGeneratedImage {
    let directory = store.generated_asset_temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should exist");
    let path = directory.join(format!("{}.png.part", uuid::Uuid::new_v4()));
    RgbaImage::from_pixel(32, 16, Rgba([20, 40, 60, 255]))
        .save_with_format(&path, ImageFormat::Png)
        .expect("PNG fixture should be written");
    PreparedGeneratedImage::from_validated_png(path, 32, 16)
        .expect("prepared PNG should be accepted")
}

#[test]
fn resolves_only_completed_selected_assets_to_native_files() {
    let (store, _, completed) = completed_images();
    let asset = store
        .generated_asset_file(&completed.generated_assets[0].id)
        .expect("completed asset should resolve");

    assert!(asset.path.is_file());
    assert_eq!(asset.file_name, "bottie-generated-image-1.png");
    assert!(store.generated_asset_file("not-an-asset").is_err());
}

#[test]
fn deletes_outputs_and_removes_only_the_final_content_reference() {
    let (store, conversation, completed) = completed_images();
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Continue after the images".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("later lineage should persist");
    let blob = store
        .generated_asset_blob_path(
            completed.generated_assets[0]
                .sha256
                .as_deref()
                .expect("completed hash should exist natively"),
        )
        .expect("blob path should resolve");

    let after_first = store
        .delete_generated_asset(&completed.generated_assets[0].id)
        .expect("first output should be deleted");
    assert_eq!(after_first.messages[1].generated_assets.len(), 1);
    assert_eq!(after_first.messages[1].text, "Generated image.");
    assert!(blob.is_file());

    let after_last = store
        .delete_generated_asset(&completed.generated_assets[1].id)
        .expect("last output should be deleted");
    assert_eq!(after_last.id, conversation.id);
    assert_eq!(after_last.messages.len(), 2);
    assert_eq!(after_last.messages[0].role, StoredRole::User);
    assert_eq!(after_last.messages[1].text, "Continue after the images");
    assert!(!blob.exists());
}
