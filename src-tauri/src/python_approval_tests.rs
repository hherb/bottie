//! Provider-neutral pending Python approval lifecycle tests.

use std::{sync::Arc, time::Duration};

use serde_json::{json, to_value};

use crate::{
    python_approval::{
        ConsumedPythonApproval, PythonApprovalController, PythonApprovalDecision,
        PythonApprovalDecisionRequest, PythonApprovalErrorCode, PythonApprovalPhase,
        PythonApprovalResolution,
    },
    tool_contract::RUN_PYTHON_TOOL_NAME,
    tool_loop::{NativeToolCall, ToolLoopCancellation},
    tool_policy::authorize_tool_call,
};

/// Maximum time an async test waits for the spawned orchestration to publish its review.
const PENDING_REVIEW_TIMEOUT: Duration = Duration::from_secs(1);

/// Builds one exact bounded Python call for lifecycle tests.
fn python_call(call_id: &str, source: &str, purpose: &str) -> NativeToolCall {
    NativeToolCall {
        call_id: call_id.into(),
        tool_name: RUN_PYTHON_TOOL_NAME.into(),
        arguments: json!({"source": source, "purpose": purpose}),
    }
}

/// Waits for a spawned orchestration task to publish its review without assuming scheduler order.
async fn pending_review(
    controller: &PythonApprovalController,
) -> crate::python_approval::PythonApprovalStatus {
    tokio::time::timeout(PENDING_REVIEW_TIMEOUT, async {
        loop {
            if let Some(status) = controller.current() {
                return status;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("provider-neutral orchestration should publish one pending review")
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

#[tokio::test]
async fn waits_for_approval_then_resumes_with_one_exact_grant() {
    let controller = Arc::new(PythonApprovalController::default());
    let cancellation = ToolLoopCancellation::default();
    let call = python_call("python-call", "print(4)", "Calculate two plus two.");
    let waiting_controller = controller.clone();
    let waiting_cancellation = cancellation.clone();
    let waiting_call = call.clone();
    let waiter = tokio::spawn(async move {
        waiting_controller
            .request_and_wait(waiting_call, &waiting_cancellation)
            .await
    });

    let pending = pending_review(&controller).await;
    assert!(!waiter.is_finished());
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect("the exact opaque token should wake the waiter");

    let PythonApprovalResolution::Approved(grant) = waiter.await.unwrap().unwrap() else {
        panic!("approval should resume with one exact native grant");
    };
    assert!(authorize_tool_call(&call, Some(grant)).is_ok());
    assert_eq!(controller.current(), None);
}

#[tokio::test]
async fn denial_wakes_the_waiter_as_a_terminal_non_execution_path() {
    let controller = Arc::new(PythonApprovalController::default());
    let cancellation = ToolLoopCancellation::default();
    let call = python_call("python-call", "print(4)", "Calculate two plus two.");
    let waiting_controller = controller.clone();
    let waiting_cancellation = cancellation.clone();
    let waiter = tokio::spawn(async move {
        waiting_controller
            .request_and_wait(call, &waiting_cancellation)
            .await
    });

    let pending = pending_review(&controller).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Deny)
        .expect("denial should resolve the exact pending proposal");

    assert_eq!(
        waiter.await.unwrap().unwrap(),
        PythonApprovalResolution::Denied
    );
    assert_eq!(controller.current(), None);
}

#[tokio::test]
async fn shared_cancellation_wakes_the_waiter_and_releases_the_slot() {
    let controller = Arc::new(PythonApprovalController::default());
    let cancellation = ToolLoopCancellation::default();
    let waiting_controller = controller.clone();
    let waiting_cancellation = cancellation.clone();
    let waiter = tokio::spawn(async move {
        waiting_controller
            .request_and_wait(
                python_call("cancelled-call", "print(4)", "Calculate exactly."),
                &waiting_cancellation,
            )
            .await
    });

    let pending = pending_review(&controller).await;
    cancellation.cancel();

    assert_eq!(
        waiter.await.unwrap().unwrap(),
        PythonApprovalResolution::Cancelled
    );
    assert_eq!(controller.current(), None);
    let stale = controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect_err("cancellation must make the old decision token stale");
    assert_eq!(stale.code, PythonApprovalErrorCode::RequestNotFound);
    assert!(
        controller
            .request(python_call("next-call", "print(5)", "Print five."))
            .is_ok(),
        "cancellation should release the one-proposal slot"
    );
}

#[tokio::test]
async fn aborting_the_waiter_releases_the_pending_slot() {
    let controller = Arc::new(PythonApprovalController::default());
    let waiting_controller = controller.clone();
    let waiter = tokio::spawn(async move {
        waiting_controller
            .request_and_wait(
                python_call("aborted-call", "print(4)", "Calculate exactly."),
                &ToolLoopCancellation::default(),
            )
            .await
    });

    pending_review(&controller).await;
    waiter.abort();
    assert!(
        waiter
            .await
            .expect_err("the waiter should be aborted")
            .is_cancelled()
    );

    assert_eq!(controller.current(), None);
    assert!(
        controller
            .request(python_call("next-call", "print(5)", "Print five."))
            .is_ok(),
        "an aborted waiter should release the one-proposal slot"
    );
}

#[tokio::test]
async fn cancellation_before_a_request_never_publishes_a_review() {
    let controller = PythonApprovalController::default();
    let cancellation = ToolLoopCancellation::default();
    cancellation.cancel();

    assert_eq!(
        controller
            .request_and_wait(
                python_call("cancelled-call", "print(4)", "Calculate exactly."),
                &cancellation,
            )
            .await
            .unwrap(),
        PythonApprovalResolution::Cancelled
    );
    assert_eq!(controller.current(), None);
}
