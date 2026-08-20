//! Manual SQLite-backup contract tests.

use rusqlite::Connection;

use super::*;

#[test]
fn creates_a_consistent_independently_readable_backup_without_changing_live_state() {
    let source_path = tests::test_database_path();
    let backup_path = source_path.with_file_name("bottie-backup.sqlite3");
    let store =
        ConversationStore::initialize(source_path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Backup snapshot")
        .expect("conversation should be created");
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Retain committed WAL content".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("message should append");

    store
        .backup_to(&backup_path)
        .expect("online backup should complete");

    let backup = Connection::open(&backup_path).expect("backup should open independently");
    let integrity: String = backup
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .expect("backup integrity should be readable");
    let backed_up_text: String = backup
        .query_row(
            "SELECT message_blocks.text_content
             FROM message_blocks JOIN messages ON messages.id = message_blocks.message_id
             WHERE messages.conversation_id = ?1 AND message_blocks.block_type = 'text'",
            [&conversation.id],
            |row| row.get(0),
        )
        .expect("committed message should exist in the backup");
    let selected = store
        .load_last_open_conversation()
        .expect("live selection should load")
        .expect("live conversation should remain selected");

    assert_ne!(source_path, backup_path);
    assert_eq!(integrity, "ok");
    assert_eq!(backed_up_text, "Retain committed WAL content");
    assert_eq!(selected.id, conversation.id);
}

#[test]
fn rejects_the_live_database_as_its_own_backup_destination() {
    let source_path = tests::test_database_path();
    let store =
        ConversationStore::initialize(source_path.clone()).expect("storage should initialize");

    let error = store
        .backup_to(&source_path)
        .expect_err("the live database path must be rejected");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        error.message,
        "Choose a different location for the Bottie backup."
    );
}
