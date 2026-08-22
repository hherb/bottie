//! Ollama-native function definition, call, and result wire values.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_contract::ToolDefinition;

const FUNCTION_TOOL_TYPE: &str = "function";

/// One Ollama function definition mapped from Bottie's closed native tool contract.
#[derive(Serialize)]
pub(super) struct OllamaToolDefinition {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OllamaToolDefinitionFunction,
}

#[derive(Serialize)]
struct OllamaToolDefinitionFunction {
    name: &'static str,
    description: &'static str,
    parameters: Value,
}

impl From<ToolDefinition> for OllamaToolDefinition {
    /// Preserves Bottie's exact name, guidance, and closed input schema in Ollama's function shape.
    fn from(definition: ToolDefinition) -> Self {
        Self {
            kind: FUNCTION_TOOL_TYPE,
            function: OllamaToolDefinitionFunction {
                name: definition.name,
                description: definition.description,
                parameters: definition.input_schema,
            },
        }
    }
}

/// One complete Ollama-native tool call emitted by a streamed assistant message.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct OllamaToolCall {
    #[serde(default = "function_tool_type", rename = "type")]
    kind: String,
    function: OllamaToolCallFunction,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OllamaToolCallFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<usize>,
    name: String,
    arguments: Value,
}

impl OllamaToolCall {
    /// Returns the stable native tool name requested by Ollama.
    pub(crate) fn tool_name(&self) -> &str {
        &self.function.name
    }

    /// Returns the raw JSON arguments for Bottie's strict native contract validation.
    pub(crate) fn arguments(&self) -> &Value {
        &self.function.arguments
    }

    /// Confirms the provider emitted Ollama's function-call kind, a bounded name, and object arguments.
    pub(super) fn is_valid(&self) -> bool {
        self.kind == FUNCTION_TOOL_TYPE
            && !self.function.name.trim().is_empty()
            && self.function.name.chars().count() <= 128
            && self.function.arguments.is_object()
    }

    #[cfg(test)]
    /// Builds one provider call without serializing a fixture stream.
    pub(crate) fn fixture(index: usize, tool_name: &str, arguments: Value) -> Self {
        Self {
            kind: FUNCTION_TOOL_TYPE.into(),
            function: OllamaToolCallFunction {
                index: Some(index),
                name: tool_name.into(),
                arguments,
            },
        }
    }
}

/// One ordered Ollama tool-result message ready for the next chat request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OllamaToolResult {
    /// Tool name Ollama uses to associate this ordered result.
    pub(crate) tool_name: String,
    /// Serialized bounded native success/error envelope supplied as inert message text.
    pub(crate) content: String,
}

/// Supplies the default function discriminator when Ollama omits the optional field.
fn function_tool_type() -> String {
    FUNCTION_TOOL_TYPE.into()
}
