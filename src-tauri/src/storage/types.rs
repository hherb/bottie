//! Serializable contracts and stable errors for durable conversation storage.

use serde::{Deserialize, Serialize};

/// Stable storage failure returned across the native command boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageError {
    /// Stable machine-readable failure category.
    pub(crate) code: &'static str,
    /// Human-readable failure safe to show in the interface.
    pub(crate) message: String,
}

impl StorageError {
    /// Creates an invalid-input failure.
    pub(super) fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_request",
            message: message.into(),
        }
    }

    /// Creates a missing-record failure.
    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: "not_found",
            message: message.into(),
        }
    }

    /// Creates an internal storage failure without exposing SQL or local paths.
    pub(super) fn internal() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not access its local conversation store.".into(),
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(_: rusqlite::Error) -> Self {
        Self::internal()
    }
}

impl From<std::io::Error> for StorageError {
    fn from(_: std::io::Error) -> Self {
        Self::internal()
    }
}

/// Role persisted for one conversation message.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredRole {
    /// Human-authored prompt content.
    User,
    /// Provider-generated assistant content.
    Assistant,
}

impl StoredRole {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    /// Parses a trusted role constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Durable completion state for one message.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MessageState {
    /// A retained incomplete provider response.
    Partial,
    /// A completed user or assistant message.
    Final,
    /// A response stopped by the user.
    Cancelled,
    /// A response terminated by a provider or orchestration failure.
    Failed,
}

impl MessageState {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Partial => "partial",
            Self::Final => "final",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Parses a trusted state constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "partial" => Ok(Self::Partial),
            "final" => Ok(Self::Final),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Summary used by conversation navigation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationSummary {
    /// Stable conversation identity.
    pub(crate) id: String,
    /// Human-readable title.
    pub(crate) title: String,
    /// Last persisted activity time.
    pub(crate) updated_at_ms: i64,
    /// Current soft lifecycle state.
    pub(crate) lifecycle: ConversationLifecycle,
}

/// Soft lifecycle state used to organize recoverable conversations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConversationLifecycle {
    /// Visible in the recent conversation list.
    Active,
    /// Retained outside the recent list until unarchived or reactivated by append.
    Archived,
    /// Hidden from normal navigation but available for restore.
    Deleted,
}

impl ConversationLifecycle {
    /// Parses the lifecycle derived by trusted storage queries.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(StorageError::internal()),
        }
    }
}

/// One reconstructed text message returned to the WebView.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredMessage {
    /// Stable message identity.
    pub(crate) id: String,
    /// Message participant role.
    pub(crate) role: StoredRole,
    /// Ordered plain-text content.
    pub(crate) text: String,
    /// Optional reasoning content kept separate from the answer.
    pub(crate) reasoning: Option<String>,
    /// Durable completion state.
    pub(crate) state: MessageState,
    /// Provider used for an assistant response.
    pub(crate) provider_id: Option<String>,
    /// Provider-owned model identity used for an assistant response.
    pub(crate) model_id: Option<String>,
    /// Persisted creation time.
    pub(crate) created_at_ms: i64,
}

/// Complete durable conversation returned for reopening.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredConversation {
    /// Stable conversation identity.
    pub(crate) id: String,
    /// Human-readable title.
    pub(crate) title: String,
    /// Ordered messages on the current main branch.
    pub(crate) messages: Vec<StoredMessage>,
}

/// One message submitted for durable append.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewStoredMessage {
    /// Owning conversation identity.
    pub(crate) conversation_id: String,
    /// Participant role.
    pub(crate) role: StoredRole,
    /// Plain-text message content.
    pub(crate) text: String,
    /// Optional separate reasoning content.
    pub(crate) reasoning: Option<String>,
    /// Durable completion state.
    pub(crate) state: MessageState,
    /// Provider used for assistant generation.
    pub(crate) provider_id: Option<String>,
    /// Model used for assistant generation.
    pub(crate) model_id: Option<String>,
}
