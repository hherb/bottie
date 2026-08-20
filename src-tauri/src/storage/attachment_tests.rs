//! Native content-addressed attachment ingestion tests.

use std::fs;

use sha2::{Digest, Sha256};

use super::{
    ConversationStore,
    attachments::{MAX_ATTACHMENT_BYTES, detect_mime_type, safe_display_name},
    tests::test_database_path,
};

const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

#[test]
fn upgrades_version_seven_stores_with_an_empty_attachment_catalog() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch("DROP TABLE attachments;")
        .expect("version eight table should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 7", [])
        .expect("newer migration records should be removable in the fixture");
    connection
        .pragma_update(None, "user_version", 7)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version seven store should upgrade");
    let connection = upgraded.open().expect("upgraded database should open");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'attachments'",
            [],
            |row| row.get(0),
        )
        .expect("attachment table should be queryable");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        8
    );
    assert_eq!(table_count, 1);
}

#[test]
fn ingests_content_once_and_reuses_it_by_sha256() {
    let database_path = test_database_path();
    let source_directory = database_path
        .parent()
        .expect("test database should have a parent")
        .join("source");
    fs::create_dir_all(&source_directory).expect("source directory should be created");
    let first_path = source_directory.join("diagram.png");
    let duplicate_path = source_directory.join("renamed.png");
    fs::write(&first_path, PNG_BYTES).expect("first source should be written");
    fs::write(&duplicate_path, PNG_BYTES).expect("duplicate source should be written");
    let store =
        ConversationStore::initialize(database_path.clone()).expect("storage should initialize");

    let first = store
        .ingest_attachment(&first_path)
        .expect("first attachment should ingest");
    let expected_hash = format!("{:x}", Sha256::digest(PNG_BYTES));

    assert_eq!(first.display_name, "diagram.png");
    assert_eq!(first.mime_type, "image/png");
    assert_eq!(first.byte_size, PNG_BYTES.len() as u64);
    assert_eq!(first.sha256, expected_hash);
    assert!(!first.duplicate);
    assert_eq!(store.attachment_count().expect("count should load"), 1);
    assert_eq!(
        fs::read(store.attachment_blob_path(&first.sha256)).expect("blob should exist"),
        PNG_BYTES
    );
    drop(store);

    let reopened = ConversationStore::initialize(database_path).expect("storage should reopen");
    let duplicate = reopened
        .ingest_attachment(&duplicate_path)
        .expect("duplicate attachment should resolve after restart");

    assert_eq!(duplicate.id, first.id);
    assert_eq!(duplicate.display_name, first.display_name);
    assert!(duplicate.duplicate);
    assert_eq!(
        reopened
            .attachment_count()
            .expect("count should survive restart"),
        1
    );
}

#[test]
fn rejects_oversized_files_without_leaving_metadata_or_blobs() {
    let database_path = test_database_path();
    let source_path = database_path.with_file_name("too-large.bin");
    let source = fs::File::create(&source_path).expect("oversized source should be created");
    source
        .set_len(MAX_ATTACHMENT_BYTES + 1)
        .expect("oversized source should be sparse");
    let store = ConversationStore::initialize(database_path).expect("storage should initialize");

    let error = store
        .ingest_attachment(&source_path)
        .expect_err("oversized input should be rejected");

    assert_eq!(error.code, "invalid_request");
    assert!(error.message.contains("25 MiB"));
    assert_eq!(store.attachment_count().expect("count should load"), 0);
}

#[test]
fn sniffs_text_and_binary_content_without_trusting_extensions() {
    assert_eq!(
        detect_mime_type(b"# Bottie\n\nLocal notes", "notes.bin"),
        "text/plain"
    );
    assert_eq!(detect_mime_type(PNG_BYTES, "pretend.txt"), "image/png");
    assert_eq!(
        detect_mime_type(b"\0\x01\x02unknown", "notes.txt"),
        "application/octet-stream"
    );
}

#[test]
fn bounds_and_sanitizes_untrusted_display_names() {
    assert_eq!(
        safe_display_name("  report\u{202e}cod.exe  "),
        "reportcod.exe"
    );
    assert_eq!(
        safe_display_name("../folder\\secret.txt"),
        "foldersecret.txt"
    );
    assert_eq!(safe_display_name("..."), "attachment");
    assert_eq!(safe_display_name(&"a".repeat(200)).chars().count(), 120);
}
