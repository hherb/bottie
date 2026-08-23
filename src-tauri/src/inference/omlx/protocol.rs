//! Pure oMLX Chat Completions request mapping built on the shared streamed function-call contract.

use serde::Serialize;

use crate::tool_contract::ToolDefinition;

use super::super::{
    multimodal::{OpenAiContent, openai_content},
    openai::protocol::{OpenAiAssistantToolCall, OpenAiToolCall, OpenAiToolResult},
    types::{ChatRequest, ChatRole, ProviderError, ReasoningEffort},
};

/// Native oMLX streaming request including only Bottie's explicitly enabled tools.
#[derive(Serialize)]
pub(super) struct OmlxChatRequest {
    model: String,
    messages: Vec<OmlxChatTurn>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    chat_template_kwargs: OmlxChatTemplateSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
    stream_options: OmlxStreamOptions,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OmlxToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
}

#[derive(Serialize)]
struct OmlxChatTemplateSettings {
    enable_thinking: bool,
}

#[derive(Serialize)]
struct OmlxStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct OmlxChatTurn {
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

impl From<ChatRequest> for OmlxChatRequest {
    /// Converts provider-neutral history and bounded settings into oMLX fields.
    fn from(request: ChatRequest) -> Self {
        Self::with_tools(request, Vec::new())
    }
}

impl OmlxChatRequest {
    /// Adds exactly the closed native definitions enabled for an explicitly capable endpoint.
    pub(super) fn with_tools(request: ChatRequest, definitions: Vec<ToolDefinition>) -> Self {
        let settings = request.settings;
        let reasoning_enabled = settings.reasoning_effort == ReasoningEffort::Low;
        let tools = definitions
            .into_iter()
            .map(OmlxToolDefinition::from)
            .collect::<Vec<_>>();
        let tool_choice = (!tools.is_empty()).then_some("auto");
        Self {
            model: request.model_id,
            messages: request
                .messages
                .into_iter()
                .map(|turn| OmlxChatTurn {
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
            temperature: settings.temperature,
            max_tokens: settings.max_output_tokens,
            chat_template_kwargs: OmlxChatTemplateSettings {
                enable_thinking: reasoning_enabled,
            },
            reasoning_effort: reasoning_enabled.then_some("low"),
            stream_options: OmlxStreamOptions {
                include_usage: true,
            },
            tools,
            tool_choice,
        }
    }

    /// Appends one assistant call batch and its exact provider-correlated native results.
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
                .any(|(call, result)| call.call_id() != result.tool_call_id)
        {
            return Err(ProviderError::internal(
                "oMLX tool results could not be correlated safely.",
                None,
            ));
        }
        self.messages.push(OmlxChatTurn {
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
            .extend(results.into_iter().map(|result| OmlxChatTurn {
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
struct OmlxToolDefinition {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OmlxToolDefinitionFunction,
}

#[derive(Serialize)]
struct OmlxToolDefinitionFunction {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

impl From<ToolDefinition> for OmlxToolDefinition {
    /// Preserves Bottie's closed native schema in oMLX's OpenAI-shaped function definition.
    fn from(definition: ToolDefinition) -> Self {
        Self {
            kind: "function",
            function: OmlxToolDefinitionFunction {
                name: definition.name,
                description: definition.description,
                parameters: definition.input_schema,
            },
        }
    }
}
