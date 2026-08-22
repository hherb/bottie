//! Pure OpenAI Chat Completions request, stream, and native-tool normalization.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_contract::ToolDefinition;

use super::super::{
    multimodal::{OpenAiContent, openai_content},
    types::{ChatRequest, ChatRole, ProviderError, ReasoningEffort, Usage},
};

const FUNCTION_TOOL_TYPE: &str = "function";
const MAX_PROVIDER_CALL_ID_CHARACTERS: usize = 512;
const MAX_TOOL_NAME_CHARACTERS: usize = 128;
const MAX_STREAMED_TOOL_CALLS: usize = 64;

/// Native OpenAI streaming-chat request body.
#[derive(Serialize)]
pub(super) struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiTurn>,
    stream: bool,
    stream_options: OpenAiStreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiToolDefinition>,
}

#[derive(Serialize)]
struct OpenAiStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct OpenAiTurn {
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<OpenAiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OpenAiAssistantToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

impl From<ChatRequest> for OpenAiChatRequest {
    /// Converts provider-neutral history and settings into Chat Completions fields.
    fn from(request: ChatRequest) -> Self {
        Self {
            model: request.model_id,
            messages: request
                .messages
                .into_iter()
                .map(|turn| OpenAiTurn {
                    role: match turn.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    },
                    content: Some(openai_content(turn.content)),
                    reasoning_content: None,
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                })
                .collect(),
            stream: true,
            stream_options: OpenAiStreamOptions {
                include_usage: true,
            },
            max_completion_tokens: request.settings.max_output_tokens,
            reasoning_effort: (request.settings.reasoning_effort == ReasoningEffort::Low)
                .then_some("low"),
            tools: Vec::new(),
        }
    }
}

impl OpenAiChatRequest {
    /// Adds exactly the closed native tool definitions enabled for this request.
    pub(super) fn with_tools(request: ChatRequest, definitions: Vec<ToolDefinition>) -> Self {
        let mut request = Self::from(request);
        request.tools = definitions
            .into_iter()
            .map(OpenAiToolDefinition::from)
            .collect();
        request
    }

    /// Appends one assistant call batch and its exactly correlated tool results.
    pub(super) fn append_tool_exchange(
        &mut self,
        reasoning: String,
        content: String,
        tool_calls: Vec<OpenAiToolCall>,
        results: Vec<OpenAiToolResult>,
    ) -> Result<(), ProviderError> {
        if tool_calls.len() != results.len()
            || tool_calls
                .iter()
                .zip(&results)
                .any(|(call, result)| call.call_id != result.tool_call_id)
        {
            return Err(ProviderError::internal(
                "OpenAI-compatible tool results could not be correlated safely.",
                None,
            ));
        }
        self.messages.push(OpenAiTurn {
            role: "assistant",
            content: (!content.is_empty()).then_some(OpenAiContent::Text(content)),
            reasoning_content: (!reasoning.is_empty()).then_some(reasoning),
            tool_calls: tool_calls
                .into_iter()
                .map(OpenAiAssistantToolCall::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            tool_call_id: None,
        });
        self.messages
            .extend(results.into_iter().map(|result| OpenAiTurn {
                role: "tool",
                content: Some(OpenAiContent::Text(result.content)),
                reasoning_content: None,
                tool_calls: Vec::new(),
                tool_call_id: Some(result.tool_call_id),
            }));
        Ok(())
    }
}

#[derive(Serialize)]
struct OpenAiToolDefinition {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiToolDefinitionFunction,
}

#[derive(Serialize)]
struct OpenAiToolDefinitionFunction {
    name: &'static str,
    description: &'static str,
    parameters: Value,
}

impl From<ToolDefinition> for OpenAiToolDefinition {
    /// Preserves Bottie's exact closed definition in Chat Completions function shape.
    fn from(definition: ToolDefinition) -> Self {
        Self {
            kind: FUNCTION_TOOL_TYPE,
            function: OpenAiToolDefinitionFunction {
                name: definition.name,
                description: definition.description,
                parameters: definition.input_schema,
            },
        }
    }
}

#[derive(Serialize)]
struct OpenAiAssistantToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiAssistantToolCallFunction,
}

#[derive(Serialize)]
struct OpenAiAssistantToolCallFunction {
    name: String,
    arguments: String,
}

impl TryFrom<OpenAiToolCall> for OpenAiAssistantToolCall {
    type Error = ProviderError;

    /// Serializes validated object arguments back into the required JSON string field.
    fn try_from(call: OpenAiToolCall) -> Result<Self, Self::Error> {
        let arguments = serde_json::to_string(&call.arguments).map_err(|_| {
            ProviderError::internal(
                "OpenAI-compatible tool arguments could not be serialized.",
                None,
            )
        })?;
        Ok(Self {
            id: call.call_id,
            kind: FUNCTION_TOOL_TYPE,
            function: OpenAiAssistantToolCallFunction {
                name: call.tool_name,
                arguments,
            },
        })
    }
}

/// One complete OpenAI provider call ready for strict native validation and dispatch.
#[derive(Clone, Debug)]
pub(crate) struct OpenAiToolCall {
    call_id: String,
    tool_name: String,
    arguments: Value,
}

impl OpenAiToolCall {
    /// Returns the opaque provider identity required on the correlated tool result.
    pub(crate) fn call_id(&self) -> &str {
        &self.call_id
    }

    /// Returns the stable native tool name requested by the provider.
    pub(crate) fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// Returns raw object arguments for Bottie's strict native contract validation.
    pub(crate) fn arguments(&self) -> &Value {
        &self.arguments
    }

    #[cfg(test)]
    /// Builds one complete provider call without a streamed fixture.
    pub(crate) fn fixture(call_id: &str, tool_name: &str, arguments: Value) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            arguments,
        }
    }
}

/// One ordered OpenAI tool result ready for the next Chat Completions request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenAiToolResult {
    /// Opaque provider call identity returned without modification.
    pub(crate) tool_call_id: String,
    /// Serialized bounded native success/error envelope supplied as inert message text.
    pub(crate) content: String,
}

#[derive(Default, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<Choice>,
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Default, Deserialize)]
struct Delta {
    content: Option<String>,
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCallDelta>,
}

#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    #[serde(alias = "cost")]
    cost_usd: Option<f64>,
}

#[derive(Deserialize)]
pub(super) struct OpenAiToolCallDelta {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    function: Option<OpenAiToolCallFunctionDelta>,
}

#[derive(Deserialize)]
struct OpenAiToolCallFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

/// One normalized stream payload whose independent fields can coexist in a provider event.
pub(super) struct DecodedStreamEvent {
    pub(super) text_delta: String,
    pub(super) reasoning_delta: String,
    pub(super) tool_call_deltas: Vec<OpenAiToolCallDelta>,
    pub(super) usage: Option<Usage>,
    pub(super) done: bool,
}

/// Decodes one Chat Completions SSE data payload without performing I/O.
pub(super) fn decode_stream_payload(payload: &str) -> Result<DecodedStreamEvent, ProviderError> {
    if payload.trim() == "[DONE]" {
        return Ok(DecodedStreamEvent {
            text_delta: String::new(),
            reasoning_delta: String::new(),
            tool_call_deltas: Vec::new(),
            usage: None,
            done: true,
        });
    }
    let chunk: StreamChunk = serde_json::from_str(payload).map_err(|error| {
        ProviderError::malformed(
            "The OpenAI-compatible provider sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    let delta = chunk
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.delta)
        .unwrap_or_default();
    Ok(DecodedStreamEvent {
        text_delta: delta.content.unwrap_or_default(),
        reasoning_delta: delta.reasoning_content.unwrap_or_default(),
        tool_call_deltas: delta.tool_calls,
        usage: chunk.usage.map(|usage| Usage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            cost_usd: usage.cost_usd,
        }),
        done: false,
    })
}

#[derive(Default)]
/// Incrementally reconstructs streamed Chat Completions function-call fragments by index.
pub(super) struct OpenAiToolCallAccumulator {
    calls: BTreeMap<usize, PartialOpenAiToolCall>,
}

#[derive(Default)]
struct PartialOpenAiToolCall {
    call_id: Option<String>,
    kind: Option<String>,
    tool_name: String,
    arguments: String,
}

impl OpenAiToolCallAccumulator {
    /// Extends partial calls while rejecting conflicting identities and an excessive index set.
    pub(super) fn extend(&mut self, deltas: Vec<OpenAiToolCallDelta>) -> Result<(), ProviderError> {
        for delta in deltas {
            if !self.calls.contains_key(&delta.index) && self.calls.len() >= MAX_STREAMED_TOOL_CALLS
            {
                return Err(malformed_tool_call("too many streamed call indexes"));
            }
            let call = self.calls.entry(delta.index).or_default();
            merge_identity(
                &mut call.call_id,
                delta.id,
                "conflicting provider call identity",
            )?;
            merge_identity(&mut call.kind, delta.kind, "conflicting tool-call type")?;
            if let Some(function) = delta.function {
                if let Some(name) = function.name {
                    call.tool_name.push_str(&name);
                }
                if let Some(arguments) = function.arguments {
                    call.arguments.push_str(&arguments);
                }
            }
        }
        Ok(())
    }

    /// Finalizes ordered calls only when identity, function kind, name, and object JSON are valid.
    pub(super) fn finish(self) -> Result<Vec<OpenAiToolCall>, ProviderError> {
        self.calls
            .into_values()
            .map(|call| {
                let call_id = bounded_identity(
                    call.call_id,
                    MAX_PROVIDER_CALL_ID_CHARACTERS,
                    "missing or overlong provider call identity",
                )?;
                if call.kind.as_deref() != Some(FUNCTION_TOOL_TYPE) {
                    return Err(malformed_tool_call("tool-call type was not function"));
                }
                let tool_name = bounded_identity(
                    Some(call.tool_name),
                    MAX_TOOL_NAME_CHARACTERS,
                    "missing or overlong tool name",
                )?;
                let arguments: Value = serde_json::from_str(&call.arguments)
                    .map_err(|_| malformed_tool_call("arguments were not complete JSON"))?;
                if !arguments.is_object() {
                    return Err(malformed_tool_call("arguments were not a JSON object"));
                }
                Ok(OpenAiToolCall {
                    call_id,
                    tool_name,
                    arguments,
                })
            })
            .collect()
    }
}

fn merge_identity(
    current: &mut Option<String>,
    next: Option<String>,
    diagnostic: &'static str,
) -> Result<(), ProviderError> {
    let Some(next) = next else {
        return Ok(());
    };
    if current.as_ref().is_some_and(|current| current != &next) {
        return Err(malformed_tool_call(diagnostic));
    }
    *current = Some(next);
    Ok(())
}

fn bounded_identity(
    value: Option<String>,
    maximum: usize,
    diagnostic: &'static str,
) -> Result<String, ProviderError> {
    value
        .filter(|value| !value.trim().is_empty() && value.chars().count() <= maximum)
        .ok_or_else(|| malformed_tool_call(diagnostic))
}

fn malformed_tool_call(diagnostic: &'static str) -> ProviderError {
    ProviderError::malformed(
        "The OpenAI-compatible provider sent an invalid native tool call.",
        Some(diagnostic.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes one complete call delta and returns its accumulator.
    fn call_accumulator(arguments: &str) -> OpenAiToolCallAccumulator {
        let payload = serde_json::json!({
            "choices": [{"delta": {"tool_calls": [{
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": {"name": "search_memory", "arguments": arguments}
            }]}}]
        });
        let event = decode_stream_payload(&payload.to_string()).unwrap();
        let mut calls = OpenAiToolCallAccumulator::default();
        calls.extend(event.tool_call_deltas).unwrap();
        calls
    }

    #[test]
    fn rejects_non_object_tool_arguments() {
        let error = call_accumulator("[]")
            .finish()
            .expect_err("array arguments must fail");
        assert_eq!(error.code.as_str(), "malformed_response");
    }

    #[test]
    fn rejects_mismatched_tool_result_identity() {
        let request: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "openai",
            "modelId": "gpt-example",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "recall"}]}]
        }))
        .unwrap();
        let mut request = OpenAiChatRequest::from(request);
        let error = request
            .append_tool_exchange(
                String::new(),
                String::new(),
                call_accumulator(r#"{"query":"release"}"#).finish().unwrap(),
                vec![OpenAiToolResult {
                    tool_call_id: "different_call".into(),
                    content: r#"{"ok":true}"#.into(),
                }],
            )
            .expect_err("mismatched identity must fail");
        assert_eq!(error.code.as_str(), "internal");
    }
}
