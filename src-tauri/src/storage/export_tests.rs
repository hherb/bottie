//! Selected and batch conversation export contract tests.

use serde_json::json;

use super::export::{
    json_export, markdown_export, render_conversation_json, render_conversation_markdown,
};
use super::tools::StoredToolInvocation;
use super::*;

/// Creates one durable message for pure export-rendering coverage.
fn export_message(
    role: StoredRole,
    text: &str,
    reasoning: Option<&str>,
    state: MessageState,
) -> StoredMessage {
    StoredMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role,
        text: text.into(),
        reasoning: reasoning.map(str::to_owned),
        state,
        provider_id: (role == StoredRole::Assistant).then(|| "ollama".into()),
        model_id: (role == StoredRole::Assistant).then(|| "qwen3:latest".into()),
        provider_run: None,
        rating: (role == StoredRole::Assistant).then_some(ResponseRating::Good),
        attachments: Vec::new(),
        created_at_ms: 1,
    }
}

#[test]
fn renders_selected_conversation_content_and_metadata_as_markdown() {
    let conversation = StoredConversation {
        id: "conversation-id".into(),
        title: "Architecture & safety".into(),
        current_branch_id: "branch-id".into(),
        branches: vec![],
        messages: vec![
            export_message(
                StoredRole::User,
                "Explain the **boundary**.",
                None,
                MessageState::Final,
            ),
            export_message(
                StoredRole::Assistant,
                "Rust owns trust.",
                Some("Check the IPC boundary."),
                MessageState::Final,
            ),
        ],
    };

    assert_eq!(
        render_conversation_markdown(&conversation),
        "# Architecture & safety\n\n\
## User\n\n\
Explain the **boundary**.\n\n\
## Assistant\n\n\
> Provider: `ollama`  \n\
> Model: `qwen3:latest`  \n\
> Rating: Good\n\n\
### Reasoning\n\n\
Check the IPC boundary.\n\n\
### Response\n\n\
Rust owns trust.\n",
    );
}

#[test]
fn labels_non_final_assistant_output_and_builds_a_safe_default_filename() {
    let conversation = StoredConversation {
        id: "conversation-id".into(),
        title: "../../Plans: Q3?".into(),
        current_branch_id: "branch-id".into(),
        branches: vec![],
        messages: vec![export_message(
            StoredRole::Assistant,
            "Retained partial answer",
            None,
            MessageState::Cancelled,
        )],
    };

    let export = markdown_export(&conversation);

    assert_eq!(export.file_name, "bottie-plans-q3.md");
    assert!(export.contents.contains("> Status: Cancelled"));
    assert!(export.contents.ends_with("Retained partial answer\n"));
}

#[test]
fn renders_tool_activity_as_structured_markdown_without_opaque_call_ids() {
    let mut response = export_message(
        StoredRole::Assistant,
        "I found the retained note.",
        None,
        MessageState::Final,
    );
    response.provider_run = Some(StoredProviderRun {
        id: "native-run-id".into(),
        state: ProviderRunState::Completed,
        reasoning_effort: StoredReasoningEffort::Off,
        started_at_ms: 1,
        completed_at_ms: Some(3),
        error_code: None,
        usage: None,
        tool_invocations: vec![StoredToolInvocation {
            ordinal: 0,
            tool_name: "search_memory".into(),
            arguments: json!({"query": "release"}),
            result: Some(super::tools::StoredToolResult {
                output: json!({"title": "Release checklist"}),
                is_error: false,
                created_at_ms: 2,
            }),
            created_at_ms: 1,
        }],
    });
    let conversation = StoredConversation {
        id: "conversation-id".into(),
        title: "Tool record".into(),
        current_branch_id: "branch-id".into(),
        branches: vec![],
        messages: vec![response],
    };

    let rendered = render_conversation_markdown(&conversation);

    assert!(rendered.contains("### Tool activity\n\n#### `search_memory`"));
    assert!(rendered.contains("**Arguments**\n\n```json\n{\n  \"query\": \"release\"\n}\n```"));
    assert!(
        rendered.contains("**Result**\n\n```json\n{\n  \"title\": \"Release checklist\"\n}\n```")
    );
    assert!(rendered.contains("### Response\n\nI found the retained note."));
    assert!(!rendered.contains("native-run-id"));
}

#[test]
fn prepares_only_the_selected_visible_lineage_without_changing_last_open_selection() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let first = store
        .create_conversation("Preserved branches")
        .expect("first conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: first.id.clone(),
            role: StoredRole::User,
            text: "Original request".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("request should append");
    store
        .append_message(NewStoredMessage {
            conversation_id: first.id.clone(),
            role: StoredRole::Assistant,
            text: "Hidden sibling answer".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: Some("ollama".into()),
            model_id: Some("qwen3:latest".into()),
        })
        .expect("response should append");
    store
        .fork_from_user_message(&first.id, &request.id, "Selected request")
        .expect("selected branch should fork");
    store
        .append_message(NewStoredMessage {
            conversation_id: first.id.clone(),
            role: StoredRole::Assistant,
            text: "Selected answer".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: Some("ollama".into()),
            model_id: Some("qwen3:latest".into()),
        })
        .expect("selected response should append");
    let second = store
        .create_conversation("Still selected")
        .expect("second conversation should be created");

    let export = store
        .prepare_markdown_export(&first.id)
        .expect("selected lineage should export");
    let selected = store
        .load_last_open_conversation()
        .expect("selection should load")
        .expect("a conversation should remain selected");

    assert!(export.contents.contains("Selected request"));
    assert!(export.contents.contains("Selected answer"));
    assert!(!export.contents.contains("Original request"));
    assert!(!export.contents.contains("Hidden sibling answer"));
    assert_eq!(selected.id, second.id);
}

#[test]
fn writes_the_prepared_utf8_document_to_an_explicit_native_path() {
    let conversation = StoredConversation {
        id: "conversation-id".into(),
        title: "Unicode export".into(),
        current_branch_id: "branch-id".into(),
        branches: vec![],
        messages: vec![export_message(
            StoredRole::User,
            "Keep café notes exact.",
            None,
            MessageState::Final,
        )],
    };
    let export = markdown_export(&conversation);
    let path = tests::test_database_path().with_file_name("conversation.md");

    export
        .write_to(&path)
        .expect("the Markdown file should be written");

    assert_eq!(
        std::fs::read_to_string(path).expect("the Markdown file should be readable"),
        "# Unicode export\n\n## User\n\nKeep café notes exact.\n"
    );
}

#[test]
fn renders_a_versioned_portable_json_contract_without_storage_identifiers() {
    let mut response = export_message(
        StoredRole::Assistant,
        "Rust owns trust.",
        Some("Check the IPC boundary."),
        MessageState::Cancelled,
    );
    response.provider_run = Some(StoredProviderRun {
        id: "native-run-id".into(),
        state: ProviderRunState::Cancelled,
        reasoning_effort: StoredReasoningEffort::Low,
        started_at_ms: 2,
        completed_at_ms: Some(3),
        error_code: None,
        usage: Some(StoredUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            cost_usd: None,
        }),
        tool_invocations: vec![StoredToolInvocation {
            ordinal: 0,
            tool_name: "search_memory".into(),
            arguments: json!({"query": "trust boundary"}),
            result: Some(super::tools::StoredToolResult {
                output: json!({"matches": 2}),
                is_error: false,
                created_at_ms: 4,
            }),
            created_at_ms: 3,
        }],
    });
    let conversation = StoredConversation {
        id: "conversation-id".into(),
        title: "Architecture & safety".into(),
        current_branch_id: "branch-id".into(),
        branches: vec![ConversationBranch {
            id: "branch-id".into(),
            name: "Main".into(),
        }],
        messages: vec![
            export_message(
                StoredRole::User,
                "Explain the **boundary**.",
                None,
                MessageState::Final,
            ),
            response,
        ],
    };

    let rendered = render_conversation_json(&conversation).expect("JSON should serialize");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("JSON should parse");

    assert_eq!(value["format"], "bottie-conversation");
    assert_eq!(value["version"], 2);
    assert_eq!(value["title"], "Architecture & safety");
    assert_eq!(value["messages"][0]["role"], "user");
    assert_eq!(value["messages"][0]["text"], "Explain the **boundary**.");
    assert_eq!(value["messages"][1]["reasoning"], "Check the IPC boundary.");
    assert_eq!(value["messages"][1]["state"], "cancelled");
    assert_eq!(value["messages"][1]["providerId"], "ollama");
    assert_eq!(value["messages"][1]["modelId"], "qwen3:latest");
    assert_eq!(value["messages"][1]["rating"], "good");
    assert_eq!(value["messages"][1]["createdAtMs"], 1);
    assert_eq!(value["messages"][1]["generation"]["state"], "cancelled");
    assert_eq!(value["messages"][1]["generation"]["reasoningEffort"], "low");
    assert_eq!(
        value["messages"][1]["generation"]["usage"]["inputTokens"],
        10
    );
    assert_eq!(
        value["messages"][1]["generation"]["toolInvocations"][0]["toolName"],
        "search_memory"
    );
    assert_eq!(
        value["messages"][1]["generation"]["toolInvocations"][0]["arguments"]["query"],
        "trust boundary"
    );
    assert_eq!(
        value["messages"][1]["generation"]["toolInvocations"][0]["result"]["output"]["matches"],
        2
    );
    assert!(value.get("id").is_none());
    assert!(value.get("currentBranchId").is_none());
    assert!(value["messages"][0].get("id").is_none());
    assert!(value["messages"][1]["generation"].get("id").is_none());
    assert!(
        value["messages"][1]["generation"]["toolInvocations"][0]
            .get("providerCallId")
            .is_none()
    );
    assert!(rendered.ends_with('\n'));
}

#[test]
fn builds_a_json_filename_and_keeps_the_selected_lineage_policy() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("../../Plans: Q3?")
        .expect("conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Original request".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("request should append");
    store
        .fork_from_user_message(&conversation.id, &request.id, "Selected request")
        .expect("selected branch should fork");

    let export = store
        .prepare_json_export(&conversation.id)
        .expect("selected lineage should export");
    let direct = json_export(
        &store
            .load_conversation(&conversation.id)
            .expect("selected conversation should load"),
    )
    .expect("direct JSON export should serialize");

    assert_eq!(export.file_name, "bottie-plans-q3.json");
    assert_eq!(export.contents, direct.contents);
    assert!(export.contents.contains("Selected request"));
    assert!(!export.contents.contains("Original request"));
}

#[test]
fn batches_active_and_archived_selected_lineages_without_trash_or_opaque_ids() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let active = store
        .create_conversation("Active notes")
        .expect("active conversation should be created");
    let original = store
        .append_message(NewStoredMessage {
            conversation_id: active.id.clone(),
            role: StoredRole::User,
            text: "Hidden original request".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("original request should append");
    store
        .fork_from_user_message(&active.id, &original.id, "Selected active request")
        .expect("selected active branch should fork");

    let archived = store
        .create_conversation("Archived notes")
        .expect("archived conversation should be created");
    store
        .append_message(NewStoredMessage {
            conversation_id: archived.id.clone(),
            role: StoredRole::User,
            text: "Retained archived request".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("archived request should append");
    store
        .set_conversation_archived(&archived.id, true)
        .expect("conversation should archive");

    let deleted = store
        .create_conversation("Deleted notes")
        .expect("deleted conversation should be created");
    store
        .append_message(NewStoredMessage {
            conversation_id: deleted.id.clone(),
            role: StoredRole::User,
            text: "Trash should stay excluded".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("deleted request should append");
    store
        .delete_conversation(&deleted.id)
        .expect("conversation should move to trash");
    store
        .open_conversation(&active.id)
        .expect("active conversation should be selected");

    let export = store
        .prepare_batch_json_export()
        .expect("batch JSON should export");
    let value: serde_json::Value =
        serde_json::from_str(&export.contents).expect("batch JSON should parse");
    let selected = store
        .load_last_open_conversation()
        .expect("selection should load")
        .expect("the active conversation should remain selected");

    assert_eq!(export.file_name, "bottie-conversations.json");
    assert_eq!(value["format"], "bottie-conversation-batch");
    assert_eq!(value["version"], 2);
    assert_eq!(value["conversations"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["conversations"][0]["title"], "Active notes");
    assert_eq!(value["conversations"][0]["lifecycle"], "active");
    assert_eq!(
        value["conversations"][0]["messages"][0]["text"],
        "Selected active request"
    );
    assert_eq!(value["conversations"][1]["title"], "Archived notes");
    assert_eq!(value["conversations"][1]["lifecycle"], "archived");
    assert!(value["conversations"][0].get("updatedAtMs").is_some());
    assert!(value["conversations"][0].get("id").is_none());
    assert!(!export.contents.contains("Hidden original request"));
    assert!(!export.contents.contains("Trash should stay excluded"));
    assert!(!export.contents.contains(&active.id));
    assert!(!export.contents.contains(&archived.id));
    assert!(export.contents.ends_with('\n'));
    assert_eq!(selected.id, active.id);
}

#[test]
fn rejects_a_batch_export_when_only_trashed_conversations_exist() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let deleted = store
        .create_conversation("Deleted notes")
        .expect("conversation should be created");
    store
        .delete_conversation(&deleted.id)
        .expect("conversation should move to trash");

    let error = match store.prepare_batch_json_export() {
        Ok(_) => panic!("trash alone should not produce a batch export"),
        Err(error) => error,
    };

    assert_eq!(error.code, "not_found");
    assert_eq!(
        error.message,
        "There are no active or archived conversations to export."
    );
}
