//! Closed provider-independent contract for a proposed bounded Python execution.

#![allow(dead_code, reason = "provider advertisement is intentionally deferred")]

use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ToolContractError, ToolContractErrorCode, ToolDefinition, argument_object, deserialize,
    invalid_arguments, require_bounded_string, require_only_fields,
};

/// Stable native name reserved for approval-required bounded Python execution.
pub(crate) const RUN_PYTHON_TOOL_NAME: &str = "run_python";
/// Maximum UTF-8 source bytes accepted by both the native proposal and standalone runner.
pub(crate) const MAX_PYTHON_SOURCE_BYTES: usize = 32 * 1_024;
/// Maximum Unicode scalar count accepted for the user-visible execution purpose.
pub(crate) const MAX_PYTHON_PURPOSE_CHARACTERS: usize = 512;

/// Exact proposal produced only after raw Python tool arguments pass the closed native contract.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonToolArguments {
    /// Complete Python source shown to the user before any later execution.
    pub(crate) source: String,
    /// Short explanation shown beside the exact proposed source.
    pub(crate) purpose: String,
}

/// Returns the reserved Python definition without advertising it to any provider adapter.
pub(crate) fn python_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: RUN_PYTHON_TOOL_NAME,
        description: concat!(
            "Propose bounded Python source for isolated execution. Bottie must show the exact source and purpose ",
            "and obtain user approval before execution."
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Complete Python source to show for approval before execution.",
                    "minLength": 1,
                    "maxLength": MAX_PYTHON_SOURCE_BYTES
                },
                "purpose": {
                    "type": "string",
                    "description": "Brief user-visible explanation of why the source should run.",
                    "minLength": 1,
                    "maxLength": MAX_PYTHON_PURPOSE_CHARACTERS
                }
            },
            "required": ["source", "purpose"],
            "additionalProperties": false
        }),
    }
}

/// Validates exact source and purpose without launching, persisting, or approving the helper.
pub(crate) fn validate_python_tool_arguments(
    tool_name: &str,
    arguments: &Value,
) -> Result<PythonToolArguments, ToolContractError> {
    if tool_name != RUN_PYTHON_TOOL_NAME {
        return Err(ToolContractError {
            code: ToolContractErrorCode::UnsupportedTool,
            message: "The provider requested an unsupported native tool.",
        });
    }
    let object = argument_object(arguments)?;
    require_only_fields(object, &["source", "purpose"])?;
    require_bounded_string(object, "source", MAX_PYTHON_SOURCE_BYTES)?;
    require_bounded_string(object, "purpose", MAX_PYTHON_PURPOSE_CHARACTERS)?;
    let parsed: PythonToolArguments = deserialize(arguments)?;
    if parsed.source.len() > MAX_PYTHON_SOURCE_BYTES
        || parsed.source.contains('\0')
        || parsed.purpose.contains('\0')
    {
        return Err(invalid_arguments());
    }
    Ok(parsed)
}
