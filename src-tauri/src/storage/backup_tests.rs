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

#[test]
fn restores_a_valid_backup_after_preserving_the_live_store() {
    let live_path = tests::test_database_path();
    let backup_path = live_path.with_file_name("selected-backup.sqlite3");
    let safety_path = live_path.with_file_name("bottie-pre-restore.sqlite3");
    let live = ConversationStore::initialize(live_path).expect("live storage should initialize");
    let original = live
        .create_conversation("Current local conversation")
        .expect("live conversation should be created");
    let backup_source_path = live
        .path
        .with_file_name("backup-source")
        .join("bottie.sqlite3");
    let backup_source =
        ConversationStore::initialize(backup_source_path).expect("backup source should initialize");
    let restored = backup_source
        .create_conversation("Conversation from backup")
        .expect("backup conversation should be created");
    backup_source
        .backup_to(&backup_path)
        .expect("selected backup should be created");

    live.restore_from(&backup_path, &safety_path)
        .expect("valid backup should restore");

    let restored_conversations = live
        .list_conversations()
        .expect("restored conversations should list");
    let safety = ConversationStore::initialize(safety_path).expect("safety copy should reopen");
    let preserved_conversations = safety
        .list_conversations()
        .expect("preserved conversations should list");
    let restored_selection = live
        .load_last_open_conversation()
        .expect("restored selection should load")
        .expect("backup selection should be preserved");
    let preserved_selection = safety
        .load_last_open_conversation()
        .expect("safety selection should load")
        .expect("live selection should be preserved");
    assert_eq!(restored_conversations.len(), 1);
    assert_eq!(restored_conversations[0].id, restored.id);
    assert_eq!(restored_selection.id, restored.id);
    assert_eq!(preserved_conversations.len(), 1);
    assert_eq!(preserved_conversations[0].id, original.id);
    assert_eq!(preserved_selection.id, original.id);
}

#[test]
fn rejects_a_non_bottie_database_without_changing_the_live_store() {
    let live_path = tests::test_database_path();
    let invalid_path = live_path.with_file_name("unrelated.sqlite3");
    let safety_path = live_path.with_file_name("bottie-pre-restore.sqlite3");
    let live = ConversationStore::initialize(live_path).expect("live storage should initialize");
    let original = live
        .create_conversation("Keep this conversation")
        .expect("live conversation should be created");
    let unrelated = Connection::open(&invalid_path).expect("unrelated SQLite database should open");
    unrelated
        .execute("CREATE TABLE unrelated (value TEXT NOT NULL)", [])
        .expect("unrelated schema should be created");
    drop(unrelated);

    let error = live
        .restore_from(&invalid_path, &safety_path)
        .expect_err("an unrelated SQLite database must be rejected");

    let conversations = live
        .list_conversations()
        .expect("live conversations should still list");
    assert_eq!(error.code, "invalid_request");
    assert_eq!(error.message, "Choose a valid Bottie backup.");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].id, original.id);
    assert!(!safety_path.exists());
}
