//! Pure Anthropic Messages request, stream, and native-tool normalization.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_contract::ToolDefinition;

use super::super::{
    multimodal::{AnthropicContent, anthropic_content, text_content},
    types::{ChatRequest, ChatRole, ProviderError, ReasoningEffort, Usage},
};

mod response;

pub(super) use response::AnthropicResponseAccumulator;

/// Native Anthropic Messages request body.
#[derive(Serialize)]
pub(super) struct AnthropicChatRequest {
    model: String,
    messages: Vec<AnthropicTurn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    thinking: ThinkingConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicToolDefinition>,
}

#[derive(Serialize)]
struct AnthropicTurn {
    role: &'static str,
    content: AnthropicTurnContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicTurnContent {
    Initial(AnthropicContent),
    Assistant(Vec<AnthropicAssistantBlock>),
    Results(Vec<AnthropicToolResultBlock>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ThinkingConfig {
    Disabled,
    Adaptive,
}

#[derive(Serialize)]
struct OutputConfig {
    effort: &'static str,
}

impl From<ChatRequest> for AnthropicChatRequest {
    /// Converts provider-neutral history and settings into native Messages fields.
    fn from(request: ChatRequest) -> Self {
        let reasoning_enabled = request.settings.reasoning_effort == ReasoningEffort::Low;
        let mut system = Vec::new();
        let mut messages = Vec::new();
        for turn in request.messages {
            match turn.role {
                ChatRole::System => system.push(text_content(turn.content)),
                ChatRole::User => messages.push(AnthropicTurn {
                    role: "user",
                    content: AnthropicTurnContent::Initial(anthropic_content(turn.content)),
                }),
                ChatRole::Assistant => messages.push(AnthropicTurn {
                    role: "assistant",
                    content: AnthropicTurnContent::Initial(anthropic_content(turn.content)),
                }),
            }
        }
        Self {
            model: request.model_id,
            messages,
            system: (!system.is_empty()).then(|| system.join("\n\n")),
            max_tokens: request.settings.max_output_tokens.unwrap_or(4_096),
            stream: true,
            temperature: (!reasoning_enabled)
                .then_some(request.settings.temperature)
                .flatten(),
            thinking: if reasoning_enabled {
                ThinkingConfig::Adaptive
            } else {
                ThinkingConfig::Disabled
            },
            output_config: reasoning_enabled.then_some(OutputConfig { effort: "low" }),
            tools: Vec::new(),
        }
    }
}

impl AnthropicChatRequest {
    /// Adds exactly the closed native tool definitions enabled for this request.
    pub(super) fn with_tools(request: ChatRequest, definitions: Vec<ToolDefinition>) -> Self {
        let mut request = Self::from(request);
        request.tools = definitions
            .into_iter()
            .map(AnthropicToolDefinition::from)
            .collect();
        request
    }

    /// Appends one exact assistant block sequence and its immediately following correlated results.
    pub(super) fn append_tool_exchange(
        &mut self,
        round: AnthropicToolRound,
        results: Vec<AnthropicToolResult>,
    ) -> Result<(), ProviderError> {
        if round.tool_calls.len() != results.len()
            || round
                .tool_calls
                .iter()
                .zip(&results)
                .any(|(call, result)| call.call_id != result.tool_use_id)
        {
            return Err(ProviderError::internal(
                "Anthropic-compatible tool results could not be correlated safely.",
                None,
            ));
        }
        self.messages.push(AnthropicTurn {
            role: "assistant",
            content: AnthropicTurnContent::Assistant(round.assistant_blocks),
        });
        self.messages.push(AnthropicTurn {
            role: "user",
            content: AnthropicTurnContent::Results(
                results
                    .into_iter()
                    .map(AnthropicToolResultBlock::from)
                    .collect(),
            ),
        });
        Ok(())
    }
}

#[derive(Serialize)]
struct AnthropicToolDefinition {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

impl From<ToolDefinition> for AnthropicToolDefinition {
    /// Preserves Bottie's exact closed definition in Anthropic's client-tool shape.
    fn from(definition: ToolDefinition) -> Self {
        Self {
            name: definition.name,
            description: definition.description,
            input_schema: definition.input_schema,
        }
    }
}

/// One complete Anthropic provider call ready for strict native validation and dispatch.
#[derive(Clone, Debug)]
pub(crate) struct AnthropicToolCall {
    call_id: String,
    tool_name: String,
    arguments: Value,
}

impl AnthropicToolCall {
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

/// One ordered Anthropic tool result ready for the next Messages request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnthropicToolResult {
    /// Opaque provider call identity returned without modification.
    pub(crate) tool_use_id: String,
    /// Serialized bounded native success/error envelope supplied as inert message text.
    pub(crate) content: String,
    /// Marks native structured failures using Anthropic's client-tool error signal.
    pub(crate) is_error: bool,
}

#[derive(Serialize)]
struct AnthropicToolResultBlock {
    #[serde(rename = "type")]
    kind: &'static str,
    tool_use_id: String,
    content: String,
    #[serde(skip_serializing_if = "is_false")]
    is_error: bool,
}

impl From<AnthropicToolResult> for AnthropicToolResultBlock {
    fn from(result: AnthropicToolResult) -> Self {
        Self {
            kind: "tool_result",
            tool_use_id: result.tool_use_id,
            content: result.content,
            is_error: result.is_error,
        }
    }
}

/// Omits Anthropic's optional error flag for successful native results.
fn is_false(value: &bool) -> bool {
    !value
}

/// One complete streamed Messages assistant round before optional native execution.
pub(crate) struct AnthropicToolRound {
    assistant_blocks: Vec<AnthropicAssistantBlock>,
    /// Ordered complete client-tool calls reconstructed from streamed blocks.
    pub(crate) tool_calls: Vec<AnthropicToolCall>,
    /// Provider-reported usage for this Messages request.
    pub(crate) usage: Option<Usage>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicAssistantBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamPayload {
    MessageStart {
        message: StartMessage,
    },
    ContentBlockStart {
        index: usize,
        content_block: StartBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: WireUsage,
    },
    MessageStop,
    Error {
        error: AnthropicError,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
struct StartMessage {
    usage: WireUsage,
}

#[derive(Default, Deserialize)]
struct WireUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_usd: Option<f64>,
}

#[derive(Default, Deserialize)]
struct MessageDelta {
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum StartBlock {
    Text {
        #[serde(default)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
        #[serde(default)]
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ContentDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    SignatureDelta {
        signature: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    #[serde(other)]
    Unknown,
}

/// One decoded Messages SSE event with enough structure for exact block reconstruction.
pub(super) enum DecodedEvent {
    BlockStart {
        index: usize,
        block: StartBlock,
    },
    BlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    BlockStop {
        index: usize,
    },
    Usage(Usage),
    MessageDelta {
        stop_reason: Option<String>,
        usage: Usage,
    },
    Done,
    Ignored,
}

impl DecodedEvent {
    /// Returns visible answer text without consuming the block event.
    pub(super) fn text_delta(&self) -> Option<&str> {
        match self {
            Self::BlockDelta {
                delta: ContentDelta::TextDelta { text },
                ..
            } => Some(text),
            _ => None,
        }
    }

    /// Returns separate reasoning text without consuming the block event.
    pub(super) fn reasoning_delta(&self) -> Option<&str> {
        match self {
            Self::BlockDelta {
                delta: ContentDelta::ThinkingDelta { thinking },
                ..
            } => Some(thinking),
            _ => None,
        }
    }

    /// Reports whether this event updates the current request's usage counters.
    pub(super) fn has_usage(&self) -> bool {
        matches!(self, Self::Usage(_) | Self::MessageDelta { .. })
    }
}

/// Decodes one Anthropic Messages SSE data payload without performing I/O.
pub(super) fn decode_stream_payload(payload: &str) -> Result<DecodedEvent, ProviderError> {
    let event: StreamPayload = serde_json::from_str(payload).map_err(|error| {
        ProviderError::malformed(
            "The Anthropic-compatible provider sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    match event {
        StreamPayload::MessageStart { message } => Ok(DecodedEvent::Usage(message.usage.into())),
        StreamPayload::ContentBlockStart {
            index,
            content_block,
        } => Ok(DecodedEvent::BlockStart {
            index,
            block: content_block,
        }),
        StreamPayload::ContentBlockDelta { index, delta } => {
            Ok(DecodedEvent::BlockDelta { index, delta })
        }
        StreamPayload::ContentBlockStop { index } => Ok(DecodedEvent::BlockStop { index }),
        StreamPayload::MessageDelta { delta, usage } => Ok(DecodedEvent::MessageDelta {
            stop_reason: delta.stop_reason,
            usage: usage.into(),
        }),
        StreamPayload::MessageStop => Ok(DecodedEvent::Done),
        StreamPayload::Error { error } => Err(ProviderError::server(error.message, None)),
        StreamPayload::Unknown => Ok(DecodedEvent::Ignored),
    }
}

#[derive(Deserialize)]
struct AnthropicError {
    message: String,
}

impl From<WireUsage> for Usage {
    fn from(value: WireUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cost_usd: value.cost_usd,
        }
    }
}
