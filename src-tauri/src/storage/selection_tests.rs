//! Last-open conversation selection contract tests.

use super::*;

#[test]
fn seeds_a_version_three_store_from_its_newest_active_conversation() {
    let path = tests::test_database_path();
    let connection = Connection::open(&path).expect("version three database should open");
    connection
        .execute_batch(MIGRATION_1)
        .expect("foundation migration should apply");
    connection
        .execute_batch(MIGRATION_2)
        .expect("message order migration should apply");
    connection
        .execute_batch(MIGRATION_3)
        .expect("provider run migration should apply");
    for version in 1..=3 {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (?1, ?2, ?1)",
                params![
                    version,
                    migrate::migration_name(version).expect("fixture migration name should exist")
                ],
            )
            .expect("historical migration should be recorded");
    }
    connection
        .execute(
            "INSERT INTO profiles (id, name, created_at_ms) VALUES (?1, ?2, 1)",
            params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME],
        )
        .expect("default profile should be inserted");
    for (id, title, updated_at_ms) in [("older", "Older", 10), ("newer", "Newer", 20)] {
        connection
            .execute(
                "INSERT INTO conversations (id, profile_id, title, created_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![id, DEFAULT_PROFILE_ID, title, updated_at_ms],
            )
            .expect("conversation should be inserted");
        connection
            .execute(
                "INSERT INTO branches (id, conversation_id, name, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    format!("{id}-branch"),
                    id,
                    DEFAULT_BRANCH_NAME,
                    updated_at_ms
                ],
            )
            .expect("branch should be inserted");
    }
    connection
        .pragma_update(None, "user_version", 3)
        .expect("version should be set");
    drop(connection);

    let restored = ConversationStore::initialize(path)
        .expect("store should upgrade")
        .load_last_open_conversation()
        .expect("selection should load")
        .expect("migration should seed one selection");

    assert_eq!(restored.id, "newer");
}

#[test]
fn restores_the_exact_last_open_conversation_across_restart() {
    let path = tests::test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let first = store
        .create_conversation("First conversation")
        .expect("first conversation should be created");
    let second = store
        .create_conversation("Second conversation")
        .expect("second conversation should be created");

    store
        .open_conversation(&first.id)
        .expect("the older conversation should open");
    drop(store);

    let restored = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_last_open_conversation()
        .expect("selection should load")
        .expect("one conversation should be selected");

    assert_eq!(restored.id, first.id);
    assert_ne!(restored.id, second.id);
}

#[test]
fn preserves_an_intentional_blank_new_chat_across_restart() {
    let path = tests::test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    store
        .create_conversation("Existing conversation")
        .expect("conversation should be created");

    store
        .clear_last_open_conversation()
        .expect("selection should clear");
    drop(store);

    let restored = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_last_open_conversation()
        .expect("selection should load");

    assert!(restored.is_none());
}

#[test]
fn lifecycle_changes_clear_only_the_selected_conversation() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let selected = store
        .create_conversation("Selected conversation")
        .expect("selected conversation should be created");
    let other = store
        .create_conversation("Other conversation")
        .expect("other conversation should be created");
    store
        .open_conversation(&selected.id)
        .expect("selected conversation should open");

    store
        .set_conversation_archived(&other.id, true)
        .expect("other conversation should archive");
    assert_eq!(
        store
            .load_last_open_conversation()
            .expect("selection should load")
            .expect("selection should remain")
            .id,
        selected.id
    );

    store
        .delete_conversation(&selected.id)
        .expect("selected conversation should move to trash");
    assert!(
        store
            .load_last_open_conversation()
            .expect("selection should load")
            .is_none()
    );
}
