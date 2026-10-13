//! Opt-in native generation, persistence, and cancellation with existing local 2.1 weights.

use std::{fs, sync::Arc, time::Duration};
use tokio::sync::{Notify, watch};

use super::{ImageGenerationRequest, LocalImageGenerator, local::LocalImageGenerationError};
use crate::{
    local_image_worker::{
        availability_service::LocalImageAvailabilityService, server::IMAGE_PACKAGE_ID,
    },
    storage::{
        ConversationStore, GeneratedAssetExecution, GeneratedImageProvenance,
        GeneratedImageRequestOptions, MessageState, NewStoredMessage, StoredRole,
    },
};

#[tokio::test]
#[ignore = "requires the installed licensed Qwen-Image 2.1 pack on loopback MLX Serve port 11234"]
async fn live_installed_qwen_21_generates_persists_and_cancels_through_native_adapter() {
    let root = std::env::temp_dir().join(format!("bottie-qwen21-live-{}", uuid::Uuid::new_v4()));
    let store = ConversationStore::initialize(root.join("bottie.sqlite3")).unwrap();
    let availability = LocalImageAvailabilityService::new(&root).unwrap();
    assert_eq!(
        availability.inspect().await.unwrap().model_id,
        IMAGE_PACKAGE_ID
    );
    let installation = availability.inspect_for_execution().await.unwrap();
    assert_eq!(installation.model_id(), IMAGE_PACKAGE_ID);
    let generator = LocalImageGenerator::new(root.join("worker-output")).unwrap();
    let prompt = "A friendly violet glass robot tending a tiny greenhouse, botanical illustration";
    let (_keep_alive, cancellation) = watch::channel(false);
    let prepared = generator
        .generate(
            installation,
            ImageGenerationRequest::new_local(prompt, 512, 512, 1).unwrap(),
            42,
            store.clone(),
            cancellation,
            || {},
        )
        .await
        .unwrap_or_else(|_| panic!("local native generation failed"));
    assert_eq!(prepared.len(), 1);
    assert_eq!((prepared[0].width, prepared[0].height), (512, 512));

    let conversation = store.create_conversation("Local image proof").unwrap();
    let user = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: prompt.into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .unwrap();
    let provenance = GeneratedImageProvenance::new(
        "qwen-image",
        IMAGE_PACKAGE_ID,
        GeneratedAssetExecution::Local,
        Some(42),
    )
    .unwrap();
    let options = GeneratedImageRequestOptions::new(512, 512, false).unwrap();
    let pending = store
        .start_generated_image_message(&conversation.id, &user.id, prompt, 1, &provenance, &options)
        .unwrap();
    let completed = store
        .complete_generated_image_message(&pending.message.id, &prepared)
        .unwrap();
    assert_eq!(completed.generated_assets.len(), 1);
    drop(prepared);
    assert_eq!(
        fs::read_dir(store.generated_asset_temporary_directory())
            .unwrap()
            .count(),
        0
    );

    let installation = availability
        .inspect_for_model(IMAGE_PACKAGE_ID)
        .await
        .unwrap();
    let (cancel, cancellation) = watch::channel(false);
    let started = Arc::new(Notify::new());
    let progress = started.clone();
    let cancelled_store = store.clone();
    let run = tokio::spawn(async move {
        generator
            .generate(
                installation,
                ImageGenerationRequest::new_local("A simple blue circle", 512, 512, 1).unwrap(),
                43,
                cancelled_store,
                cancellation,
                move || progress.notify_one(),
            )
            .await
    });
    started.notified().await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    cancel.send(true).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), run)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(LocalImageGenerationError::Cancelled)));
    assert_eq!(
        fs::read_dir(store.generated_asset_temporary_directory())
            .unwrap()
            .count(),
        0
    );
    println!(
        "Verified native 512x512 generation, durable completion with exact local model provenance, and cancellation."
    );
}
