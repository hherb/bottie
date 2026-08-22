//! Native content-addressed attachment ingestion tests.

use std::fs;

use sha2::{Digest, Sha256};

use super::{
    AttachmentExtractionFormat, AttachmentExtractionState, ConversationStore, MessageState,
    NewStoredMessage, StoredRole,
    attachments::{MAX_ATTACHMENT_BYTES, detect_mime_type, safe_display_name},
    extraction::MAX_EXTRACTED_TEXT_BYTES,
    tests::{completed_ingestion, process_pending_attachments, test_database_path},
};

const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

/// Writes and ingests one attachment fixture beside the isolated test store.
fn ingest_fixture(
    store: &ConversationStore,
    name: &str,
    bytes: &[u8],
) -> super::IngestedAttachment {
    let source_path = store.path.with_file_name(name);
    fs::write(&source_path, bytes).expect("attachment fixture should be written");
    let ingested = store
        .ingest_attachment(&source_path)
        .expect("attachment fixture should ingest");
    completed_ingestion(store, ingested)
}

#[test]
fn upgrades_version_seven_stores_with_an_empty_attachment_catalog() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE conversation_retention_policies;
             DROP TABLE conversation_memory_preferences;
             DROP TABLE conversation_attachments;
             DROP TABLE attachment_text_indexing;
             DROP TABLE attachment_image_normalizations;
             DROP TABLE attachment_extractions;
             DROP TABLE message_attachments;
             DROP TABLE attachments;",
        )
        .expect("newer attachment tables should be removable in the fixture");
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
        20
    );
    assert_eq!(table_count, 1);
}

#[test]
fn upgrades_version_eight_stores_with_empty_message_associations() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE conversation_retention_policies;
             DROP TABLE conversation_memory_preferences;
             DROP TABLE conversation_attachments;
             DROP TABLE attachment_text_indexing;
             DROP TABLE attachment_image_normalizations;
             DROP TABLE attachment_extractions;
             DROP TABLE message_attachments;",
        )
        .expect("version nine table should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 8", [])
        .expect("newer migration records should be removable in the fixture");
    connection
        .pragma_update(None, "user_version", 8)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version eight store should upgrade");
    let connection = upgraded.open().expect("upgraded database should open");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'message_attachments'",
            [],
            |row| row.get(0),
        )
        .expect("message attachment table should be queryable");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        20
    );
    assert_eq!(table_count, 1);
}

#[test]
fn upgrades_version_nine_stores_and_extracts_existing_text_content() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "migration.md", b"# Existing attachment");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE conversation_retention_policies;
             DROP TABLE conversation_memory_preferences;
             DROP TABLE conversation_attachments;
             DROP TABLE attachment_text_indexing;
             DROP TABLE attachment_image_normalizations;
             DROP TABLE attachment_extractions;",
        )
        .expect("version ten table should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 9", [])
        .expect("newer migration records should be removable in the fixture");
    connection
        .pragma_update(None, "user_version", 9)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version nine store should upgrade");
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
    assert_eq!(stored.extraction.state, AttachmentExtractionState::Ready);
    assert_eq!(
        stored.extraction.format,
        Some(AttachmentExtractionFormat::Markdown)
    );
    assert_eq!(
        upgraded
            .extracted_text_for_test(&attachment.id)
            .expect("extracted text should load")
            .as_deref(),
        Some("# Existing attachment")
    );
}

#[test]
fn upgrades_version_fourteen_stores_with_empty_conversation_scope() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE conversation_retention_policies;
             DROP TABLE conversation_memory_preferences;
             DROP TABLE conversation_attachments;",
        )
        .expect("conversation attachment table should be removable");
    connection
        .execute("DELETE FROM schema_migrations WHERE version >= 15", [])
        .expect("newer migration records should be removable");
    connection
        .pragma_update(None, "user_version", 14)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded =
        ConversationStore::initialize(path).expect("version fourteen store should upgrade");
    let connection = upgraded.open().expect("upgraded database should open");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name = 'conversation_attachments'",
            [],
            |row| row.get(0),
        )
        .expect("conversation attachment table should be queryable");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        20
    );
    assert_eq!(table_count, 1);
}

#[test]
fn associates_ordered_attachments_with_a_user_message_across_reopen_and_branching() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let first = ingest_fixture(&store, "notes.txt", b"Durable notes");
    let second = ingest_fixture(&store, "diagram.png", PNG_BYTES);
    let conversation = store
        .create_conversation("Attachment association")
        .expect("conversation should be created");
    let request = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Keep these with the request".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[second.id.clone(), first.id.clone()],
        )
        .expect("message and attachments should commit together");

    assert_eq!(
        request
            .attachments
            .iter()
            .map(|attachment| attachment.id.as_str())
            .collect::<Vec<_>>(),
        vec![second.id.as_str(), first.id.as_str()]
    );
    assert_eq!(
        request.attachments[1].extraction.state,
        AttachmentExtractionState::Ready
    );
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    let loaded = reopened
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    assert_eq!(loaded.messages[0].attachments, request.attachments);

    let forked = reopened
        .fork_from_user_message(&conversation.id, &request.id, "Edit but retain context")
        .expect("request should fork");
    assert_eq!(
        forked.conversation.messages[0].attachments,
        request.attachments
    );
}

#[test]
fn associates_ordered_attachments_with_a_conversation_across_reopen_and_branching() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let first = ingest_fixture(&store, "project.md", b"# Project context");
    let second = ingest_fixture(&store, "reference.txt", b"Shared reference");
    let conversation = store
        .create_conversation("Conversation attachment scope")
        .expect("conversation should be created");

    let attached = store
        .add_conversation_attachments(
            &conversation.id,
            &[second.id.clone(), first.id.clone(), second.id.clone()],
        )
        .expect("conversation attachments should commit");

    assert_eq!(
        attached
            .iter()
            .map(|attachment| attachment.id.as_str())
            .collect::<Vec<_>>(),
        vec![second.id.as_str(), first.id.as_str()]
    );
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    let loaded = reopened
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    assert_eq!(loaded.attachments, attached);

    let request = reopened
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Use the shared context".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("request should append");
    let forked = reopened
        .fork_from_user_message(&conversation.id, &request.id, "Keep shared context")
        .expect("request should fork");
    assert_eq!(forked.conversation.attachments, attached);
}

#[test]
fn validates_and_removes_only_conversation_scoped_associations() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "scope.txt", b"Conversation scope");
    let unrelated = ingest_fixture(&store, "other.txt", b"Other content");
    let conversation = store
        .create_conversation("Scoped removal")
        .expect("conversation should be created");
    store
        .add_conversation_attachments(&conversation.id, &[attachment.id.clone()])
        .expect("conversation attachment should commit");

    let missing = store.remove_conversation_attachment(&conversation.id, &unrelated.id);
    assert_eq!(
        missing.expect_err("unassociated content should fail").code,
        "not_found"
    );

    let remaining = store
        .remove_conversation_attachment(&conversation.id, &attachment.id)
        .expect("conversation attachment should be removable");
    assert!(remaining.is_empty());
    assert_eq!(store.attachment_count().expect("catalog should remain"), 2);
}

#[test]
fn bounds_conversation_scope_without_partially_associating_content() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Bounded conversation scope")
        .expect("conversation should be created");
    let attachments = (0..=super::attachments::MAX_ATTACHMENT_SELECTION_COUNT)
        .map(|index| {
            ingest_fixture(
                &store,
                &format!("context-{index}.txt"),
                format!("Context {index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let accepted = attachments
        .iter()
        .take(super::attachments::MAX_ATTACHMENT_SELECTION_COUNT)
        .map(|attachment| attachment.id.clone())
        .collect::<Vec<_>>();
    store
        .add_conversation_attachments(&conversation.id, &accepted)
        .expect("bounded conversation scope should commit");

    let error = store
        .add_conversation_attachments(&conversation.id, &[attachments.last().unwrap().id.clone()])
        .expect_err("ninth conversation attachment should fail");
    let loaded = store
        .load_conversation(&conversation.id)
        .expect("conversation should remain readable");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        loaded.attachments.len(),
        super::attachments::MAX_ATTACHMENT_SELECTION_COUNT
    );
}

#[test]
fn rejects_invalid_attachment_sets_without_appending_a_message() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "notes.txt", b"Durable notes");
    let conversation = store
        .create_conversation("Invalid association")
        .expect("conversation should be created");

    let duplicate = store.append_message_with_attachments(
        NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Do not store this".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        },
        &[attachment.id.clone(), attachment.id.clone()],
    );
    let missing = store.append_message_with_attachments(
        NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Do not store this either".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        },
        &["missing".into()],
    );

    assert_eq!(
        duplicate.expect_err("duplicates should fail").code,
        "invalid_request"
    );
    assert_eq!(
        missing.expect_err("unknown identities should fail").code,
        "invalid_request"
    );
    assert!(
        store
            .load_conversation(&conversation.id)
            .expect("conversation should remain readable")
            .messages
            .is_empty()
    );
}

#[test]
fn removes_only_the_visible_message_association_and_retains_content() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let attachment = ingest_fixture(&store, "notes.txt", b"Durable notes");
    let blob_path = store.attachment_blob_path(&attachment.sha256);
    let conversation = store
        .create_conversation("Remove association")
        .expect("conversation should be created");
    let main_branch_id = conversation.current_branch_id.clone();
    let request = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Attach then remove".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[attachment.id.clone()],
        )
        .expect("association should be stored");
    let alternative = store
        .fork_from_user_message(&conversation.id, &request.id, "Alternative request")
        .expect("alternative should be created");
    let alternative_request = alternative
        .conversation
        .messages
        .last()
        .expect("alternative request should be visible");
    store
        .select_branch(&conversation.id, &main_branch_id)
        .expect("main branch should be selected");

    assert!(
        store
            .remove_message_attachment(&conversation.id, &alternative_request.id, &attachment.id,)
            .is_err()
    );

    let updated = store
        .remove_message_attachment(&conversation.id, &request.id, &attachment.id)
        .expect("visible association should be removable");

    assert!(updated.attachments.is_empty());
    assert_eq!(
        store
            .attachment_count()
            .expect("catalog should remain readable"),
        1
    );
    assert_eq!(
        fs::read(blob_path).expect("retained blob should remain"),
        b"Durable notes"
    );
    assert!(
        store
            .remove_message_attachment(&conversation.id, &request.id, &attachment.id)
            .is_err()
    );
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
fn extracts_utf8_plain_text_and_markdown_without_exposing_content_metadata() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let plain = ingest_fixture(&store, "notes.txt", b"Durable notes\n");
    let markdown = ingest_fixture(&store, "guide.MD", b"# Guide\n\nUse **Bottie**.\n");

    let plain_stored = store
        .stored_attachment_for_test(&plain.id)
        .expect("plain attachment should load")
        .expect("plain attachment should exist");
    let markdown_stored = store
        .stored_attachment_for_test(&markdown.id)
        .expect("Markdown attachment should load")
        .expect("Markdown attachment should exist");

    assert_eq!(
        plain_stored.extraction.state,
        AttachmentExtractionState::Ready
    );
    assert_eq!(
        plain_stored.extraction.format,
        Some(AttachmentExtractionFormat::PlainText)
    );
    assert_eq!(plain_stored.extraction.character_count, Some(14));
    assert_eq!(
        markdown_stored.extraction.state,
        AttachmentExtractionState::Ready
    );
    assert_eq!(
        markdown_stored.extraction.format,
        Some(AttachmentExtractionFormat::Markdown)
    );
    assert_eq!(
        store
            .extracted_text_for_test(&markdown.id)
            .expect("Markdown extraction should load")
            .as_deref(),
        Some("# Guide\n\nUse **Bottie**.\n")
    );
}

#[test]
fn records_unsupported_and_bounded_failure_states_without_extracted_text() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let image = ingest_fixture(&store, "diagram.png", PNG_BYTES);
    let oversized = ingest_fixture(
        &store,
        "large.txt",
        &vec![b'a'; MAX_EXTRACTED_TEXT_BYTES + 1],
    );
    let image_stored = store
        .stored_attachment_for_test(&image.id)
        .expect("image attachment should load")
        .expect("image attachment should exist");
    let oversized_stored = store
        .stored_attachment_for_test(&oversized.id)
        .expect("oversized text attachment should load")
        .expect("oversized text attachment should exist");

    assert_eq!(
        image_stored.extraction.state,
        AttachmentExtractionState::Unsupported
    );
    assert_eq!(image_stored.extraction.format, None);
    assert_eq!(
        oversized_stored.extraction.state,
        AttachmentExtractionState::Failed
    );
    assert_eq!(
        oversized_stored.extraction.error_code.as_deref(),
        Some("content_too_large")
    );
    assert_eq!(
        store
            .extracted_text_for_test(&oversized.id)
            .expect("failed extraction should remain queryable"),
        None
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
