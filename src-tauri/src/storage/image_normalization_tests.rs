//! Native JPEG and PNG normalization policy tests.

use std::fs;

use image::{
    ExtendedColorType, GenericImageView, ImageEncoder, ImageFormat, ImageReader,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};

use super::{
    ConversationStore,
    image_codec::{MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS},
    tests::{completed_ingestion, process_pending_attachments, test_database_path},
};

const PRIVATE_PNG_METADATA: &[u8] = b"Comment\0secret-camera-metadata";

/// Writes and ingests one image fixture beside the isolated test store.
fn ingest_fixture(
    store: &ConversationStore,
    name: &str,
    bytes: &[u8],
) -> super::IngestedAttachment {
    let source_path = store.path.with_file_name(name);
    fs::write(&source_path, bytes).expect("image fixture should be written");
    let ingested = store
        .ingest_attachment(&source_path)
        .expect("image fixture should ingest");
    completed_ingestion(store, ingested)
}

/// Encodes one small RGBA PNG through the same codec family used by normalization.
fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![127_u8; width as usize * height as usize * 4];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

/// Inserts one valid PNG text chunk immediately before the terminal IEND chunk.
fn png_with_private_metadata() -> Vec<u8> {
    let mut encoded = png_fixture(2, 1);
    let iend = encoded.split_off(encoded.len() - 12);
    encoded.extend_from_slice(&(PRIVATE_PNG_METADATA.len() as u32).to_be_bytes());
    encoded.extend_from_slice(b"tEXt");
    encoded.extend_from_slice(PRIVATE_PNG_METADATA);
    encoded.extend_from_slice(
        &crc32(&[b"tEXt".as_slice(), PRIVATE_PNG_METADATA].concat()).to_be_bytes(),
    );
    encoded.extend_from_slice(&iend);
    encoded
}

/// Encodes a 2x1 JPEG and adds EXIF orientation six so normalization must rotate it.
fn oriented_jpeg_fixture() -> Vec<u8> {
    let pixels = [255_u8, 0, 0, 0, 255, 0];
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 90)
        .write_image(&pixels, 2, 1, ExtendedColorType::Rgb8)
        .expect("JPEG fixture should encode");
    let exif = [
        b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 0x01, 3, 0, 1, 0,
        0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
    ];
    let mut oriented = encoded.drain(..2).collect::<Vec<_>>();
    oriented.extend_from_slice(&[0xff, 0xe1]);
    oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    oriented.extend_from_slice(&exif);
    oriented.extend_from_slice(&encoded);
    oriented
}

/// Computes the PNG chunk checksum without adding another production dependency.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[test]
fn normalizes_png_pixels_without_retaining_private_metadata() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "private.png", &png_with_private_metadata());

    assert_eq!(
        attachment.normalization.state,
        super::image_normalization::ImageNormalizationState::Ready
    );
    assert_eq!(
        attachment.normalization.format,
        Some(super::image_normalization::NormalizedImageFormat::Png)
    );
    assert_eq!(
        (
            attachment.normalization.width,
            attachment.normalization.height
        ),
        (Some(2), Some(1))
    );
    let normalized = store
        .normalized_image_bytes_for_test(&attachment.id)
        .expect("normalized bytes should load")
        .expect("normalized bytes should exist");
    assert!(normalized.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(
        !normalized
            .windows(PRIVATE_PNG_METADATA.len())
            .any(|window| window == PRIVATE_PNG_METADATA)
    );
    let normalized_sha256: String = store
        .open()
        .expect("database should open")
        .query_row(
            "SELECT normalized_sha256 FROM attachment_image_normalizations WHERE attachment_id = ?1",
            [&attachment.id],
            |row| row.get(0),
        )
        .expect("native derivative identity should exist");
    let serialized =
        serde_json::to_string(&attachment).expect("path-free metadata should serialize");
    assert!(!serialized.contains("normalizedSha256"));
    assert!(!serialized.contains(&normalized_sha256));
    assert!(!serialized.contains("normalized-images"));
}

#[test]
fn applies_jpeg_orientation_before_removing_exif_metadata() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "oriented.jpg", &oriented_jpeg_fixture());

    assert_eq!(
        attachment.normalization.state,
        super::image_normalization::ImageNormalizationState::Ready
    );
    assert_eq!(
        attachment.normalization.format,
        Some(super::image_normalization::NormalizedImageFormat::Jpeg)
    );
    assert_eq!(
        (
            attachment.normalization.width,
            attachment.normalization.height
        ),
        (Some(1), Some(2))
    );
    let normalized = store
        .normalized_image_bytes_for_test(&attachment.id)
        .expect("normalized bytes should load")
        .expect("normalized bytes should exist");
    assert!(!normalized.windows(6).any(|window| window == b"Exif\0\0"));
    let decoded = ImageReader::with_format(std::io::Cursor::new(normalized), ImageFormat::Jpeg)
        .decode()
        .expect("normalized JPEG should decode");
    assert_eq!(decoded.dimensions(), (1, 2));
}

#[test]
fn records_dimension_and_pixel_policy_failures_without_a_derivative() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let too_wide = ingest_fixture(&store, "wide.png", &png_fixture(MAX_IMAGE_DIMENSION + 1, 1));
    let width = MAX_IMAGE_DIMENSION;
    let height = (MAX_IMAGE_PIXELS / u64::from(width)) as u32 + 1;
    let too_many_pixels = ingest_fixture(&store, "pixels.png", &png_header(width, height));

    assert_eq!(
        too_wide.normalization.error_code.as_deref(),
        Some("image_dimension_limit_exceeded")
    );
    assert_eq!(
        too_many_pixels.normalization.error_code.as_deref(),
        Some("image_pixel_limit_exceeded")
    );
    assert_eq!(
        store
            .normalized_image_bytes_for_test(&too_wide.id)
            .expect("state should load"),
        None
    );
    assert_eq!(
        store
            .normalized_image_bytes_for_test(&too_many_pixels.id)
            .expect("state should load"),
        None
    );
}

#[test]
fn upgrades_version_twelve_images_and_resumes_normalization() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "migration.png", &png_fixture(3, 2));
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE conversation_retention_policies;
             DROP TABLE conversation_memory_preferences;
             DROP TABLE conversation_attachments;
             DROP TABLE attachment_text_indexing;
             DROP TABLE attachment_image_normalizations;",
        )
        .expect("newer attachment-processing tables should be removable");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 12", [])
        .expect("newer migration record should be removable");
    connection
        .pragma_update(None, "user_version", 12)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded =
        ConversationStore::initialize(path).expect("version twelve store should upgrade");
    process_pending_attachments(&upgraded);
    let stored = upgraded
        .stored_attachment_for_test(&attachment.id)
        .expect("attachment should load")
        .expect("attachment should remain present");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        20
    );
    assert_eq!(
        stored.normalization.state,
        super::image_normalization::ImageNormalizationState::Ready
    );
    assert_eq!(
        stored.normalization.format,
        Some(super::image_normalization::NormalizedImageFormat::Png)
    );
    assert_eq!(
        (stored.normalization.width, stored.normalization.height),
        (Some(3), Some(2))
    );
}

/// Builds a header-only PNG because pixel policy must reject it before decompression.
fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    append_png_chunk(&mut bytes, b"IHDR", &ihdr);
    append_png_chunk(
        &mut bytes,
        b"IDAT",
        &[0x78, 0x01, 0x01, 0, 0, 0xff, 0xff, 0, 0, 0, 1],
    );
    append_png_chunk(&mut bytes, b"IEND", &[]);
    bytes
}

/// Appends one checksummed PNG chunk to an in-memory fixture.
fn append_png_chunk(bytes: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(data);
    bytes.extend_from_slice(&crc32(&[kind.as_slice(), data].concat()).to_be_bytes());
}
