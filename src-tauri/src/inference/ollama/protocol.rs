//! Pure Ollama request and response normalization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{PROVIDER_ID, PROVIDER_NAME};
use crate::inference::multimodal::{base64_image, text_content};
use crate::inference::types::{
    ChatRequest, ChatRole, ContentBlock, ModelInfo, ModelLoadState, ProviderCapabilities,
    ProviderError, ReasoningEffort, Usage,
};
use crate::tool_contract::ToolDefinition;

mod tools;

use tools::OllamaToolDefinition;
pub(crate) use tools::{OllamaToolCall, OllamaToolResult};

#[derive(Deserialize)]
/// Top-level installed-model response.
pub(super) struct OllamaModelList {
    #[serde(default)]
    /// Installed models reported by Ollama.
    pub(super) models: Vec<OllamaListedModel>,
}

#[derive(Deserialize)]
/// One model reported by the installed-model endpoint.
pub(super) struct OllamaListedModel {
    #[serde(default)]
    /// Legacy model identity field.
    pub(super) name: String,
    #[serde(default)]
    /// Preferred model identity field.
    pub(super) model: String,
    #[serde(default)]
    /// Capabilities advertised directly in the listing.
    pub(super) capabilities: Vec<String>,
    #[serde(default)]
    /// Optional details advertised directly in the listing.
    pub(super) details: OllamaListedDetails,
}

#[derive(Default, Deserialize)]
/// Model details embedded in an installed-model record.
pub(super) struct OllamaListedDetails {
    /// Advertised context length when present.
    pub(super) context_length: Option<u64>,
}

/// Decodes the installed-model response without performing I/O.
pub(super) fn decode_model_list(bytes: &[u8]) -> Result<OllamaModelList, ProviderError> {
    serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "Ollama returned an invalid model list.",
            Some(error.to_string()),
        )
    })
}

#[derive(Clone, Deserialize)]
struct OllamaRunningList {
    #[serde(default)]
    models: Vec<OllamaRunningModel>,
}

#[derive(Clone, Deserialize)]
struct OllamaRunningModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
    context_length: Option<u64>,
}

/// Decodes running-model identities and optional context lengths without performing I/O.
pub(super) fn decode_running_models(
    bytes: &[u8],
) -> Result<HashMap<String, Option<u64>>, ProviderError> {
    let response: OllamaRunningList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "Ollama returned an invalid running-model list.",
            Some(error.to_string()),
        )
    })?;
    Ok(response
        .models
        .into_iter()
        .map(|model| {
            let id = if model.model.trim().is_empty() {
                model.name
            } else {
                model.model
            };
            (id, model.context_length)
        })
        .collect())
}

#[derive(Serialize)]
/// Request body for detailed model metadata.
pub(super) struct OllamaShowRequest<'a> {
    /// Provider-owned model identity.
    pub(super) model: &'a str,
    /// Whether Ollama should return verbose tokenizer data.
    pub(super) verbose: bool,
}

#[derive(Deserialize)]
/// Capability and model metadata returned by the detail endpoint.
pub(super) struct OllamaShowResponse {
    #[serde(default)]
    /// Advertised capability labels.
    pub(super) capabilities: Vec<String>,
    #[serde(default)]
    /// Provider-specific model metadata.
    pub(super) model_info: HashMap<String, Value>,
}

/// Normalizes installed, detailed, and running metadata into one model record.
pub(super) fn model_info(
    model_id: String,
    listed_capabilities: &[String],
    listed_context: Option<u64>,
    details: Option<&OllamaShowResponse>,
    running_known: bool,
    running_context: Option<&Option<u64>>,
) -> ModelInfo {
    let capabilities = details
        .map(|details| details.capabilities.as_slice())
        .filter(|capabilities| !capabilities.is_empty())
        .or((!listed_capabilities.is_empty()).then_some(listed_capabilities))
        .map(capability_map)
        .unwrap_or_default();
    let max_context_tokens = details
        .and_then(|details| context_length(&details.model_info))
        .or(listed_context)
        .or(running_context.copied().flatten());
    ModelInfo {
        provider_id: PROVIDER_ID.into(),
        provider_name: PROVIDER_NAME.into(),
        display_name: model_id.clone(),
        model_id,
        max_context_tokens,
        load_state: if running_context.is_some() {
            ModelLoadState::Loaded
        } else if running_known {
            ModelLoadState::Unloaded
        } else {
            ModelLoadState::Unknown
        },
        capabilities,
    }
}

/// Maps Ollama capability labels into the provider-neutral capability shape.
pub(super) fn capability_map(capabilities: &[String]) -> ProviderCapabilities {
    let has = |name: &str| capabilities.iter().any(|capability| capability == name);
    ProviderCapabilities {
        text: has("completion"),
        streaming: has("completion"),
        tools: has("tools"),
        vision: has("vision"),
        embeddings: has("embedding") || has("embeddings"),
    }
}

/// Returns the greatest advertised context length from Ollama model metadata.
fn context_length(model_info: &HashMap<String, Value>) -> Option<u64> {
    model_info
        .iter()
        .filter(|(key, _)| key.ends_with(".context_length"))
        .filter_map(|(_, value)| value.as_u64())
        .max()
}

#[derive(Serialize)]
/// Native Ollama streaming-chat request body.
pub(super) struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatTurn>,
    stream: bool,
    think: OllamaThinkValue,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OllamaToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
#[serde(untagged)]
/// Ollama's boolean-or-level reasoning request value.
enum OllamaThinkValue {
    /// Explicitly disables thinking.
    Enabled(bool),
    /// Requests one supported named reasoning level.
    Level(&'static str),
}

#[derive(Serialize)]
struct OllamaChatTurn {
    role: &'static str,
    content: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    thinking: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OllamaToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
}

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

impl From<ChatRequest> for OllamaChatRequest {
    /// Converts a provider-neutral request into the Ollama wire shape.
    fn from(request: ChatRequest) -> Self {
        let settings = request.settings;
        let options = (settings.temperature.is_some() || settings.max_output_tokens.is_some())
            .then_some(OllamaOptions {
                temperature: settings.temperature,
                num_predict: settings.max_output_tokens,
            });
        Self {
            model: request.model_id,
            messages: request
                .messages
                .into_iter()
                .map(|turn| {
                    let role = match turn.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    };
                    let images = turn
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Image { bytes, .. } => Some(base64_image(bytes)),
                            ContentBlock::Text { .. } => None,
                        })
                        .collect();
                    OllamaChatTurn {
                        role,
                        content: text_content(turn.content),
                        thinking: String::new(),
                        images,
                        tool_calls: Vec::new(),
                        tool_name: None,
                    }
                })
                .collect(),
            stream: true,
            think: match settings.reasoning_effort {
                ReasoningEffort::Off => OllamaThinkValue::Enabled(false),
                ReasoningEffort::Low => OllamaThinkValue::Level("low"),
            },
            tools: Vec::new(),
            options,
        }
    }
}

impl OllamaChatRequest {
    /// Adds Bottie's explicitly enabled closed native definitions to one Ollama request session.
    pub(super) fn with_tools(
        request: ChatRequest,
        definitions: impl IntoIterator<Item = ToolDefinition>,
    ) -> Self {
        let mut request = Self::from(request);
        request.tools = definitions
            .into_iter()
            .map(OllamaToolDefinition::from)
            .collect();
        request
    }

    /// Appends one accumulated assistant call batch and its ordered correlated tool results.
    pub(super) fn append_tool_exchange(
        &mut self,
        thinking: String,
        content: String,
        tool_calls: Vec<OllamaToolCall>,
        results: Vec<OllamaToolResult>,
    ) -> Result<(), ProviderError> {
        if tool_calls.len() != results.len()
            || tool_calls
                .iter()
                .zip(&results)
                .any(|(call, result)| call.tool_name() != result.tool_name)
        {
            return Err(ProviderError::internal(
                "Ollama tool results could not be correlated safely.",
                None,
            ));
        }
        self.messages.push(OllamaChatTurn {
            role: "assistant",
            content,
            thinking,
            images: Vec::new(),
            tool_calls,
            tool_name: None,
        });
        self.messages
            .extend(results.into_iter().map(|result| OllamaChatTurn {
                role: "tool",
                content: result.content,
                thinking: String::new(),
                images: Vec::new(),
                tool_calls: Vec::new(),
                tool_name: Some(result.tool_name),
            }));
        Ok(())
    }
}

#[derive(Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaStreamMessage>,
    #[serde(default)]
    done: bool,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OllamaStreamMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug)]
/// One decoded native stream event.
pub(super) struct DecodedStreamEvent {
    /// Newly generated text.
    pub(super) text_delta: String,
    /// Newly generated reasoning text.
    pub(super) reasoning_delta: String,
    /// Complete tool calls emitted in this stream chunk, in provider order.
    pub(super) tool_calls: Vec<OllamaToolCall>,
    /// Whether the provider marked the stream complete.
    pub(super) done: bool,
    /// Provider-reported prompt token count.
    pub(super) prompt_eval_count: Option<u64>,
    /// Provider-reported generated token count.
    pub(super) eval_count: Option<u64>,
}

/// Decodes one native Ollama NDJSON event without performing I/O.
pub(super) fn decode_stream_line(line: &str) -> Result<DecodedStreamEvent, ProviderError> {
    let chunk: OllamaStreamChunk = serde_json::from_str(line).map_err(|error| {
        ProviderError::malformed(
            "Ollama sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    if let Some(message) = chunk.error.filter(|message| !message.trim().is_empty()) {
        return Err(ProviderError::server(
            message,
            Some("Ollama stream error".into()),
        ));
    }
    let tool_calls = chunk
        .message
        .as_ref()
        .map(|message| message.tool_calls.clone())
        .unwrap_or_default();
    if tool_calls.iter().any(|call| !call.is_valid()) {
        return Err(ProviderError::malformed(
            "Ollama sent an invalid native tool call.",
            Some("tool call name or arguments did not match the Ollama API shape".into()),
        ));
    }
    Ok(DecodedStreamEvent {
        text_delta: chunk
            .message
            .as_ref()
            .map(|message| message.content.clone())
            .unwrap_or_default(),
        reasoning_delta: chunk
            .message
            .as_ref()
            .map(|message| message.thinking.clone())
            .unwrap_or_default(),
        tool_calls,
        done: chunk.done,
        prompt_eval_count: chunk.prompt_eval_count,
        eval_count: chunk.eval_count,
    })
}

/// Normalizes optional provider token counts into an optional usage record.
pub(super) fn normalize_usage(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
) -> Option<Usage> {
    (input_tokens.is_some() || output_tokens.is_some()).then_some(Usage {
        input_tokens,
        output_tokens,
        cost_usd: None,
    })
}

#[derive(Default)]
/// Incremental newline-delimited JSON byte decoder.
pub(super) struct NdjsonDecoder {
    buffer: Vec<u8>,
}

impl NdjsonDecoder {
    /// Appends bytes and returns every newly completed JSON line.
    pub(super) fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(bytes);
        self.drain(false)
    }

    /// Flushes a final unterminated JSON line when the stream closes.
    pub(super) fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        self.drain(true)
    }

    /// Drains complete NDJSON records from the internal byte buffer.
    fn drain(&mut self, finish: bool) -> Result<Vec<String>, ProviderError> {
        let mut lines = Vec::new();
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line = self.buffer.drain(..index).collect::<Vec<_>>();
            self.buffer.drain(..1);
            if let Some(line) = decode_ndjson_line(&line)? {
                lines.push(line);
            }
        }
        if finish && !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            if let Some(line) = decode_ndjson_line(&line)? {
                lines.push(line);
            }
        }
        Ok(lines)
    }
}

/// Decodes one non-empty UTF-8 NDJSON record.
fn decode_ndjson_line(bytes: &[u8]) -> Result<Option<String>, ProviderError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        ProviderError::malformed(
            "Ollama sent invalid text in its stream.",
            Some(error.to_string()),
        )
    })?;
    let line = text.trim();
    Ok((!line.is_empty()).then(|| line.to_owned()))
}

#[derive(Deserialize)]
/// Error body returned by Ollama HTTP endpoints.
pub(super) struct OllamaErrorResponse {
    /// Provider-supplied error message.
    pub(super) error: String,
}
