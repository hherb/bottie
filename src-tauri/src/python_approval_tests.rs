//! Provider-neutral pending Python approval lifecycle tests.

use serde_json::{json, to_value};

use crate::{
    python_approval::{
        ConsumedPythonApproval, PythonApprovalController, PythonApprovalDecision,
        PythonApprovalDecisionRequest, PythonApprovalErrorCode, PythonApprovalPhase,
    },
    tool_contract::RUN_PYTHON_TOOL_NAME,
    tool_loop::NativeToolCall,
    tool_policy::authorize_tool_call,
};

/// Builds one exact bounded Python call for lifecycle tests.
fn python_call(call_id: &str, source: &str, purpose: &str) -> NativeToolCall {
    NativeToolCall {
        call_id: call_id.into(),
        tool_name: RUN_PYTHON_TOOL_NAME.into(),
        arguments: json!({"source": source, "purpose": purpose}),
    }
}

#[test]
fn exposes_only_one_bounded_pending_review_with_an_opaque_request_token() {
    let controller = PythonApprovalController::default();
    let call = python_call(
        "provider-secret-call-id",
        "print(sum([2, 3, 5]))",
        "Add the values exactly.",
    );

    let pending = controller
        .request(call)
        .expect("a bounded Python proposal should become pending");
    assert_eq!(pending.phase, PythonApprovalPhase::Pending);
    assert_eq!(pending.source, "print(sum([2, 3, 5]))");
    assert_eq!(pending.purpose, "Add the values exactly.");
    assert!(!pending.request_id.is_empty());
    assert_ne!(pending.request_id, "provider-secret-call-id");
    assert_eq!(controller.current(), Some(pending.clone()));

    let serialized = to_value(pending).expect("public approval status should serialize");
    assert_eq!(
        serialized
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["phase", "purpose", "requestId", "source"]
    );
    assert!(!serialized.to_string().contains("provider-secret-call-id"));
}

#[test]
fn rejects_invalid_or_competing_proposals_without_retaining_private_input() {
    let controller = PythonApprovalController::default();
    let private_source = "print('private value')";
    let malformed = NativeToolCall {
        call_id: "invalid-call".into(),
        tool_name: RUN_PYTHON_TOOL_NAME.into(),
        arguments: json!({"source": private_source, "purpose": " "}),
    };

    let invalid = controller
        .request(malformed)
        .expect_err("malformed Python arguments must fail closed");
    assert_eq!(invalid.code, PythonApprovalErrorCode::InvalidRequest);
    assert!(!invalid.message.contains(private_source));
    assert_eq!(controller.current(), None);

    controller
        .request(python_call("first", "print(1)", "Print one."))
        .expect("the first proposal should become pending");
    let competing = controller
        .request(python_call("second", "print(2)", "Print two."))
        .expect_err("a second proposal must not replace the pending call");
    assert_eq!(competing.code, PythonApprovalErrorCode::RequestPending);
    assert!(!competing.message.contains("second"));
    assert_eq!(controller.current().unwrap().source, "print(1)");
}

#[test]
fn approval_is_single_decision_single_use_and_bound_to_the_complete_call() {
    let controller = PythonApprovalController::default();
    let call = python_call("python-call", "print(4)", "Calculate two plus two.");
    let pending = controller.request(call.clone()).unwrap();

    let approved = controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect("the exact pending token should approve once");
    assert_eq!(approved.phase, PythonApprovalPhase::Approved);
    let repeated = controller
        .decide(&pending.request_id, PythonApprovalDecision::Deny)
        .expect_err("an approval decision must not be changed");
    assert_eq!(repeated.code, PythonApprovalErrorCode::AlreadyDecided);

    for changed in [
        python_call("changed-call", "print(4)", "Calculate two plus two."),
        python_call("python-call", "print(5)", "Calculate two plus two."),
        python_call(
            "python-call",
            "print(4)",
            "Explain a different calculation.",
        ),
    ] {
        let mismatch = controller
            .take_decision(&changed)
            .expect_err("a changed call must not consume the approval");
        assert_eq!(mismatch.code, PythonApprovalErrorCode::CallMismatch);
        assert_eq!(
            controller.current().unwrap().phase,
            PythonApprovalPhase::Approved
        );
    }

    let ConsumedPythonApproval::Approved(grant) = controller
        .take_decision(&call)
        .expect("the exact call should consume its decision")
        .expect("the approved decision should be ready")
    else {
        panic!("the decision should carry an exact approval grant");
    };
    assert!(authorize_tool_call(&call, Some(grant)).is_ok());
    assert_eq!(controller.current(), None);
    assert!(controller.take_decision(&call).unwrap().is_none());
}

#[test]
fn denial_is_terminal_single_use_and_never_produces_an_approval_grant() {
    let controller = PythonApprovalController::default();
    let call = python_call("python-call", "print(4)", "Calculate two plus two.");
    let pending = controller.request(call.clone()).unwrap();

    let denied = controller
        .decide(&pending.request_id, PythonApprovalDecision::Deny)
        .expect("the exact pending token should deny once");
    assert_eq!(denied.phase, PythonApprovalPhase::Denied);
    assert_eq!(
        controller.take_decision(&call).unwrap(),
        Some(ConsumedPythonApproval::Denied)
    );
    assert_eq!(controller.current(), None);
}

#[test]
fn rejects_unknown_or_stale_webview_tokens_without_reflecting_them() {
    let controller = PythonApprovalController::default();
    let pending = controller
        .request(python_call("python-call", "print(4)", "Calculate exactly."))
        .unwrap();
    let private_token = "provider-controlled-token";

    let unknown = controller
        .decide(private_token, PythonApprovalDecision::Approve)
        .expect_err("only the native process-local token may resolve a request");
    assert_eq!(unknown.code, PythonApprovalErrorCode::RequestNotFound);
    assert!(!unknown.message.contains(private_token));
    assert_eq!(controller.current(), Some(pending));
}

#[test]
fn webview_decisions_accept_only_the_closed_token_and_decision_shape() {
    let valid = serde_json::from_value::<PythonApprovalDecisionRequest>(json!({
        "requestId": "opaque-native-token",
        "decision": "approve"
    }))
    .expect("the closed decision request should deserialize");
    assert_eq!(valid.request_id, "opaque-native-token");
    assert_eq!(valid.decision, PythonApprovalDecision::Approve);

    for malformed in [
        json!({"requestId": "opaque-native-token"}),
        json!({"requestId": "opaque-native-token", "decision": "allow"}),
        json!({"requestId": "opaque-native-token", "decision": "deny", "source": "print(4)"}),
    ] {
        assert!(serde_json::from_value::<PythonApprovalDecisionRequest>(malformed).is_err());
    }
}
