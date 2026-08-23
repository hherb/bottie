//! Provider-neutral execution and bounded result envelopes for Bottie's native tools.

use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Value, json};

use crate::{
    credentials::CredentialStore,
    inference::{ProviderError, ProviderErrorCode},
    localmail::{open_email_native, search_email_native},
    storage::{ConversationStore, SemanticEmbedder, StorageError},
    tool_contract::{
        LocalmailToolArguments, MemoryToolArguments, ToolContractError, ToolContractErrorCode,
        validate_current_time_tool_arguments, validate_localmail_tool_arguments,
        validate_memory_tool_arguments, validate_web_fetch_tool_arguments,
        validate_web_search_tool_arguments,
    },
    tool_loop::NativeToolCall,
    tool_policy::{ApprovedToolCall, ToolPolicyError, ToolPolicyErrorCode, authorize_tool_call},
    web_fetch::{WebFetchError, WebFetchErrorCode, WebFetchProvider, blocked_by_user_policy},
    web_policy::WebNetworkPolicy,
    web_search::{WebSearchError, WebSearchErrorCode, WebSearchProvider},
};

/// Injected execution boundary shared by the configured connector and focused dispatcher tests.
#[allow(
    dead_code,
    reason = "provider-loop wiring is explicitly deferred to the next bounded Localmail slice"
)]
pub(crate) trait LocalmailToolExecutor {
    /// Executes one already validated exact connector request and returns path-free structured content.
    async fn execute(&self, arguments: LocalmailToolArguments) -> Result<Value, ProviderError>;
}

/// Production Localmail executor retaining configuration paths and credentials entirely inside Rust.
#[allow(
    dead_code,
    reason = "provider-loop wiring is explicitly deferred to the next bounded Localmail slice"
)]
pub(crate) struct ConfiguredLocalmailToolExecutor<'a> {
    config_path: &'a Path,
    credentials: &'a dyn CredentialStore,
}

#[allow(
    dead_code,
    reason = "provider-loop wiring is explicitly deferred to the next bounded Localmail slice"
)]
impl<'a> ConfiguredLocalmailToolExecutor<'a> {
    /// Binds one dispatcher execution to the current native Localmail configuration and vault.
    pub(crate) fn new(config_path: &'a Path, credentials: &'a dyn CredentialStore) -> Self {
        Self {
            config_path,
            credentials,
        }
    }
}

impl LocalmailToolExecutor for ConfiguredLocalmailToolExecutor<'_> {
    /// Executes only the selected existing fixed-route connector contract.
    async fn execute(&self, arguments: LocalmailToolArguments) -> Result<Value, ProviderError> {
        let result = match arguments {
            LocalmailToolArguments::SearchEmail(request) => serde_json::to_value(
                search_email_native(self.config_path, self.credentials, request).await?,
            ),
            LocalmailToolArguments::OpenEmail(request) => serde_json::to_value(
                open_email_native(self.config_path, self.credentials, request).await?,
            ),
        };
        result.map_err(|_| {
            ProviderError::internal("Bottie could not serialize the Localmail result.", None)
        })
    }
}

/// Executes the clock with Bottie's real native system time.
pub(crate) fn dispatch_current_time_tool(call: &NativeToolCall) -> MemoryToolExecution {
    dispatch_current_time_tool_with_clock(call, Utc::now)
}

/// Executes the clock through an injected time source for exact deterministic verification.
pub(crate) fn dispatch_current_time_tool_with_clock(
    call: &NativeToolCall,
    clock: impl FnOnce() -> DateTime<Utc>,
) -> MemoryToolExecution {
    let authorized = match authorize_tool_call(call, None) {
        Ok(authorized) => authorized,
        Err(error) => return policy_error(error),
    };
    if let Err(error) = validate_current_time_tool_arguments(
        &authorized.call().tool_name,
        &authorized.call().arguments,
    ) {
        return contract_error(error);
    }
    bounded_memory_tool_success(json!({
        "utc": clock().to_rfc3339_opts(SecondsFormat::Millis, true)
    }))
}

/// Maximum serialized size of one complete successful native tool envelope.
pub(crate) const MAX_MEMORY_TOOL_OUTPUT_BYTES: usize = 64 * 1_024;

/// Provider-neutral outcome of one validated native tool execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemoryToolExecution {
    /// One bounded, path-free tool result.
    Success {
        /// Tool-specific structured content.
        result: Value,
    },
    /// One stable redacted failure safe for later provider adapter mapping.
    Error {
        /// Structured execution failure.
        error: MemoryToolExecutionError,
    },
}

impl Serialize for MemoryToolExecution {
    /// Serializes success and error variants into one unambiguous common envelope.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Success { result } => {
                let mut state = serializer.serialize_struct("MemoryToolExecution", 2)?;
                state.serialize_field("ok", &true)?;
                state.serialize_field("result", result)?;
                state.end()
            }
            Self::Error { error } => {
                let mut state = serializer.serialize_struct("MemoryToolExecution", 2)?;
                state.serialize_field("ok", &false)?;
                state.serialize_field("error", error)?;
                state.end()
            }
        }
    }
}

/// Stable provider-neutral native-tool failure categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MemoryToolExecutionErrorCode {
    /// The requested name is outside Bottie's advertised native tool set.
    UnsupportedTool,
    /// Raw arguments failed the selected tool's closed native schema.
    InvalidArguments,
    /// The requested tool needs an exact trusted native approval grant.
    ApprovalRequired,
    /// Valid provenance no longer resolves under profile or lifecycle policy.
    Unavailable,
    /// Native storage or embedding work failed without exposing implementation details.
    ExecutionFailed,
    /// Structured success content exceeded Bottie's serialized output ceiling.
    OutputTooLarge,
}

/// Bounded redacted failure returned by the common native execution envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryToolExecutionError {
    /// Stable machine-readable category for provider-independent loop policy.
    pub(crate) code: MemoryToolExecutionErrorCode,
    /// Fixed safe explanation that never includes arguments, queries, paths, or storage details.
    pub(crate) message: &'static str,
}

/// Validates and executes one raw provider-style native memory-tool call.
pub(crate) fn dispatch_memory_tool(
    store: &ConversationStore,
    embedder: &mut impl SemanticEmbedder,
    call: &NativeToolCall,
    approval: Option<ApprovedToolCall>,
) -> MemoryToolExecution {
    let authorized = match authorize_tool_call(call, approval) {
        Ok(authorized) => authorized,
        Err(error) => return policy_error(error),
    };
    let call = authorized.call();
    let arguments = match validate_memory_tool_arguments(&call.tool_name, &call.arguments) {
        Ok(arguments) => arguments,
        Err(error) => return contract_error(error),
    };
    let result = match arguments {
        MemoryToolArguments::SearchMemory(arguments) => store
            .execute_search_memory(arguments, embedder)
            .and_then(serialized_result),
        MemoryToolArguments::OpenMemory(arguments) => store
            .execute_open_memory(arguments)
            .and_then(serialized_result),
        MemoryToolArguments::SearchAttachedFiles(arguments) => store
            .execute_search_attached_files(arguments, embedder)
            .and_then(serialized_result),
    };
    match result {
        Ok(result) => bounded_memory_tool_success(result),
        Err(error) => storage_error(error),
    }
}

/// Validates and executes one Localmail read through the common bounded native-tool envelope.
#[allow(
    dead_code,
    reason = "provider-loop wiring is explicitly deferred to the next bounded Localmail slice"
)]
pub(crate) async fn dispatch_localmail_tool(
    executor: &impl LocalmailToolExecutor,
    call: &NativeToolCall,
    approval: Option<ApprovedToolCall>,
) -> MemoryToolExecution {
    let authorized = match authorize_tool_call(call, approval) {
        Ok(authorized) => authorized,
        Err(error) => return policy_error(error),
    };
    let call = authorized.call();
    let arguments = match validate_localmail_tool_arguments(&call.tool_name, &call.arguments) {
        Ok(arguments) => arguments,
        Err(error) => return contract_error(error),
    };
    match executor.execute(arguments).await {
        Ok(result) => bounded_memory_tool_success(result),
        Err(error) => localmail_error(error),
    }
}

/// Validates and executes one raw provider-style web-search call through a selected native provider.
pub(crate) async fn dispatch_web_search_tool(
    provider: &impl WebSearchProvider,
    call: &NativeToolCall,
    network_policy: &WebNetworkPolicy,
    approval: Option<ApprovedToolCall>,
) -> MemoryToolExecution {
    let authorized = match authorize_tool_call(call, approval) {
        Ok(authorized) => authorized,
        Err(error) => return policy_error(error),
    };
    let call = authorized.call();
    let arguments = match validate_web_search_tool_arguments(&call.tool_name, &call.arguments) {
        Ok(arguments) => arguments,
        Err(error) => return contract_error(error),
    };
    let request = match arguments.into_request() {
        Ok(request) => request,
        Err(error) => return web_search_error(error),
    };
    match provider.search(request).await {
        Ok(response) => match serde_json::to_value(response.filtered_by(network_policy)) {
            Ok(result) => bounded_memory_tool_success(result),
            Err(_) => execution_error(
                MemoryToolExecutionErrorCode::ExecutionFailed,
                "Bottie could not serialize the native web-search result.",
            ),
        },
        Err(error) => web_search_error(error),
    }
}

/// Validates and executes one raw provider-style web-fetch call through native network policy.
pub(crate) async fn dispatch_web_fetch_tool(
    provider: &impl WebFetchProvider,
    call: &NativeToolCall,
    network_policy: &WebNetworkPolicy,
    approval: Option<ApprovedToolCall>,
) -> MemoryToolExecution {
    let authorized = match authorize_tool_call(call, approval) {
        Ok(authorized) => authorized,
        Err(error) => return policy_error(error),
    };
    let call = authorized.call();
    let arguments = match validate_web_fetch_tool_arguments(&call.tool_name, &call.arguments) {
        Ok(arguments) => arguments,
        Err(error) => return contract_error(error),
    };
    let request = match arguments.into_request_with_policy(network_policy) {
        Ok(request) => request,
        Err(error) => return web_fetch_error(error),
    };
    match provider.fetch(request).await {
        Ok(response) if !response.allowed_by(network_policy) => {
            web_fetch_error(blocked_by_user_policy())
        }
        Ok(response) => match serde_json::to_value(response) {
            Ok(result) => bounded_memory_tool_success(result),
            Err(_) => execution_error(
                MemoryToolExecutionErrorCode::ExecutionFailed,
                "Bottie could not serialize the native web-fetch result.",
            ),
        },
        Err(error) => web_fetch_error(error),
    }
}

/// Maps fail-closed execution-policy failures without reflecting provider-controlled call data.
pub(crate) fn policy_error(error: ToolPolicyError) -> MemoryToolExecution {
    let code = match error.code {
        ToolPolicyErrorCode::UnsupportedTool => MemoryToolExecutionErrorCode::UnsupportedTool,
        ToolPolicyErrorCode::ApprovalRequired => MemoryToolExecutionErrorCode::ApprovalRequired,
    };
    execution_error(code, error.message)
}

/// Converts one serializable tool result into JSON without exposing serializer internals.
fn serialized_result<T: Serialize>(result: T) -> Result<Value, StorageError> {
    serde_json::to_value(result).map_err(|_| StorageError::internal())
}

/// Enforces the common serialized native-tool envelope ceiling before returning success.
pub(crate) fn bounded_memory_tool_success(result: Value) -> MemoryToolExecution {
    let execution = MemoryToolExecution::Success { result };
    if serde_json::to_vec(&execution)
        .is_ok_and(|serialized| serialized.len() <= MAX_MEMORY_TOOL_OUTPUT_BYTES)
    {
        execution
    } else {
        execution_error(
            MemoryToolExecutionErrorCode::OutputTooLarge,
            "The native tool result exceeded its output limit.",
        )
    }
}

/// Maps strict contract failures without repeating provider-controlled names or arguments.
fn contract_error(error: ToolContractError) -> MemoryToolExecution {
    let code = match error.code {
        ToolContractErrorCode::UnsupportedTool => MemoryToolExecutionErrorCode::UnsupportedTool,
        ToolContractErrorCode::InvalidArguments => MemoryToolExecutionErrorCode::InvalidArguments,
    };
    execution_error(code, error.message)
}

/// Maps storage failures into stable dispatcher policy without forwarding local detail.
fn storage_error(error: StorageError) -> MemoryToolExecution {
    match error.code {
        "invalid_request" => execution_error(
            MemoryToolExecutionErrorCode::InvalidArguments,
            "The native memory tool could not accept those arguments.",
        ),
        "not_found" | "recovery_required" => execution_error(
            MemoryToolExecutionErrorCode::Unavailable,
            "The requested native memory is unavailable.",
        ),
        _ => execution_error(
            MemoryToolExecutionErrorCode::ExecutionFailed,
            "Bottie could not execute the native memory tool.",
        ),
    }
}

/// Maps native web-search failures into the common redacted dispatcher categories.
fn web_search_error(error: WebSearchError) -> MemoryToolExecution {
    match error.code {
        WebSearchErrorCode::InvalidRequest => execution_error(
            MemoryToolExecutionErrorCode::InvalidArguments,
            "The native web-search tool could not accept those arguments.",
        ),
        WebSearchErrorCode::CredentialRequired
        | WebSearchErrorCode::CredentialRejected
        | WebSearchErrorCode::RateLimited
        | WebSearchErrorCode::Timeout
        | WebSearchErrorCode::Unavailable => execution_error(
            MemoryToolExecutionErrorCode::Unavailable,
            "The native web-search provider is unavailable.",
        ),
        WebSearchErrorCode::MalformedResponse | WebSearchErrorCode::Internal => execution_error(
            MemoryToolExecutionErrorCode::ExecutionFailed,
            "Bottie could not execute the native web-search tool.",
        ),
    }
}

/// Maps native web-fetch failures into stable common dispatcher categories.
fn web_fetch_error(error: WebFetchError) -> MemoryToolExecution {
    match error.code {
        WebFetchErrorCode::InvalidRequest
        | WebFetchErrorCode::BlockedAddress
        | WebFetchErrorCode::RedirectRejected => execution_error(
            MemoryToolExecutionErrorCode::InvalidArguments,
            "The native web-fetch tool could not accept that public URL.",
        ),
        WebFetchErrorCode::Timeout | WebFetchErrorCode::Unavailable => execution_error(
            MemoryToolExecutionErrorCode::Unavailable,
            "The native web destination is unavailable.",
        ),
        WebFetchErrorCode::UnsupportedContentType
        | WebFetchErrorCode::ResponseTooLarge
        | WebFetchErrorCode::MalformedResponse
        | WebFetchErrorCode::Internal => execution_error(
            MemoryToolExecutionErrorCode::ExecutionFailed,
            "Bottie could not execute the native web-fetch tool.",
        ),
    }
}

/// Maps connector, trust, credential, and response failures without forwarding native detail.
#[allow(
    dead_code,
    reason = "provider-loop wiring is explicitly deferred to the next bounded Localmail slice"
)]
fn localmail_error(error: ProviderError) -> MemoryToolExecution {
    match error.code {
        ProviderErrorCode::InvalidRequest
        | ProviderErrorCode::Unavailable
        | ProviderErrorCode::Timeout
        | ProviderErrorCode::Server => execution_error(
            MemoryToolExecutionErrorCode::Unavailable,
            "The native Localmail connector is unavailable.",
        ),
        ProviderErrorCode::MalformedResponse | ProviderErrorCode::Internal => execution_error(
            MemoryToolExecutionErrorCode::ExecutionFailed,
            "Bottie could not execute the native Localmail tool.",
        ),
    }
}

/// Creates one small common error envelope with a fixed safe message.
fn execution_error(
    code: MemoryToolExecutionErrorCode,
    message: &'static str,
) -> MemoryToolExecution {
    MemoryToolExecution::Error {
        error: MemoryToolExecutionError { code, message },
    }
}
