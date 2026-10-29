//! Opt-in real MOSS MLX generation, durable provenance, active cancellation, and fresh-worker recovery.
use super::{ImageGenerationRequest, LocalImageGenerator, local::LocalImageGenerationError};
use crate::{
    local_image_worker::availability_service::LocalImageAvailabilityService,
    storage::{
        ConversationStore, GeneratedAssetExecution, GeneratedImageProvenance,
        GeneratedImageRequestOptions, MessageState, NewStoredMessage, StoredRole,
    },
};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};
use tokio::sync::watch;

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires explicitly installed MOSS MLX runtime/model and performs real Metal image generation"]
async fn live_moss_layout_persists_cancels_and_recovers() {
    let installed = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/com.bottie.app/moss-image/runtime.json");
    let root = std::env::temp_dir().join(format!("bottie-moss-live-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("moss-image")).unwrap();
    fs::copy(installed, root.join("moss-image/runtime.json")).unwrap();
    let store = ConversationStore::initialize(root.join("bottie.sqlite3")).unwrap();
    let availability = LocalImageAvailabilityService::new(&root).unwrap();
    let metadata = availability.inspect().await.unwrap();
    let model = metadata.model_id.clone();
    assert!(crate::local_image_worker::moss::is_moss_model(&model));
    let installation = availability.inspect_for_model(&model).await.unwrap();
    let exact_model = installation.model_id().to_owned();
    let generator = LocalImageGenerator::new(root.join("output")).unwrap();
    let prompt = r#"{"summary":{"meta":["poster"],"canvas":"single","caption":"A minimalist cream poster. The uppercase navy title BOTTIE is at the top, a solid blue circle is centered, and the uppercase navy words LOCAL IMAGE are at the bottom. Flat vector style, clean empty margins."},"items":[{"label":"text","caption":"Large bold navy uppercase title centered near the top.","text":"BOTTIE","bbox":[-448,-384,-320,384]},{"label":"object","caption":"A solid sky-blue circle in the exact center.","bbox":[-160,-160,160,160]},{"label":"text","caption":"Navy uppercase subtitle centered near the bottom.","text":"LOCAL IMAGE","bbox":[320,-384,416,384]}],"bbox_coord_space":"abs_centralized","bbox_format":"yxyx"}"#;
    let (_keep, receiver) = watch::channel(false);
    let started = Instant::now();
    let prepared = generator
        .generate(
            installation,
            ImageGenerationRequest::new_local_for_model(prompt, 1024, 1024, 1, &model).unwrap(),
            42,
            store.clone(),
            receiver,
            || {},
        )
        .await
        .unwrap_or_else(|_| panic!("MOSS live generation failed"));
    assert_eq!((prepared[0].width, prepared[0].height), (1024, 1024));
    let proof = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/moss-image");
    fs::create_dir_all(&proof).unwrap();
    fs::copy(&prepared[0].temporary_path, proof.join("layout-proof.png")).unwrap();
    let seconds = started.elapsed().as_secs_f64();
    let conversation = store
        .create_conversation("MOSS synthetic layout proof")
        .unwrap();
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
        "moss-image",
        &exact_model,
        GeneratedAssetExecution::Local,
        Some(42),
    )
    .unwrap();
    let pending = store
        .start_generated_image_message(
            &conversation.id,
            &user.id,
            prompt,
            1,
            &provenance,
            &GeneratedImageRequestOptions::new(1024, 1024, false).unwrap(),
        )
        .unwrap();
    let completed = store
        .complete_generated_image_message(&pending.message.id, &prepared)
        .unwrap();
    assert_eq!(completed.generated_assets[0].provider_id, "moss-image");
    assert_eq!(completed.generated_assets[0].model_id, exact_model);
    let retry = store
        .inspect_generated_image_retry(&completed.id)
        .unwrap_err();
    let _ = retry; // A completed response is not a failed/cancelled retry candidate.
    drop(prepared);
    let (cancel, receiver) = watch::channel(false);
    let count = Arc::new(AtomicUsize::new(0));
    let callbacks = count.clone();
    let cancelled = generator
        .generate(
            availability.inspect_for_model(&model).await.unwrap(),
            ImageGenerationRequest::new_local_for_model(
                "A blue circle on white",
                512,
                512,
                1,
                &model,
            )
            .unwrap(),
            43,
            store.clone(),
            receiver,
            move || {
                if callbacks.fetch_add(1, Ordering::SeqCst) >= 2 {
                    let _ = cancel.send(true);
                }
            },
        )
        .await;
    assert!(
        count.load(Ordering::SeqCst) >= 3,
        "cancel only after real denoising callbacks"
    );
    assert!(matches!(
        cancelled,
        Err(LocalImageGenerationError::Cancelled)
    ));
    let (_keep, receiver) = watch::channel(false);
    let recovered = generator
        .generate(
            availability.inspect_for_model(&model).await.unwrap(),
            ImageGenerationRequest::new_local_for_model(
                "A simple blue circle on white",
                512,
                512,
                1,
                &model,
            )
            .unwrap(),
            44,
            store.clone(),
            receiver,
            || {},
        )
        .await
        .unwrap_or_else(|_| panic!("MOSS restart recovery failed"));
    drop(recovered);
    assert_eq!(
        fs::read_dir(store.generated_asset_temporary_directory())
            .unwrap()
            .count(),
        0
    );
    fs::write(proof.join("native-proof.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "modelId":exact_model,"packageId":model,"revision":metadata.source_revision,"seed":42,"dimensions":[1024,1024],
        "generationAndVerificationSeconds":seconds,"durableProvenance":true,"activeCancellation":true,
        "freshWorkerRecovery":true,"nativeUiClicks":false,"pixelExactClaims":false})).unwrap()).unwrap();
    drop(store);
    fs::remove_dir_all(root).unwrap();
    println!(
        "MOSS native layout generation, durable provenance, active cancellation and recovery passed in {seconds:.2}s (first image)."
    );
}
