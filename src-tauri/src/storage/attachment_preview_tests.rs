//! Attachment-preview storage and protocol policy tests.

use std::fs;

use image::{
    ExtendedColorType, GenericImageView, ImageEncoder, ImageReader,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use tauri::http::{Method, Request, StatusCode, header};

use super::{ConversationStore, tests::test_database_path};

const PREVIEW_MAX_AXIS: u32 = 320;

/// Encodes a deliberately larger source image so preview resizing is observable.
fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![127_u8; width as usize * height as usize * 4];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

/// Encodes one JPEG fixture for trusted media-type and re-encoding coverage.
fn jpeg_fixture(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![127_u8; width as usize * height as usize * 3];
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 90)
        .write_image(&pixels, width, height, ExtendedColorType::Rgb8)
        .expect("JPEG fixture should encode");
    encoded
}

/// Ingests a named byte fixture beside the isolated test database.
fn ingest_fixture(
    store: &ConversationStore,
    name: &str,
    bytes: &[u8],
) -> super::IngestedAttachment {
    let path = store.path.with_file_name(name);
    fs::write(&path, bytes).expect("attachment fixture should be written");
    store
        .ingest_attachment(&path)
        .expect("fixture should ingest")
}

#[test]
fn reencodes_only_ready_images_into_bounded_metadata_free_previews() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let image = ingest_fixture(&store, "large.png", &png_fixture(640, 480));
    let picker_json = serde_json::to_string(&image).expect("picker metadata should serialize");
    assert!(!picker_json.contains(&image.sha256));

    assert!(
        store
            .load_attachment_preview(&image.id)
            .expect("pending preview lookup should succeed")
            .is_none()
    );
    super::tests::process_pending_attachments(&store);
    let stored = store
        .stored_attachment_for_test(&image.id)
        .expect("stored metadata should load")
        .expect("stored attachment should exist");
    let reopened_json = serde_json::to_string(&stored).expect("reopened metadata should serialize");
    assert!(!reopened_json.contains(&image.sha256));

    let preview = store
        .load_attachment_preview(&image.id)
        .expect("ready preview lookup should succeed")
        .expect("ready image should have a preview");
    assert_eq!(preview.mime_type, "image/png");
    assert!(preview.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = ImageReader::new(std::io::Cursor::new(&preview.bytes))
        .with_guessed_format()
        .expect("preview format should be detected")
        .decode()
        .expect("preview should decode");
    let (width, height) = decoded.dimensions();
    assert_eq!((width, height), (PREVIEW_MAX_AXIS, 240));

    let document = ingest_fixture(&store, "notes.txt", b"local-only text");
    super::tests::process_pending_attachments(&store);
    assert!(
        store
            .load_attachment_preview(&document.id)
            .expect("document preview lookup should succeed")
            .is_none()
    );
    assert!(
        store
            .load_attachment_preview("missing")
            .expect("missing preview lookup should succeed")
            .is_none()
    );
}

#[test]
fn protocol_serves_get_only_with_narrow_response_headers() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let image = ingest_fixture(&store, "preview.png", &png_fixture(4, 3));
    super::tests::process_pending_attachments(&store);

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("bottie-attachment://localhost/{}", image.id))
        .body(Vec::new())
        .expect("request should build");
    let response = crate::attachment_preview_protocol::response(&store, &request);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(!response.body().is_empty());

    let post = Request::builder()
        .method(Method::POST)
        .uri(format!("bottie-attachment://localhost/{}", image.id))
        .body(Vec::new())
        .expect("request should build");
    assert_eq!(
        crate::attachment_preview_protocol::response(&store, &post).status(),
        StatusCode::METHOD_NOT_ALLOWED
    );

    let nested = Request::builder()
        .method(Method::GET)
        .uri(format!("bottie-attachment://localhost/{}/extra", image.id))
        .body(Vec::new())
        .expect("request should build");
    assert_eq!(
        crate::attachment_preview_protocol::response(&store, &nested).status(),
        StatusCode::NOT_FOUND
    );

    for uri in [
        format!(
            "bottie-attachment://localhost/{}?path=/Users/alice/private.png",
            image.id
        ),
        "bottie-attachment://localhost/not-a-uuid".into(),
        "bottie-attachment://localhost/%2e%2e/private.png".into(),
    ] {
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Vec::new())
            .expect("adversarial request should build");
        let response = crate::attachment_preview_protocol::response(&store, &request);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert!(response.body().is_empty());
    }
}

#[test]
fn preserves_trusted_jpeg_format_while_reencoding_preview_pixels() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let image = ingest_fixture(&store, "preview.jpg", &jpeg_fixture(400, 200));
    super::tests::process_pending_attachments(&store);

    let preview = store
        .load_attachment_preview(&image.id)
        .expect("JPEG preview lookup should succeed")
        .expect("ready JPEG should have a preview");
    assert_eq!(preview.mime_type, "image/jpeg");
    assert!(preview.bytes.starts_with(&[0xff, 0xd8]));
    let decoded = ImageReader::new(std::io::Cursor::new(&preview.bytes))
        .with_guessed_format()
        .expect("preview format should be detected")
        .decode()
        .expect("preview should decode");
    assert_eq!(decoded.dimensions(), (PREVIEW_MAX_AXIS, 160));
}
