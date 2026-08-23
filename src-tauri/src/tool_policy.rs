//! Fail-closed execution policy for provider-requested native tools.

use serde_json::Value;

use crate::{
    storage::{OPEN_MEMORY_TOOL_NAME, SEARCH_ATTACHED_FILES_TOOL_NAME, SEARCH_MEMORY_TOOL_NAME},
    tool_contract::{CURRENT_TIME_TOOL_NAME, OPEN_EMAIL_TOOL_NAME, SEARCH_EMAIL_TOOL_NAME},
    tool_loop::NativeToolCall,
    web_fetch::WEB_FETCH_TOOL_NAME,
    web_search::WEB_SEARCH_TOOL_NAME,
};

/// Native user-consent requirement applied before argument validation or tool execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolExecutionPolicy {
    /// The explicitly enabled tool is bounded and read-only, so it can run without a per-call prompt.
    Safe,
    /// The tool can affect external or sensitive state and needs an exact native approval grant.
    #[allow(
        dead_code,
        reason = "no approval-required native tool is registered yet"
    )]
    ApprovalRequired,
}

/// Exact native authorization for one provider-requested call.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ApprovedToolCall {
    call_id: String,
    tool_name: String,
    arguments: Value,
}

impl ApprovedToolCall {
    /// Captures the exact call identity, tool, and arguments approved by a trusted native flow.
    #[allow(
        dead_code,
        reason = "the native approval prompt is a later bounded slice"
    )]
    pub(crate) fn for_call(call: &NativeToolCall) -> Self {
        Self {
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            arguments: call.arguments.clone(),
        }
    }

    /// Rejects replay when any provider-controlled part of the approved call has changed.
    fn matches(&self, call: &NativeToolCall) -> bool {
        self.call_id == call.call_id
            && self.tool_name == call.tool_name
            && self.arguments == call.arguments
    }
}

/// Proof that one exact provider request passed Bottie's native execution policy.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthorizedToolCall<'a> {
    call: &'a NativeToolCall,
}

impl<'a> AuthorizedToolCall<'a> {
    /// Returns the immutable call that may now pass into strict validation and dispatch.
    pub(crate) fn call(self) -> &'a NativeToolCall {
        self.call
    }
}

/// Stable failure category for native execution-policy enforcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolPolicyErrorCode {
    /// The requested tool has no explicit native policy entry.
    UnsupportedTool,
    /// An approval-required call had no exact trusted native grant.
    ApprovalRequired,
}

/// Redacted policy failure that never reflects provider-controlled call data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolPolicyError {
    /// Stable machine-readable policy category.
    pub(crate) code: ToolPolicyErrorCode,
    /// Fixed safe explanation for provider-facing error mapping.
    pub(crate) message: &'static str,
}

/// Returns the explicit policy for one registered native tool, or none for an unknown name.
pub(crate) fn tool_execution_policy(tool_name: &str) -> Option<ToolExecutionPolicy> {
    match tool_name {
        CURRENT_TIME_TOOL_NAME
        | SEARCH_MEMORY_TOOL_NAME
        | OPEN_MEMORY_TOOL_NAME
        | SEARCH_ATTACHED_FILES_TOOL_NAME
        | WEB_SEARCH_TOOL_NAME
        | WEB_FETCH_TOOL_NAME
        | SEARCH_EMAIL_TOOL_NAME
        | OPEN_EMAIL_TOOL_NAME => Some(ToolExecutionPolicy::Safe),
        _ => None,
    }
}

/// Authorizes one registered call against its native policy and an optional trusted grant.
pub(crate) fn authorize_tool_call<'a>(
    call: &'a NativeToolCall,
    approval: Option<ApprovedToolCall>,
) -> Result<AuthorizedToolCall<'a>, ToolPolicyError> {
    let Some(policy) = tool_execution_policy(&call.tool_name) else {
        return Err(policy_error(ToolPolicyErrorCode::UnsupportedTool));
    };
    authorize_tool_call_with_policy(call, policy, approval)
}

/// Applies a selected policy while binding any approval to the exact requested call.
pub(crate) fn authorize_tool_call_with_policy<'a>(
    call: &'a NativeToolCall,
    policy: ToolExecutionPolicy,
    approval: Option<ApprovedToolCall>,
) -> Result<AuthorizedToolCall<'a>, ToolPolicyError> {
    let allowed = match policy {
        ToolExecutionPolicy::Safe => true,
        ToolExecutionPolicy::ApprovalRequired => {
            approval.is_some_and(|approved| approved.matches(call))
        }
    };
    if allowed {
        Ok(AuthorizedToolCall { call })
    } else {
        Err(policy_error(ToolPolicyErrorCode::ApprovalRequired))
    }
}

/// Builds one fixed error without provider names, identities, arguments, or local details.
fn policy_error(code: ToolPolicyErrorCode) -> ToolPolicyError {
    let message = match code {
        ToolPolicyErrorCode::UnsupportedTool => {
            "The provider requested an unsupported native tool."
        }
        ToolPolicyErrorCode::ApprovalRequired => {
            "The native tool requires approval before it can run."
        }
    };
    ToolPolicyError { code, message }
}
