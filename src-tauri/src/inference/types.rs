use serde::{Deserialize, Serialize};

/// A provider and model pair exposed to the presentation layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub display_name: String,
    pub max_context_tokens: Option<u64>,
    pub capabilities: ProviderCapabilities,
}

/// Capabilities known to be supported by a provider/model pair.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub text: bool,
    pub streaming: bool,
    pub tools: bool,
    pub vision: bool,
}

/// One conversation request accepted by the provider-neutral command.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub model_id: String,
    pub messages: Vec<ChatTurn>,
    #[serde(default)]
    pub settings: ChatSettings,
}

/// A role and ordered content blocks in a conversation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    pub role: ChatRole,
    pub content: Vec<ContentBlock>,
}

/// Roles currently supported by the text-only slice.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// Provider-neutral message content. Later slices can add block variants here.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
}

/// Generation settings shared by compatible text providers.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettings {
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_output_tokens: None,
        }
    }
}

/// Opaque identity returned immediately after a run is accepted.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRun {
    pub run_id: String,
}

/// Usage values normalized across provider responses.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Events delivered over one typed Tauri IPC channel per generation.
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StreamEvent {
    Started {
        run_id: String,
        provider_id: String,
        model_id: String,
    },
    TextDelta {
        run_id: String,
        delta: String,
    },
    UsageUpdated {
        run_id: String,
        usage: Usage,
    },
    Completed {
        run_id: String,
        usage: Option<Usage>,
    },
    Cancelled {
        run_id: String,
    },
    Failed {
        run_id: String,
        error: ProviderError,
    },
}

/// Stable error categories presented to the frontend without provider JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCode {
    Unavailable,
    Timeout,
    InvalidRequest,
    Server,
    MalformedResponse,
    Internal,
}

/// A normalized, user-readable provider failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ProviderError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: ProviderErrorCode::InvalidRequest,
            message: message.into(),
            retryable: false,
            diagnostic: None,
        }
    }

    pub fn unavailable(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::Unavailable,
            message: message.into(),
            retryable: true,
            diagnostic,
        }
    }

    pub fn malformed(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::MalformedResponse,
            message: message.into(),
            retryable: true,
            diagnostic,
        }
    }

    pub fn server(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::Server,
            message: message.into(),
            retryable: true,
            diagnostic,
        }
    }

    pub fn internal(message: impl Into<String>, diagnostic: Option<String>) -> Self {
        Self {
            code: ProviderErrorCode::Internal,
            message: message.into(),
            retryable: false,
            diagnostic,
        }
    }
}
