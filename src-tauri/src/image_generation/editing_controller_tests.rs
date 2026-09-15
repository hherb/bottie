//! Path-free hosted image-editing command request tests.

use super::controller::editing::StartImageEditingRequest;
use crate::storage::{GeneratedImageSourceReference, validate_generated_image_source_references};

const GENERATED_ASSET_ID: &str = "00000000-0000-4000-8000-000000000001";
const ATTACHMENT_ID: &str = "00000000-0000-4000-8000-000000000002";

#[test]
fn editing_command_accepts_only_ordered_opaque_source_references() {
    let request: StartImageEditingRequest = serde_json::from_value(serde_json::json!({
        "conversationId": "conversation-1",
        "requestMessageId": "message-1",
        "prompt": "Combine these",
        "width": 1024,
        "height": 1024,
        "count": 1,
        "sources": [
            {"sourceType": "generated_asset", "sourceId": GENERATED_ASSET_ID},
            {"sourceType": "attachment", "sourceId": ATTACHMENT_ID}
        ]
    }))
    .expect("closed editing request should decode");

    let references = request.source_references();
    validate_generated_image_source_references(&references)
        .expect("opaque source identities should preflight");
    assert_eq!(
        references,
        vec![
            GeneratedImageSourceReference::GeneratedAsset(GENERATED_ASSET_ID.into()),
            GeneratedImageSourceReference::Attachment(ATTACHMENT_ID.into()),
        ]
    );
}

#[test]
fn editing_command_rejects_paths_execution_overrides_and_unknown_source_types() {
    let base = serde_json::json!({
        "conversationId": "conversation-1",
        "requestMessageId": "message-1",
        "prompt": "Combine these",
        "width": 1024,
        "height": 1024,
        "count": 1,
        "sources": [{"sourceType": "attachment", "sourceId": ATTACHMENT_ID}]
    });
    for (field, value) in [
        ("path", serde_json::json!("/private/source.png")),
        ("execution", serde_json::json!("local")),
        ("apiKey", serde_json::json!("secret")),
    ] {
        let mut candidate = base.clone();
        candidate[field] = value;
        assert!(serde_json::from_value::<StartImageEditingRequest>(candidate).is_err());
    }
    let mut candidate = base;
    candidate["sources"][0]["sourceType"] = serde_json::json!("path");
    assert!(serde_json::from_value::<StartImageEditingRequest>(candidate).is_err());

    let malformed: StartImageEditingRequest = serde_json::from_value(serde_json::json!({
        "conversationId": "conversation-1",
        "requestMessageId": "message-1",
        "prompt": "Combine these",
        "width": 1024,
        "height": 1024,
        "count": 1,
        "sources": [{"sourceType": "attachment", "sourceId": "/private/source.png"}]
    }))
    .expect("opaque IDs remain strings at deserialization");
    assert!(validate_generated_image_source_references(&malformed.source_references()).is_err());
}
