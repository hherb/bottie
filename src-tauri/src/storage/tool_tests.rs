//! Append-oriented tool invocation and result persistence tests.

use serde_json::json;

use super::tests::test_database_path;
use super::tools::{NewToolInvocation, NewToolResult, ToolAuditOutcome, ToolAuditPolicy};
use super::*;

/// Starts one provider run that can own durable tool activity.
fn started_run(store: &ConversationStore) -> (StoredConversation, String) {
    let conversation = store
        .create_conversation("Durable tool activity")
        .expect("conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Find the relevant note".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("request should be stored");
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: request.id,
            provider_id: "ollama".into(),
            model_id: "tool-model".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: Some(512),
        })
        .expect("provider run should start");
    (conversation, run_id)
}

#[test]
fn appends_tool_invocations_and_results_in_provider_order() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let (conversation, run_id) = started_run(&store);

    store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-search".into(),
            tool_name: "search_memory".into(),
            arguments: json!({"query": "release checklist"}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect("first tool invocation should append");
    store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-search".into(),
            output: json!({"matches": [{"title": "Release notes"}]}),
            is_error: false,
            audit_outcome: ToolAuditOutcome::Success,
            duration_ms: 12,
        })
        .expect("tool result should append");
    store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-open".into(),
            tool_name: "open_memory".into(),
            arguments: json!({"message": 42}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect("second tool invocation should append");
    store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-failed".into(),
            tool_name: "open_memory".into(),
            arguments: json!({"message": 999}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect("third tool invocation should append");
    store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-failed".into(),
            output: json!({"code": "not_found"}),
            is_error: true,
            audit_outcome: ToolAuditOutcome::Unavailable,
            duration_ms: 3,
        })
        .expect("error result should append");
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("provider run should complete");
    drop(store);

    let reopened = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let tools = &reopened.messages[1]
        .provider_run
        .as_ref()
        .expect("assistant response should retain its run")
        .tool_invocations;

    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0].ordinal, 0);
    assert_eq!(tools[0].tool_name, "search_memory");
    assert_eq!(tools[0].arguments, json!({"query": "release checklist"}));
    assert_eq!(tools[0].audit.policy, ToolAuditPolicy::Safe);
    assert_eq!(tools[0].audit.outcome, Some(ToolAuditOutcome::Success));
    assert_eq!(tools[0].audit.duration_ms, Some(12));
    assert_eq!(
        tools[0].result.as_ref().map(|result| &result.output),
        Some(&json!({"matches": [{"title": "Release notes"}]}))
    );
    assert!(
        !tools[0]
            .result
            .as_ref()
            .expect("result should load")
            .is_error
    );
    assert_eq!(tools[1].ordinal, 1);
    assert_eq!(tools[1].tool_name, "open_memory");
    assert!(tools[1].result.is_none());
    assert_eq!(tools[1].audit.outcome, None);
    assert_eq!(tools[1].audit.duration_ms, None);
    assert_eq!(tools[2].ordinal, 2);
    assert!(
        tools[2]
            .result
            .as_ref()
            .expect("error should load")
            .is_error
    );
    assert_eq!(
        tools[2].result.as_ref().map(|result| &result.output),
        Some(&json!({"code": "not_found"}))
    );
    assert_eq!(tools[2].audit.outcome, Some(ToolAuditOutcome::Unavailable));
    assert_eq!(tools[2].audit.duration_ms, Some(3));
}

#[test]
fn upgrades_existing_tool_rows_with_honest_legacy_audit_metadata() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let (conversation, run_id) = started_run(&store);
    store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "legacy-call".into(),
            tool_name: "search_memory".into(),
            arguments: json!({"query": "legacy"}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect("tool call should append");
    store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id.clone(),
            provider_call_id: "legacy-call".into(),
            output: json!({"code": "old_error"}),
            is_error: true,
            audit_outcome: ToolAuditOutcome::ExecutionFailed,
            duration_ms: 9,
        })
        .expect("tool result should append");
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("run should complete");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE tool_approvals;
             ALTER TABLE tool_invocations DROP COLUMN execution_policy;
             ALTER TABLE tool_results DROP COLUMN outcome_code;
             ALTER TABLE tool_results DROP COLUMN duration_ms;
             DELETE FROM schema_migrations WHERE version > 20;
             PRAGMA user_version = 20;",
        )
        .expect("audit columns should be removable for the migration fixture");
    drop(connection);
    drop(store);

    let reopened = ConversationStore::initialize(path)
        .expect("version twenty store should upgrade")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let audit = &reopened.messages[1]
        .provider_run
        .as_ref()
        .expect("run should load")
        .tool_invocations[0]
        .audit;

    assert_eq!(audit.policy, ToolAuditPolicy::Legacy);
    assert_eq!(audit.approval, None);
    assert_eq!(audit.outcome, Some(ToolAuditOutcome::LegacyError));
    assert_eq!(audit.duration_ms, None);
}

#[test]
fn rejects_duplicate_calls_results_and_terminal_run_appends() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let (_conversation, run_id) = started_run(&store);
    let invocation = NewToolInvocation {
        provider_run_id: run_id.clone(),
        provider_call_id: "call-1".into(),
        tool_name: "search_memory".into(),
        arguments: json!({"query": "SQLite"}),
        audit_policy: ToolAuditPolicy::Safe,
    };
    store
        .checkpoint_tool_invocation(invocation.clone())
        .expect("tool invocation should append");
    let duplicate_call = store
        .checkpoint_tool_invocation(invocation)
        .expect_err("provider call identities must be unique inside one run");
    let result = NewToolResult {
        provider_run_id: run_id.clone(),
        provider_call_id: "call-1".into(),
        output: json!({"matches": []}),
        is_error: false,
        audit_outcome: ToolAuditOutcome::Success,
        duration_ms: 1,
    };
    store
        .checkpoint_tool_result(result.clone())
        .expect("first result should append");
    let duplicate_result = store
        .checkpoint_tool_result(result)
        .expect_err("one invocation cannot receive two results");
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("provider run should complete");
    let terminal_append = store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-late".into(),
            tool_name: "open_memory".into(),
            arguments: json!({}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect_err("terminal runs must reject new tool records");

    assert_eq!(duplicate_call.code, "invalid_request");
    assert_eq!(duplicate_result.code, "invalid_request");
    assert_eq!(terminal_append.code, "not_found");
}

#[test]
fn validates_tool_names_arguments_and_result_linkage() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let (_conversation, run_id) = started_run(&store);

    let empty_name = store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-empty".into(),
            tool_name: "  ".into(),
            arguments: json!({}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect_err("empty tool names should be rejected");
    let non_object_arguments = store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-array".into(),
            tool_name: "search_memory".into(),
            arguments: json!(["not", "an", "object"]),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect_err("tool arguments should be JSON objects");
    let missing_call = store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id.clone(),
            provider_call_id: "unknown-call".into(),
            output: json!(null),
            is_error: true,
            audit_outcome: ToolAuditOutcome::Unavailable,
            duration_ms: 0,
        })
        .expect_err("results must link to a retained invocation");
    let oversized_arguments = store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-large".into(),
            tool_name: "search_memory".into(),
            arguments: json!({"query": "x".repeat(1_048_577)}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect_err("oversized tool arguments should be rejected");
    store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: run_id.clone(),
            provider_call_id: "call-mismatch".into(),
            tool_name: "search_memory".into(),
            arguments: json!({}),
            audit_policy: ToolAuditPolicy::Safe,
        })
        .expect("mismatch fixture call should append");
    let mismatched_audit = store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id,
            provider_call_id: "call-mismatch".into(),
            output: json!({"ok": true}),
            is_error: false,
            audit_outcome: ToolAuditOutcome::ExecutionFailed,
            duration_ms: 1,
        })
        .expect_err("result status and audit outcome must agree");

    assert_eq!(empty_name.code, "invalid_request");
    assert_eq!(non_object_arguments.code, "invalid_request");
    assert_eq!(missing_call.code, "not_found");
    assert_eq!(oversized_arguments.code, "invalid_request");
    assert_eq!(mismatched_audit.code, "invalid_request");
}

#[test]
fn accepts_only_one_consistent_approval_before_an_approval_required_result() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let (conversation, run_id) = started_run(&store);
    for (call_id, policy) in [
        ("approval-call", ToolAuditPolicy::ApprovalRequired),
        ("safe-call", ToolAuditPolicy::Safe),
        ("denied-call", ToolAuditPolicy::ApprovalRequired),
        ("undecided-call", ToolAuditPolicy::ApprovalRequired),
    ] {
        store
            .checkpoint_tool_invocation(NewToolInvocation {
                provider_run_id: run_id.clone(),
                provider_call_id: call_id.into(),
                tool_name: "run_python".into(),
                arguments: json!({"source": "print(1)", "purpose": "Test approval."}),
                audit_policy: policy,
            })
            .expect("tool invocation should append");
    }
    let approval = NewToolApproval {
        provider_run_id: run_id.clone(),
        provider_call_id: "approval-call".into(),
        decision: ToolApprovalDecision::Approved,
    };
    store
        .checkpoint_tool_approval(approval.clone())
        .expect("first approval should append");
    let duplicate = store
        .checkpoint_tool_approval(approval)
        .expect_err("an approval must be immutable");
    let safe_approval = store
        .checkpoint_tool_approval(NewToolApproval {
            provider_run_id: run_id.clone(),
            provider_call_id: "safe-call".into(),
            decision: ToolApprovalDecision::Approved,
        })
        .expect_err("safe calls must not accept approval records");
    store
        .checkpoint_tool_approval(NewToolApproval {
            provider_run_id: run_id.clone(),
            provider_call_id: "denied-call".into(),
            decision: ToolApprovalDecision::Denied,
        })
        .expect("denial should append");
    let contradictory = store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id.clone(),
            provider_call_id: "denied-call".into(),
            output: json!({"status": "executed"}),
            is_error: false,
            audit_outcome: ToolAuditOutcome::Success,
            duration_ms: 1,
        })
        .expect_err("a denied call must not retain a successful result");
    let unapproved = store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: run_id.clone(),
            provider_call_id: "undecided-call".into(),
            output: json!({"status": "executed"}),
            is_error: false,
            audit_outcome: ToolAuditOutcome::Success,
            duration_ms: 1,
        })
        .expect_err("an undecided approval-required call must not succeed");
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .unwrap();

    let reopened = store.load_conversation(&conversation.id).unwrap();
    let approval = reopened.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0]
        .audit
        .approval
        .as_ref()
        .expect("approval should reopen");
    assert_eq!(approval.decision, ToolApprovalDecision::Approved);
    assert!(approval.decided_at_ms > 0);
    assert_eq!(duplicate.code, "invalid_request");
    assert_eq!(safe_approval.code, "invalid_request");
    assert_eq!(contradictory.code, "invalid_request");
    assert_eq!(unapproved.code, "invalid_request");
}

#[test]
fn upgrades_version_six_stores_with_empty_tool_tables() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let connection = store.open().expect("database should open");
    connection
        .execute_batch(
            "DROP TABLE tool_approvals;
             DROP TABLE conversation_retention_policies;
             DROP TABLE conversation_memory_preferences;
             DROP TABLE conversation_attachments;
             DROP TABLE attachment_text_indexing;
             DROP TABLE attachment_image_normalizations;
             DROP TABLE attachment_extractions;
             DROP TABLE message_attachments; DROP TABLE attachments;
             DROP TABLE tool_results; DROP TABLE tool_invocations;",
        )
        .expect("post-version-six tables should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 6", [])
        .expect("newer migration records should be removable in the fixture");
    connection
        .pragma_update(None, "user_version", 6)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version six store should upgrade");
    let connection = upgraded.open().expect("upgraded database should open");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table'
               AND name IN ('tool_invocations', 'tool_approvals', 'tool_results')",
            [],
            |row| row.get(0),
        )
        .expect("tool tables should be queryable");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        22
    );
    assert_eq!(table_count, 3);
}
