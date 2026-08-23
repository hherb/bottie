//! Closed provider-independent contract for Bottie's credential-free UTC clock.

use serde_json::{Value, json};

use super::{
    ToolContractError, ToolContractErrorCode, ToolDefinition, argument_object, invalid_arguments,
};

/// Stable native name used by every mapped provider for the Rust-owned clock.
pub(crate) const CURRENT_TIME_TOOL_NAME: &str = "current_time";

/// Returns the closed zero-argument native clock definition.
pub(crate) fn current_time_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: CURRENT_TIME_TOOL_NAME,
        description: "Return Bottie's current system clock as an RFC 3339 UTC timestamp.",
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        }),
    }
}

/// Accepts only the advertised clock name and an exact empty JSON object.
pub(crate) fn validate_current_time_tool_arguments(
    tool_name: &str,
    arguments: &Value,
) -> Result<(), ToolContractError> {
    if tool_name != CURRENT_TIME_TOOL_NAME {
        return Err(ToolContractError {
            code: ToolContractErrorCode::UnsupportedTool,
            message: "The provider requested an unsupported native tool.",
        });
    }
    if argument_object(arguments)?.is_empty() {
        Ok(())
    } else {
        Err(invalid_arguments())
    }
}
