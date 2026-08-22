//! Time-based Trash retention contract tests.

use super::{ConversationRetentionPeriod, ConversationStore, tests::test_database_path};

const DAY_MS: i64 = 24 * 60 * 60 * 1_000;
const TEST_NOW_MS: i64 = 400 * DAY_MS;

/// Moves one fixture to Trash and assigns its deletion timestamp directly at the test boundary.
fn trash_at(store: &ConversationStore, title: &str, deleted_at_ms: i64) -> String {
    let conversation = store
        .create_conversation(title)
        .expect("retention fixture should create");
    store
        .delete_conversation(&conversation.id)
        .expect("retention fixture should move to Trash");
    store
        .open()
        .expect("fixture database should open")
        .execute(
            "UPDATE conversations SET deleted_at_ms = ?1 WHERE id = ?2",
            rusqlite::params![deleted_at_ms, conversation.id],
        )
        .expect("fixture deletion time should update");
    conversation.id
}

#[test]
fn retention_is_disabled_by_default_and_persists_only_supported_periods() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");

    assert_eq!(
        store
            .conversation_retention_policy()
            .expect("default policy should load")
            .period,
        ConversationRetentionPeriod::Forever
    );
    let saved = store
        .set_conversation_retention_period(ConversationRetentionPeriod::NinetyDays)
        .expect("retention should save");
    assert_eq!(saved.period, ConversationRetentionPeriod::NinetyDays);
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    assert_eq!(
        reopened
            .conversation_retention_policy()
            .expect("saved policy should load")
            .period,
        ConversationRetentionPeriod::NinetyDays
    );
    reopened
        .set_conversation_retention_period(ConversationRetentionPeriod::Forever)
        .expect("manual retention should restore");
    assert_eq!(
        reopened
            .conversation_retention_policy()
            .expect("restored policy should load")
            .period,
        ConversationRetentionPeriod::Forever
    );
}

#[test]
fn retention_forgets_only_trash_at_or_before_the_inclusive_cutoff() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let expired = trash_at(&store, "Expired", TEST_NOW_MS - 31 * DAY_MS);
    let at_cutoff = trash_at(&store, "At cutoff", TEST_NOW_MS - 30 * DAY_MS);
    let recent = trash_at(&store, "Recent", TEST_NOW_MS - 29 * DAY_MS);
    let active = store
        .create_conversation("Active")
        .expect("active fixture should create");
    let archived = store
        .create_conversation("Archived")
        .expect("archived fixture should create");
    store
        .set_conversation_archived(&archived.id, true)
        .expect("fixture should archive");
    store
        .set_conversation_retention_period(ConversationRetentionPeriod::ThirtyDays)
        .expect("retention should save");

    let outcome = store
        .apply_conversation_retention_at(TEST_NOW_MS)
        .expect("retention should apply");
    let retained = store
        .list_conversations()
        .expect("remaining conversations should list")
        .into_iter()
        .map(|conversation| conversation.id)
        .collect::<Vec<_>>();

    assert_eq!(outcome.forgotten_conversations, 2);
    assert!(!retained.contains(&expired));
    assert!(!retained.contains(&at_cutoff));
    assert!(retained.contains(&recent));
    assert!(retained.contains(&active.id));
    assert!(retained.contains(&archived.id));
}

#[test]
fn healthy_startup_applies_the_saved_policy_but_forever_keeps_trash() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let retained = trash_at(&store, "Retained forever", 1);
    drop(store);

    let reopened =
        ConversationStore::initialize(path.clone()).expect("disabled retention should reopen");
    assert!(
        reopened
            .list_conversations()
            .expect("Trash should list")
            .iter()
            .any(|conversation| conversation.id == retained)
    );
    reopened
        .set_conversation_retention_period(ConversationRetentionPeriod::ThirtyDays)
        .expect("retention should save");
    drop(reopened);

    let startup =
        ConversationStore::initialize_for_app(path).expect("enabled retention should reopen");
    let enforced = startup.store;
    assert!(
        enforced
            .list_conversations()
            .expect("retained conversations should list")
            .iter()
            .all(|conversation| conversation.id != retained)
    );
}
