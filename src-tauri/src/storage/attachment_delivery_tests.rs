//! Native provider-context image delivery policy tests.

use std::fs;

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

use super::{
    ConversationStore, MessageState, NewStoredMessage, ProviderImageFormat, StoredRole,
    tests::{completed_ingestion, test_database_path},
};

/// Encodes a tiny valid PNG whose normalized bytes can be delivered by tests.
fn png_fixture() -> Vec<u8> {
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&[20_u8, 40, 60, 255], 1, 1, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

/// Retains one fixture without waiting for background processing unless requested.
fn ingest_png(store: &ConversationStore, complete: bool) -> super::IngestedAttachment {
    let source_path = store.path.with_file_name("delivery.png");
    fs::write(&source_path, png_fixture()).expect("PNG fixture should be written");
    let ingested = store
        .ingest_attachment(&source_path)
        .expect("PNG fixture should ingest");
    if complete {
        completed_ingestion(store, ingested)
    } else {
        ingested
    }
}

/// Creates one durable user request with the supplied retained attachment.
fn attached_request(
    store: &ConversationStore,
    attachment: &super::IngestedAttachment,
) -> (String, String) {
    let conversation = store
        .create_conversation("Vision delivery")
        .expect("conversation should be created");
    let request = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Describe the normalized image".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[attachment.id.clone()],
        )
        .expect("request and attachment should commit");
    (conversation.id, request.id)
}

#[test]
fn loads_bounded_normalized_bytes_on_the_owning_durable_turn() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_png(&store, true);
    let (conversation_id, request_id) = attached_request(&store, &attachment);

    let metadata = store
        .provider_attachment_context(&conversation_id, &request_id)
        .expect("provider context should load");
    assert!(metadata.messages[0].images[0].bytes.is_none());
    let context = store
        .load_provider_images(metadata)
        .expect("normalized bytes should load after vision confirmation");

    assert!(context.current_request_has_image);
    assert_eq!(context.messages.len(), 1);
    assert_eq!(context.messages[0].text, "Describe the normalized image");
    assert_eq!(context.messages[0].images.len(), 1);
    assert_eq!(
        context.messages[0].images[0].format,
        ProviderImageFormat::Png
    );
    assert!(
        context.messages[0].images[0]
            .bytes
            .as_deref()
            .expect("vision context should contain bytes")
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
}

#[test]
fn rejects_a_current_image_until_native_normalization_finishes() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_png(&store, false);
    let (conversation_id, request_id) = attached_request(&store, &attachment);

    let error = store
        .provider_attachment_context(&conversation_id, &request_id)
        .expect_err("pending current image must be rejected");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        error.message,
        "Wait for image normalization to finish before sending."
    );
}
