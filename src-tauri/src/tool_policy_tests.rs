//! Provider-independent native tool execution-policy tests.

use serde_json::json;

use crate::{
    tool_contract::{
        memory_tool_definitions, web_fetch_tool_definition, web_search_tool_definition,
    },
    tool_dispatch::{MemoryToolExecution, MemoryToolExecutionErrorCode, policy_error},
    tool_loop::NativeToolCall,
    tool_policy::{
        ApprovedToolCall, ToolExecutionPolicy, ToolPolicyErrorCode, authorize_tool_call,
        authorize_tool_call_with_policy, tool_execution_policy,
    },
};

/// Builds one provider-neutral call for exact policy and approval matching.
fn call(call_id: &str, tool_name: &str, arguments: serde_json::Value) -> NativeToolCall {
    NativeToolCall {
        call_id: call_id.into(),
        tool_name: tool_name.into(),
        arguments,
    }
}

#[test]
fn explicitly_classifies_every_advertised_memory_tool_as_safe() {
    let definitions = memory_tool_definitions();
    assert_eq!(definitions.len(), 3);
    for definition in definitions {
        assert_eq!(
            tool_execution_policy(definition.name),
            Some(ToolExecutionPolicy::Safe)
        );
        let call = call("provider-call", definition.name, json!({}));
        let authorized = authorize_tool_call(&call, None).expect("safe tool should not prompt");
        assert_eq!(authorized.call(), &call);
    }
}

#[test]
fn classifies_the_explicit_web_search_contract_as_safe() {
    let definition = web_search_tool_definition();
    assert_eq!(
        tool_execution_policy(definition.name),
        Some(ToolExecutionPolicy::Safe)
    );
    let call = call(
        "provider-call",
        definition.name,
        json!({"query": "current Bottie release"}),
    );
    assert!(authorize_tool_call(&call, None).is_ok());
}

#[test]
fn classifies_the_bounded_public_web_fetch_contract_as_safe() {
    let definition = web_fetch_tool_definition();
    assert_eq!(
        tool_execution_policy(definition.name),
        Some(ToolExecutionPolicy::Safe)
    );
    let call = call(
        "provider-call",
        definition.name,
        json!({"url": "https://www.iana.org/release"}),
    );
    assert!(authorize_tool_call(&call, None).is_ok());
}

#[test]
fn fails_closed_for_tools_without_an_explicit_policy() {
    let call = call(
        "provider-call",
        "unregistered_tool",
        json!({"path": "/private"}),
    );
    let error = authorize_tool_call(&call, None).expect_err("unknown tool should be rejected");

    assert_eq!(error.code, ToolPolicyErrorCode::UnsupportedTool);
    assert_eq!(
        error.message,
        "The provider requested an unsupported native tool."
    );
    assert!(!error.message.contains("unregistered_tool"));
    assert!(!error.message.contains("/private"));
}

#[test]
fn approval_required_policy_accepts_only_an_exact_native_grant() {
    let requested_call = call(
        "provider-call",
        "future_host_tool",
        json!({"target": "exact value"}),
    );
    let missing = authorize_tool_call_with_policy(
        &requested_call,
        ToolExecutionPolicy::ApprovalRequired,
        None,
    )
    .expect_err("approval-required calls should not run unattended");
    assert_eq!(missing.code, ToolPolicyErrorCode::ApprovalRequired);
    let execution = policy_error(missing);
    let serialized = serde_json::to_value(&execution).expect("policy error should serialize");
    assert_eq!(serialized["error"]["code"], json!("approval_required"));
    let MemoryToolExecution::Error { error } = execution else {
        panic!("missing approval should produce an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::ApprovalRequired);
    assert!(!error.message.contains("exact value"));

    let grant = ApprovedToolCall::for_call(&requested_call);
    assert!(
        authorize_tool_call_with_policy(
            &requested_call,
            ToolExecutionPolicy::ApprovalRequired,
            Some(grant),
        )
        .is_ok()
    );

    for changed in [
        call(
            "different-call",
            &requested_call.tool_name,
            requested_call.arguments.clone(),
        ),
        call(
            &requested_call.call_id,
            "different_tool",
            requested_call.arguments.clone(),
        ),
        call(
            &requested_call.call_id,
            &requested_call.tool_name,
            json!({"target": "changed value"}),
        ),
    ] {
        let grant = ApprovedToolCall::for_call(&requested_call);
        let error = authorize_tool_call_with_policy(
            &changed,
            ToolExecutionPolicy::ApprovalRequired,
            Some(grant),
        )
        .expect_err("a grant must not authorize a changed call");
        assert_eq!(error.code, ToolPolicyErrorCode::ApprovalRequired);
    }
}
