//! Hosted Qwen-Image-2.0 provider-neutral editing and wire-shape tests.

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::json;

use super::{DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, ImageEditingRequest};
use crate::storage::{
    GeneratedAssetExecution, GeneratedImageSourceFormat, GeneratedImageSourceReference,
    ValidatedGeneratedImageSource,
};

const EDIT_SOURCE_MAX_BYTES: usize = 10 * 1_024 * 1_024;

/// Encodes a tiny valid PNG for native editing-source fixtures.
fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![127_u8; width as usize * height as usize * 4];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

/// Creates one native validated source with an opaque non-path identity.
fn source(format: GeneratedImageSourceFormat, bytes: Vec<u8>) -> ValidatedGeneratedImageSource {
    ValidatedGeneratedImageSource::for_test(
        GeneratedImageSourceReference::Attachment(uuid::Uuid::new_v4().to_string()),
        format,
        8,
        8,
        bytes,
    )
    .expect("validated source should build")
}

#[test]
fn builds_one_provider_neutral_edit_request_from_exact_hosted_lineage() {
    let request = ImageEditingRequest::new(
        "  Turn this into a Bottie poster  ",
        1_024,
        1_024,
        1,
        vec![source(GeneratedImageSourceFormat::Png, png_fixture(8, 8))],
    )
    .expect("hosted edit request should be valid");

    assert_eq!(request.prompt(), "Turn this into a Bottie poster");
    assert_eq!(request.dimensions(), (1_024, 1_024));
    assert_eq!(request.count(), 1);
    assert_eq!(request.sources().len(), 1);
    assert_eq!(
        request.sources()[0].format(),
        GeneratedImageSourceFormat::Png
    );
    assert_eq!(request.sources()[0].dimensions(), (8, 8));
    assert!(
        request.sources()[0]
            .bytes()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    assert_eq!(request.provenance().provider_id, "qwen-image");
    assert_eq!(request.provenance().model_id, DASHSCOPE_QWEN_IMAGE_MODEL_ID);
    assert_eq!(
        request.provenance().execution,
        GeneratedAssetExecution::Cloud
    );
    assert_eq!(request.provenance().seed, None);
}

#[test]
fn edit_request_requires_one_to_three_bounded_validated_native_sources() {
    let new_source = || source(GeneratedImageSourceFormat::Png, png_fixture(8, 8));

    assert!(ImageEditingRequest::new("Edit", 1_024, 1_024, 1, Vec::new()).is_err());
    assert!(
        ImageEditingRequest::new(
            "Edit",
            1_024,
            1_024,
            1,
            vec![new_source(), new_source(), new_source(), new_source()],
        )
        .is_err()
    );
    assert!(
        ValidatedGeneratedImageSource::for_test(
            GeneratedImageSourceReference::Attachment(uuid::Uuid::new_v4().to_string()),
            GeneratedImageSourceFormat::Png,
            8,
            8,
            vec![0; EDIT_SOURCE_MAX_BYTES + 1],
        )
        .is_err()
    );
}

#[test]
fn serializes_ordered_native_edit_bytes_as_closed_data_uris_followed_by_text() {
    let request = ImageEditingRequest::new(
        "Use image 1's shape and image 2's colors",
        1_024,
        1_536,
        2,
        vec![
            source(GeneratedImageSourceFormat::Png, png_fixture(8, 8)),
            source(
                GeneratedImageSourceFormat::Jpeg,
                vec![0xff, 0xd8, 0xff, 0xd9],
            ),
        ],
    )
    .expect("edit request should be valid");
    let provider = DashScopeQwenImageProvider::for_fixture(
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/",
        "test-only-key",
    )
    .expect("fixture provider should build");

    let value = provider
        .fixture_edit_request_json(&request)
        .expect("edit request should serialize");
    let content = value["input"]["messages"][0]["content"]
        .as_array()
        .expect("content should be an array");

    assert_eq!(value["model"], DASHSCOPE_QWEN_IMAGE_MODEL_ID);
    assert_eq!(content.len(), 3);
    assert!(
        content[0]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(
        content[1]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,")
    );
    assert_eq!(
        content[2],
        json!({"text": "Use image 1's shape and image 2's colors"})
    );
    assert_eq!(value["parameters"]["size"], "1024*1536");
    assert_eq!(value["parameters"]["n"], 2);
    assert_eq!(value["parameters"]["prompt_extend"], true);
    assert_eq!(value["parameters"]["watermark"], false);
    let serialized = value.to_string();
    assert!(!serialized.contains("sourceId"));
    assert!(!serialized.contains("sha256"));
    assert!(!serialized.contains("generated-assets"));
}
