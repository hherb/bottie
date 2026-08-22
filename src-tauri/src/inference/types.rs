use serde::{Deserialize, Serialize};

/// Default completion ceiling used when the interface does not supply one.
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4_096;

/// A provider and model pair exposed to the presentation layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Stable provider identity used for routing.
    pub provider_id: String,
    /// Provider display name shown to the user.
    pub provider_name: String,
    /// Provider-owned model identity.
    pub model_id: String,
    /// Human-readable model name.
    pub display_name: String,
    /// Advertised context limit when the provider reports one.
    pub max_context_tokens: Option<u64>,
    /// Whether a local model is currently resident in memory.
    pub load_state: ModelLoadState,
    /// Features advertised for this provider/model pair.
    pub capabilities: ProviderCapabilities,
}

/// Whether a locally installed model is currently resident in memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLoadState {
    /// The provider reports the model as resident in memory.
    Loaded,
    /// The provider reports the model as installed but not resident.
    Unloaded,
    #[default]
    /// The provider does not expose model residency.
    Unknown,
}

/// Capabilities known to be supported by a provider/model pair.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    /// Whether the model accepts and produces text.
    pub text: bool,
    /// Whether responses can be streamed incrementally.
    pub streaming: bool,
    /// Whether the model supports tool calls.
    pub tools: bool,
    /// Whether the model accepts image content.
    pub vision: bool,
    /// Whether the model can create embeddings.
    pub embeddings: bool,
}

/// One conversation request accepted by the provider-neutral command.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    /// Stable provider identity used to route the request.
    pub provider_id: String,
    /// Provider-owned model identity.
    pub model_id: String,
    /// Ordered conversation turns supplied to the model.
    pub messages: Vec<ChatTurn>,
    #[serde(default)]
    /// Whether this request may advertise Bottie's native memory tools to a compatible local provider.
    pub memory_enabled: bool,
    #[serde(default)]
    /// Optional provider-neutral generation settings.
    pub settings: ChatSettings,
}

/// A role and ordered content blocks in a conversation.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    /// Participant role for this turn.
    pub role: ChatRole,
    /// Ordered content blocks in this turn.
    pub content: Vec<ContentBlock>,
}

/// Roles currently supported by durable chat context.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    /// Instruction supplied by the application.
    System,
    /// Input supplied by the user.
    User,
    /// Output supplied by the assistant.
    Assistant,
}

/// Provider-neutral message content prepared behind the native boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text message content.
    Text { text: String },
    #[serde(skip_deserializing)]
    /// Metadata-free normalized image bytes loaded only by Rust-owned storage.
    Image {
        /// Normalized encoding forwarded to compatible provider wire formats.
        media_type: ImageMediaType,
        /// Bounded derivative bytes that never cross into the WebView.
        bytes: Vec<u8>,
    },
}

/// Native image encodings accepted by provider delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageMediaType {
    /// Metadata-free JPEG derivative.
    Jpeg,
    /// Metadata-free PNG derivative.
    Png,
}

impl ImageMediaType {
    /// Returns the MIME value required by provider image blocks.
    pub(crate) fn as_mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

/// Generation settings shared by compatible text providers.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChatSettings {
    /// Optional provider sampling temperature.
    pub temperature: Option<f32>,
    /// Optional maximum number of generated tokens.
    pub max_output_tokens: Option<u32>,
    /// Requested thinking depth, kept deliberately narrow for the first reasoning slice.
    pub reasoning_effort: ReasoningEffort,
}

/// Provider-neutral reasoning levels exposed by the current interface.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    /// Ask the provider to suppress model reasoning.
    #[default]
    Off,
    /// Enable the provider's lowest supported reasoning effort.
    Low,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_output_tokens: Some(DEFAULT_MAX_OUTPUT_TOKENS),
            reasoning_effort: ReasoningEffort::Off,
        }
    }
}

/// Opaque identity returned immediately after a run is accepted.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRun {
    /// Opaque unique identity for the accepted run.
    pub run_id: String,
}

/// Usage values normalized across provider responses.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Provider-reported prompt token count.
    pub input_tokens: Option<u64>,
    /// Provider-reported generated token count.
    pub output_tokens: Option<u64>,
    /// Provider-reported request cost in US dollars, when a compatible endpoint supplies it.
    pub cost_usd: Option<f64>,
}

/// Events delivered over one typed Tauri IPC channel per generation.
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StreamEvent {
    /// The native task accepted the generation and started provider work.
    Started {
        /// Opaque identity of the generation.
        run_id: String,
        /// Stable provider identity used by the generation.
        provider_id: String,
        /// Provider-owned model identity used by the generation.
        model_id: String,
    },
    /// The provider produced one text fragment.
    TextDelta {
        /// Opaque identity of the generation.
        run_id: String,
        /// Newly produced text.
        delta: String,
    },
    /// The provider produced one reasoning fragment kept separate from answer text.
    ReasoningDelta {
        /// Opaque identity of the generation.
        run_id: String,
        /// Newly produced reasoning text.
        delta: String,
    },
    /// The provider supplied updated usage totals.
    UsageUpdated {
        /// Opaque identity of the generation.
        run_id: String,
        /// Latest normalized usage values.
        usage: Usage,
    },
    /// The provider completed the generation successfully.
    Completed {
        /// Opaque identity of the generation.
        run_id: String,
        /// Final usage values when supplied by the provider.
        usage: Option<Usage>,
    },
    /// The user or application cancelled the generation.
    Cancelled {
        /// Opaque identity of the generation.
        run_id: String,
    },
    /// The generation ended with a normalized provider failure.
    Failed {
        /// Opaque identity of the generation.
        run_id: String,
        /// User-readable normalized failure.
        error: ProviderError,
    },
}

/// Stable error categories presented to the frontend without provider JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCode {
    /// The provider cannot currently be reached or used.
    Unavailable,
    /// The provider exceeded an enforced timeout.
    Timeout,
    /// The request or selected resource was invalid.
    InvalidRequest,
    /// The provider reported a retryable server failure.
    Server,
    /// The provider response did not match its protocol.
    MalformedResponse,
    /// Bottie's native orchestration failed internally.
    Internal,
}

impl ProviderErrorCode {
    /// Returns the stable storage representation used by provider-run provenance.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::InvalidRequest => "invalid_request",
            Self::Server => "server",
            Self::MalformedResponse => "malformed_response",
            Self::Internal => "internal",
        }
    }
}

/// A normalized, user-readable provider failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    /// Stable category used by presentation logic.
    pub code: ProviderErrorCode,
    /// User-readable failure description.
    pub message: String,
    /// Whether retrying the same operation may succeed.
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Secret-redacted technical context for diagnostics.
    pub diagnostic: Option<String>,
}

impl ProviderError {
    /// Builds a non-retryable invalid-request failure.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: ProviderErrorCode::InvalidRequest,
            message: message.into(),
            retryable: false,
            diagnostic: None,
        }
    }

    /// Builds a retryable unavailable-provider failure.
    pub fn unavailable(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::Unavailable,
            message: message.into(),
            retryable: true,
            diagnostic,
        }
    }

    /// Builds a retryable malformed-response failure.
    pub fn malformed(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::MalformedResponse,
            message: message.into(),
            retryable: true,
            diagnostic,
        }
    }

    /// Builds a retryable provider-server failure.
    pub fn server(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::Server,
            message: message.into(),
            retryable: true,
            diagnostic,
        }
    }

    /// Builds a non-retryable native orchestration failure.
    pub fn internal(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::Internal,
            message: message.into(),
            retryable: false,
            diagnostic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview_requests_cannot_inject_native_image_bytes() {
        let request = serde_json::from_value::<ChatRequest>(serde_json::json!({
            "providerId": "ollama",
            "modelId": "vision-model",
            "messages": [{
                "role": "user",
                "content": [{"type": "image", "mediaType": "png", "bytes": [1, 2, 3]}]
            }]
        }));

        assert!(request.is_err());
    }

    #[test]
    fn memory_tools_require_an_explicit_request_flag() {
        let disabled: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "ollama",
            "modelId": "tool-model",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hello"}]}]
        }))
        .expect("legacy request should deserialize");
        let enabled: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "ollama",
            "modelId": "tool-model",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hello"}]}],
            "memoryEnabled": true
        }))
        .expect("explicit memory request should deserialize");

        assert!(!disabled.memory_enabled);
        assert!(enabled.memory_enabled);
    }
}
