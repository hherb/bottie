//! Approval-required native Python tool contract tests.

use serde_json::{json, to_value};

use crate::{
    tool_contract::{
        MAX_PYTHON_PURPOSE_CHARACTERS, MAX_PYTHON_SOURCE_BYTES, PythonToolArguments,
        RUN_PYTHON_TOOL_NAME, enabled_native_tool_definitions, omlx_native_tool_definitions,
        python_tool_definition, validate_python_tool_arguments,
    },
    tool_loop::NativeToolCall,
    tool_policy::{
        ApprovedToolCall, ToolExecutionPolicy, ToolPolicyErrorCode, authorize_tool_call,
        tool_execution_policy,
    },
};

#[test]
fn publishes_one_closed_provider_independent_python_schema() {
    let definition =
        to_value(python_tool_definition()).expect("Python definition should serialize");

    assert_eq!(definition["name"], json!(RUN_PYTHON_TOOL_NAME));
    assert_eq!(definition["inputSchema"]["type"], json!("object"));
    assert_eq!(
        definition["inputSchema"]["required"],
        json!(["source", "purpose"])
    );
    assert_eq!(
        definition["inputSchema"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        definition["inputSchema"]["properties"]["source"]["maxLength"],
        json!(MAX_PYTHON_SOURCE_BYTES)
    );
    assert_eq!(
        definition["inputSchema"]["properties"]["purpose"]["maxLength"],
        json!(MAX_PYTHON_PURPOSE_CHARACTERS)
    );
}

#[test]
fn validates_exact_source_and_user_visible_purpose() {
    let arguments = validate_python_tool_arguments(
        RUN_PYTHON_TOOL_NAME,
        &json!({
            "source": "values = [2, 3, 5]\nprint(sum(values))",
            "purpose": "Add the values exactly."
        }),
    )
    .expect("bounded Python arguments should validate");

    assert_eq!(
        arguments,
        PythonToolArguments {
            source: "values = [2, 3, 5]\nprint(sum(values))".into(),
            purpose: "Add the values exactly.".into(),
        }
    );
}

#[test]
fn rejects_malformed_python_arguments_without_reflecting_source_or_purpose() {
    let private_source = "print('private value')";
    let private_purpose = "Inspect private value";
    let cases = [
        (
            "execute_python",
            json!({"source": private_source, "purpose": private_purpose}),
        ),
        (RUN_PYTHON_TOOL_NAME, json!(null)),
        (RUN_PYTHON_TOOL_NAME, json!({})),
        (RUN_PYTHON_TOOL_NAME, json!({"source": private_source})),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": " ", "purpose": private_purpose}),
        ),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": private_source, "purpose": "\n"}),
        ),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": format!("{private_source}\0"), "purpose": private_purpose}),
        ),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": private_source, "purpose": format!("{private_purpose}\0")}),
        ),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": "é".repeat((MAX_PYTHON_SOURCE_BYTES / 2) + 1), "purpose": private_purpose}),
        ),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": private_source, "purpose": "x".repeat(MAX_PYTHON_PURPOSE_CHARACTERS + 1)}),
        ),
        (
            RUN_PYTHON_TOOL_NAME,
            json!({"source": private_source, "purpose": private_purpose, "network": true}),
        ),
    ];

    for (tool_name, arguments) in cases {
        let error = validate_python_tool_arguments(tool_name, &arguments)
            .expect_err("invalid Python arguments should fail closed");
        assert!(!error.message.contains(private_source));
        assert!(!error.message.contains(private_purpose));
    }
}

#[test]
fn advertises_python_only_for_an_available_omlx_runtime() {
    for flags in [(false, false, false), (true, true, true)] {
        assert!(
            enabled_native_tool_definitions(flags.0, flags.1, flags.2)
                .iter()
                .all(|definition| definition.name != RUN_PYTHON_TOOL_NAME)
        );
    }
    assert!(
        omlx_native_tool_definitions(false, false, false, false)
            .iter()
            .all(|definition| definition.name != RUN_PYTHON_TOOL_NAME)
    );
    let omlx_definitions = omlx_native_tool_definitions(false, false, false, true);
    assert_eq!(
        omlx_definitions
            .iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>(),
        ["current_time", RUN_PYTHON_TOOL_NAME]
    );
}

#[test]
fn requires_one_exact_native_python_approval() {
    assert_eq!(
        tool_execution_policy(RUN_PYTHON_TOOL_NAME),
        Some(ToolExecutionPolicy::ApprovalRequired)
    );

    let call = NativeToolCall {
        call_id: "python-call".into(),
        tool_name: RUN_PYTHON_TOOL_NAME.into(),
        arguments: json!({"source": "print(4)", "purpose": "Calculate two plus two."}),
    };
    let missing = authorize_tool_call(&call, None)
        .expect_err("Python must never run without an exact native approval");
    assert_eq!(missing.code, ToolPolicyErrorCode::ApprovalRequired);
    assert!(authorize_tool_call(&call, Some(ApprovedToolCall::for_call(&call))).is_ok());

    let changed = NativeToolCall {
        arguments: json!({"source": "print(5)", "purpose": "Calculate two plus two."}),
        ..call.clone()
    };
    let changed_error = authorize_tool_call(&changed, Some(ApprovedToolCall::for_call(&call)))
        .expect_err("approval must not cover changed Python source");
    assert_eq!(changed_error.code, ToolPolicyErrorCode::ApprovalRequired);

    let changed_purpose = NativeToolCall {
        arguments: json!({"source": "print(4)", "purpose": "Explain a different calculation."}),
        ..call.clone()
    };
    let changed_purpose_error =
        authorize_tool_call(&changed_purpose, Some(ApprovedToolCall::for_call(&call)))
            .expect_err("approval must not cover a changed user-visible purpose");
    assert_eq!(
        changed_purpose_error.code,
        ToolPolicyErrorCode::ApprovalRequired
    );
}
