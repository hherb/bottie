//! Conversation Markdown-export contract tests.

use super::export::{markdown_export, render_conversation_markdown};
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
