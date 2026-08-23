//! Provider-independent Localmail tool definition, policy, and dispatcher tests.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};

use crate::{
    credentials::CredentialStore,
    inference::ProviderError,
    tool_contract::{
        LocalmailToolArguments, enabled_native_tool_definitions, localmail_tool_definitions,
        validate_localmail_tool_arguments,
    },
    tool_dispatch::{
        ConfiguredLocalmailToolExecutor, LocalmailToolExecutor, MAX_MEMORY_TOOL_OUTPUT_BYTES,
        MemoryToolExecution, MemoryToolExecutionErrorCode, dispatch_localmail_tool,
    },
    tool_loop::NativeToolCall,
    tool_policy::{ToolExecutionPolicy, authorize_tool_call, tool_execution_policy},
};

/// Credential boundary that fails the test if invalid arguments reach native configuration work.
struct RejectCredentialAccess;

impl CredentialStore for RejectCredentialAccess {
    fn configured(&self, _provider_id: &str) -> Result<bool, ProviderError> {
        panic!("invalid tool arguments must not inspect credential state")
    }

    fn unlocked(&self, _provider_id: &str) -> Result<bool, ProviderError> {
        panic!("invalid tool arguments must not inspect credential state")
    }

    fn biometric_protected(&self) -> bool {
        false
    }

    fn get(&self, _provider_id: &str) -> Result<Option<String>, ProviderError> {
        panic!("invalid tool arguments must not read the credential vault")
    }

    fn set(&self, _provider_id: &str, _api_key: &str) -> Result<(), ProviderError> {
        panic!("tool dispatch must not write the credential vault")
    }

    fn delete(&self, _provider_id: &str) -> Result<(), ProviderError> {
        panic!("tool dispatch must not delete from the credential vault")
    }
}

#[test]
fn publishes_two_closed_schemas_without_advertising_them() {
    let serialized = localmail_tool_definitions()
        .into_iter()
        .map(|definition| serde_json::to_value(definition).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(serialized[0]["name"], json!("search_email"));
    assert_eq!(serialized[1]["name"], json!("open_email"));
    assert_eq!(
        serialized[0]["inputSchema"]["required"],
        json!(["query", "resultLimit"])
    );
    assert_eq!(
        serialized[0]["inputSchema"]["properties"]["resultLimit"]["maximum"],
        json!(20)
    );
    assert_eq!(
        serialized[0]["inputSchema"]["properties"]["filters"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        serialized[1]["inputSchema"]["required"],
        json!(["messageId"])
    );
    assert_eq!(
        serialized[1]["inputSchema"]["additionalProperties"],
        json!(false)
    );

    let enabled_names = enabled_native_tool_definitions(true, true, false)
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert!(!enabled_names.contains(&"search_email"));
    assert!(!enabled_names.contains(&"open_email"));

    let ollama_email_names = enabled_native_tool_definitions(false, false, true)
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert_eq!(
        ollama_email_names,
        vec!["search_email", "open_email", "current_time"]
    );
}

#[test]
fn converts_json_into_the_exact_existing_connector_requests() {
    let search = validate_localmail_tool_arguments(
        "search_email",
        &json!({
            "query": "  quarterly   status  ",
            "filters": {
                "from": " ops@example.com ",
                "to": "team@example.com",
                "subject": "Quarterly status",
                "after": "2026-08-01",
                "before": "2026-08-23",
                "hasAttachments": true
            },
            "resultLimit": 7
        }),
    )
    .expect("search_email arguments should validate");
    let LocalmailToolArguments::SearchEmail(request) = search else {
        panic!("search_email should produce its exact connector request");
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "query": "  quarterly   status  ",
            "filters": {
                "from": " ops@example.com ",
                "to": "team@example.com",
                "subject": "Quarterly status",
                "after": "2026-08-01",
                "before": "2026-08-23",
                "hasAttachments": true
            },
            "resultLimit": 7
        })
    );

    let opened = validate_localmail_tool_arguments("open_email", &json!({"messageId": "42"}))
        .expect("open_email arguments should validate");
    let LocalmailToolArguments::OpenEmail(request) = opened else {
        panic!("open_email should produce its exact connector request");
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({"messageId": "42"})
    );
}

#[test]
fn rejects_invalid_shapes_without_reflecting_arguments() {
    let cases = [
        ("web_search", json!({"query": "private"})),
        ("search_email", json!([])),
        (
            "search_email",
            json!({"query": "private", "resultLimit": 1, "unknown": true}),
        ),
        ("search_email", json!({"query": "   ", "resultLimit": 1})),
        (
            "search_email",
            json!({"query": "private", "resultLimit": 0}),
        ),
        (
            "search_email",
            json!({"query": "private", "resultLimit": 21}),
        ),
        (
            "search_email",
            json!({"query": "private", "resultLimit": 1, "filters": null}),
        ),
        (
            "search_email",
            json!({"query": "private", "resultLimit": 1, "filters": {"after": "2026-08-24", "before": "2026-08-23"}}),
        ),
        ("open_email", json!({})),
        ("open_email", json!({"messageId": "../42"})),
        (
            "open_email",
            json!({"messageId": "42", "includeHtml": true}),
        ),
    ];

    for (tool_name, arguments) in cases {
        let error = validate_localmail_tool_arguments(tool_name, &arguments)
            .expect_err("invalid Localmail arguments should fail");
        assert!(!error.message.contains(&arguments.to_string()));
    }
}

#[test]
fn classifies_both_bounded_read_only_tools_as_safe() {
    for definition in localmail_tool_definitions() {
        assert_eq!(
            tool_execution_policy(definition.name),
            Some(ToolExecutionPolicy::Safe)
        );
        let call = NativeToolCall {
            call_id: "email-call".into(),
            tool_name: definition.name.into(),
            arguments: json!({}),
        };
        assert!(authorize_tool_call(&call, None).is_ok());
    }
}

/// Deterministic executor that records the exact typed request selected by dispatch.
#[derive(Clone)]
struct DispatchLocalmailExecutor {
    requests: Arc<Mutex<Vec<Value>>>,
    result: Result<Value, ProviderError>,
}

impl LocalmailToolExecutor for DispatchLocalmailExecutor {
    async fn execute(&self, arguments: LocalmailToolArguments) -> Result<Value, ProviderError> {
        let request = match arguments {
            LocalmailToolArguments::SearchEmail(request) => {
                json!({"tool": "search_email", "arguments": request})
            }
            LocalmailToolArguments::OpenEmail(request) => {
                json!({"tool": "open_email", "arguments": request})
            }
        };
        self.requests.lock().unwrap().push(request);
        self.result.clone()
    }
}

/// Extracts one successful result while asserting the common envelope and byte ceiling.
fn success_result(execution: MemoryToolExecution) -> Value {
    assert!(
        serde_json::to_vec(&execution)
            .is_ok_and(|serialized| serialized.len() <= MAX_MEMORY_TOOL_OUTPUT_BYTES)
    );
    let serialized = serde_json::to_value(execution).expect("execution should serialize");
    assert_eq!(serialized["ok"], json!(true));
    assert!(serialized.get("error").is_none());
    serialized["result"].clone()
}

#[test]
fn dispatches_both_tools_through_the_common_bounded_envelope() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let executor = DispatchLocalmailExecutor {
        requests: requests.clone(),
        result: Ok(json!({"untrusted": true, "bounded": "result"})),
    };

    for (name, arguments) in [
        (
            "search_email",
            json!({"query": "release notes", "filters": {"hasAttachments": true}, "resultLimit": 3}),
        ),
        ("open_email", json!({"messageId": "42"})),
    ] {
        let execution = tauri::async_runtime::block_on(dispatch_localmail_tool(
            &executor,
            &NativeToolCall {
                call_id: "email-call".into(),
                tool_name: name.into(),
                arguments,
            },
            None,
        ));
        assert_eq!(
            success_result(execution),
            json!({"untrusted": true, "bounded": "result"})
        );
    }

    let requests = requests.lock().unwrap();
    assert_eq!(requests[0]["tool"], json!("search_email"));
    assert_eq!(requests[0]["arguments"]["resultLimit"], json!(3));
    assert_eq!(requests[1]["tool"], json!("open_email"));
    assert_eq!(requests[1]["arguments"]["messageId"], json!("42"));
}

#[test]
fn rejects_before_execution_and_redacts_connector_failures() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let executor = DispatchLocalmailExecutor {
        requests: requests.clone(),
        result: Err(ProviderError::unavailable(
            "private archive origin failed",
            Some("token and certificate detail".into()),
        )),
    };
    let invalid = tauri::async_runtime::block_on(dispatch_localmail_tool(
        &executor,
        &NativeToolCall {
            call_id: "email-call".into(),
            tool_name: "search_email".into(),
            arguments: json!({"query": "private", "resultLimit": 99}),
        },
        None,
    ));
    let MemoryToolExecution::Error { error } = invalid else {
        panic!("invalid Localmail calls should fail");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::InvalidArguments);
    assert!(requests.lock().unwrap().is_empty());

    let failed = tauri::async_runtime::block_on(dispatch_localmail_tool(
        &executor,
        &NativeToolCall {
            call_id: "email-call".into(),
            tool_name: "open_email".into(),
            arguments: json!({"messageId": "42"}),
        },
        None,
    ));
    let MemoryToolExecution::Error { error } = failed else {
        panic!("connector failures should use the common error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::Unavailable);
    assert!(!error.message.contains("archive"));
    assert!(!error.message.contains("token"));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn configured_dispatch_validates_before_config_or_vault_access() {
    let credentials = RejectCredentialAccess;
    let executor =
        ConfiguredLocalmailToolExecutor::new(Path::new("/unreadable/localmail.json"), &credentials);
    let execution = tauri::async_runtime::block_on(dispatch_localmail_tool(
        &executor,
        &NativeToolCall {
            call_id: "email-call".into(),
            tool_name: "open_email".into(),
            arguments: json!({"messageId": "../private"}),
        },
        None,
    ));
    let MemoryToolExecution::Error { error } = execution else {
        panic!("invalid configured calls should fail before native state access");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::InvalidArguments);
}

#[test]
fn applies_the_common_output_ceiling() {
    let executor = DispatchLocalmailExecutor {
        requests: Arc::new(Mutex::new(Vec::new())),
        result: Ok(json!({"body": "x".repeat(MAX_MEMORY_TOOL_OUTPUT_BYTES)})),
    };
    let execution = tauri::async_runtime::block_on(dispatch_localmail_tool(
        &executor,
        &NativeToolCall {
            call_id: "email-call".into(),
            tool_name: "open_email".into(),
            arguments: json!({"messageId": "42"}),
        },
        None,
    ));
    let MemoryToolExecution::Error { error } = execution else {
        panic!("oversized Localmail results should fail closed");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::OutputTooLarge);
}
