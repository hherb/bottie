//! Opt-in reproducible budgets for large conversation histories and long selected lineages.

use std::time::{Duration, Instant};

use rusqlite::{Connection, params};

use super::*;

const PERFORMANCE_CONVERSATION_COUNT: usize = 2_000;
const PERFORMANCE_MESSAGE_COUNT: usize = 50_000;
const PERFORMANCE_LONG_LINEAGE_COUNT: usize = 600;
const PERFORMANCE_LIST_BUDGET: Duration = Duration::from_millis(250);
const PERFORMANCE_LOAD_BUDGET: Duration = Duration::from_millis(1_000);
const PERFORMANCE_SEARCH_BUDGET: Duration = Duration::from_millis(1_000);
const FIXTURE_TIMESTAMP_MS: i64 = 1_777_003_200_000;
const SEARCH_NEEDLE: &str = "performance-needle";

/// Inserts deterministic rows directly so fixture construction is excluded from measured storage paths.
fn insert_performance_fixture(store: &ConversationStore) -> String {
    let mut connection = store.open().expect("performance fixture store should open");
    drop_derived_fixture_triggers(&connection);
    let transaction = connection
        .transaction()
        .expect("performance fixture transaction should begin");
    let long_conversation_id = "performance-conversation-0000".to_owned();

    for conversation_index in 0..PERFORMANCE_CONVERSATION_COUNT {
        let conversation_id = format!("performance-conversation-{conversation_index:04}");
        let branch_id = format!("performance-branch-{conversation_index:04}");
        let updated_at_ms = FIXTURE_TIMESTAMP_MS - conversation_index as i64;
        let archived_at_ms = (conversation_index >= 1_500).then_some(updated_at_ms);
        transaction
            .execute(
                "INSERT INTO conversations
                 (id, profile_id, title, created_at_ms, updated_at_ms, archived_at_ms, current_branch_id)
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5, NULL)",
                params![
                    conversation_id,
                    DEFAULT_PROFILE_ID,
                    format!("Performance conversation {conversation_index:04}"),
                    updated_at_ms,
                    archived_at_ms,
                ],
            )
            .expect("performance conversation should insert");
        transaction
            .execute(
                "INSERT INTO branches (id, conversation_id, name, created_at_ms) VALUES (?1, ?2, 'Main', ?3)",
                params![branch_id, conversation_id, updated_at_ms],
            )
            .expect("performance branch should insert");
        transaction
            .execute(
                "UPDATE conversations SET current_branch_id = ?1 WHERE id = ?2",
                params![branch_id, conversation_id],
            )
            .expect("performance branch should select");
    }

    insert_performance_messages(&transaction, &long_conversation_id);
    transaction
        .commit()
        .expect("performance fixture should commit atomically");
    long_conversation_id
}

/// Removes derived-index maintenance only from the disposable fixture so setup is not the benchmark.
fn drop_derived_fixture_triggers(connection: &Connection) {
    let trigger_names = {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'")
            .expect("performance fixture triggers should list");
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("performance fixture trigger query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("performance fixture trigger names should decode")
    };
    for name in trigger_names {
        assert!(
            name.chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        );
        connection
            .execute_batch(&format!("DROP TRIGGER \"{name}\""))
            .expect("performance fixture trigger should drop");
    }
}

/// Populates one long lineage and distributes the remaining searchable messages across the history.
fn insert_performance_messages(connection: &Connection, long_conversation_id: &str) {
    let mut parent_id: Option<String> = None;
    for message_index in 0..PERFORMANCE_MESSAGE_COUNT {
        let conversation_index = if message_index < PERFORMANCE_LONG_LINEAGE_COUNT {
            0
        } else {
            1 + (message_index - PERFORMANCE_LONG_LINEAGE_COUNT)
                % (PERFORMANCE_CONVERSATION_COUNT - 1)
        };
        let conversation_id = format!("performance-conversation-{conversation_index:04}");
        let branch_id = format!("performance-branch-{conversation_index:04}");
        let message_id = format!("performance-message-{message_index:05}");
        let sequence = if conversation_id == long_conversation_id {
            message_index
        } else {
            (message_index - PERFORMANCE_LONG_LINEAGE_COUNT) / (PERFORMANCE_CONVERSATION_COUNT - 1)
        };
        let message_parent = (conversation_id == long_conversation_id)
            .then(|| parent_id.clone())
            .flatten();
        let searchable_suffix = if message_index % 997 == 0 {
            SEARCH_NEEDLE
        } else {
            "ordinary retained content"
        };
        connection
            .execute(
                "INSERT INTO messages
                 (id, conversation_id, branch_id, parent_message_id, role, state, created_at_ms, sequence)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'final', ?6, ?7)",
                params![
                    message_id,
                    conversation_id,
                    branch_id,
                    message_parent,
                    if message_index % 2 == 0 { "user" } else { "assistant" },
                    FIXTURE_TIMESTAMP_MS + message_index as i64,
                    sequence as i64,
                ],
            )
            .expect("performance message should insert");
        connection
            .execute(
                "INSERT INTO message_blocks (id, message_id, ordinal, block_type, text_content)
                 VALUES (?1, ?2, 0, 'text', ?3)",
                params![
                    format!("performance-block-{message_index:05}"),
                    message_id,
                    format!("Deterministic message {message_index:05} with {searchable_suffix}"),
                ],
            )
            .expect("performance message block should insert");
        if conversation_id == long_conversation_id {
            parent_id = Some(message_id);
        }
    }
}

/// Runs and reports the native large-history budgets without adding them to the default test duration.
#[test]
#[ignore = "opt-in large-history performance budget"]
fn native_large_history_performance_budget() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("performance storage should initialize");
    let long_conversation_id = insert_performance_fixture(&store);

    let list_started = Instant::now();
    let conversations = store
        .list_conversations()
        .expect("large conversation history should list");
    let list_duration = list_started.elapsed();

    let load_started = Instant::now();
    let conversation = store
        .load_conversation(&long_conversation_id)
        .expect("long selected lineage should load");
    let load_duration = load_started.elapsed();

    let search_started = Instant::now();
    let results = store
        .search_conversations(SEARCH_NEEDLE)
        .expect("large conversation history should search");
    let search_duration = search_started.elapsed();

    eprintln!(
        "performance budgets: list={list_duration:?}, load={load_duration:?}, search={search_duration:?}"
    );
    assert_eq!(conversations.len(), PERFORMANCE_CONVERSATION_COUNT);
    assert_eq!(conversation.messages.len(), PERFORMANCE_LONG_LINEAGE_COUNT);
    assert!(!results.is_empty());
    assert!(list_duration < PERFORMANCE_LIST_BUDGET);
    assert!(load_duration < PERFORMANCE_LOAD_BUDGET);
    assert!(search_duration < PERFORMANCE_SEARCH_BUDGET);
}
