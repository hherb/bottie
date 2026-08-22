//! Serializable contracts for durable conversation storage.

use serde::{Deserialize, Serialize};

use super::{
    StorageError, attachment_indexing::StoredAttachmentIndexing,
    extraction::StoredAttachmentExtraction, image_normalization::StoredImageNormalization,
    tools::StoredToolInvocation,
};

/// Diagnostic storage policy status used by tests and future recovery UI.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StorageStatus {
    pub(super) schema_version: i64,
    pub(super) profile_name: String,
    pub(super) integrity_check: String,
    pub(super) foreign_keys_enabled: bool,
    pub(super) journal_mode: String,
}

/// Durable message identity supplied when starting one native provider run.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRunContext {
    /// Conversation that owns the request.
    pub(crate) conversation_id: String,
    /// Persisted user message that caused the generation.
    pub(crate) request_message_id: String,
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

/// Local quality rating attached to one durable assistant response.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResponseRating {
    /// The response was useful or otherwise good.
    Good,
    /// The response was not useful or otherwise poor.
    Poor,
}

impl ResponseRating {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Poor => "poor",
        }
    }

    /// Parses a trusted rating constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "good" => Ok(Self::Good),
            "poor" => Ok(Self::Poor),
            _ => Err(StorageError::internal()),
        }
    }
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

/// Durable lifecycle state for one accepted provider generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderRunState {
    /// Provider work has been accepted and has not reached a terminal outcome.
    Running,
    /// The provider completed normally.
    Completed,
    /// The user or application cancelled the provider work.
    Cancelled,
    /// Provider or orchestration work failed.
    Failed,
}

impl ProviderRunState {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Parses a trusted state constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::internal()),
        }
    }

    /// Returns whether the state can close a running provider record.
    pub(super) fn is_terminal(self) -> bool {
        self != Self::Running
    }
}

/// Reasoning setting retained with provider-run provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredReasoningEffort {
    /// Provider reasoning was disabled.
    Off,
    /// The lowest enabled reasoning effort was requested.
    Low,
}

impl StoredReasoningEffort {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
        }
    }

    /// Parses a trusted value constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "off" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Provider-reported token and cost totals retained without estimation.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredUsage {
    /// Provider-reported prompt token count.
    pub(crate) input_tokens: Option<u64>,
    /// Provider-reported generated token count.
    pub(crate) output_tokens: Option<u64>,
    /// Provider-reported request cost in US dollars.
    pub(crate) cost_usd: Option<f64>,
}

impl StoredUsage {
    /// Returns whether the provider supplied at least one usable total.
    pub(super) fn has_value(&self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some() || self.cost_usd.is_some()
    }

    /// Validates numeric values before they reach SQLite constraints.
    pub(super) fn is_valid(&self) -> bool {
        self.cost_usd
            .is_none_or(|cost| cost.is_finite() && cost >= 0.0)
    }
}

/// Persisted provenance reconstructed for one assistant response.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredProviderRun {
    /// Opaque identity created by the native inference boundary.
    pub(crate) id: String,
    /// Current provider-run state.
    pub(crate) state: ProviderRunState,
    /// Reasoning setting applied to the generation.
    pub(crate) reasoning_effort: StoredReasoningEffort,
    /// Native wall-clock time when provider work was accepted.
    pub(crate) started_at_ms: i64,
    /// Native wall-clock time when provider work ended, absent while still running.
    pub(crate) completed_at_ms: Option<i64>,
    /// Stable terminal failure category, absent for successful and cancelled runs.
    pub(crate) error_code: Option<String>,
    /// Latest provider-reported cumulative usage totals, when supplied.
    pub(crate) usage: Option<StoredUsage>,
    /// Ordered native-owned tool calls and any appended results.
    pub(crate) tool_invocations: Vec<StoredToolInvocation>,
}

/// Content-block category accepted from one native provider stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunBlockKind {
    /// User-visible assistant answer text.
    Text,
    /// Separate provider reasoning content.
    Reasoning,
}

impl RunBlockKind {
    /// Returns the stable SQLite block type.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Reasoning => "reasoning",
        }
    }
}

/// Inputs captured when native provider work is accepted.
#[derive(Clone, Debug)]
pub(crate) struct NewProviderRun {
    /// Opaque native run identity.
    pub(crate) id: String,
    /// Owning conversation identity.
    pub(crate) conversation_id: String,
    /// User message that caused the generation.
    pub(crate) request_message_id: String,
    /// Stable routed provider identity.
    pub(crate) provider_id: String,
    /// Provider-owned model identity.
    pub(crate) model_id: String,
    /// Applied reasoning setting.
    pub(crate) reasoning_effort: StoredReasoningEffort,
    /// Applied sampling temperature, when supplied.
    pub(crate) temperature: Option<f32>,
    /// Applied output ceiling, when supplied.
    pub(crate) max_output_tokens: Option<u32>,
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
    /// Whether native long-term memory retrieval excludes this conversation and its associations.
    pub(crate) memory_excluded: bool,
}

/// One native-ranked conversation search result with enough context to reveal its match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationSearchResult {
    /// Stable conversation identity.
    pub(crate) conversation_id: String,
    /// Human-readable conversation title.
    pub(crate) title: String,
    /// Short title or message excerpt containing the match.
    pub(crate) snippet: String,
    /// Branch that reveals the matching message, or the current branch for title matches.
    pub(crate) branch_id: String,
    /// Last persisted conversation activity time.
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
#[derive(Clone, Debug, PartialEq, Serialize)]
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
    /// Provider-run provenance and usage linked to an assistant response.
    pub(crate) provider_run: Option<StoredProviderRun>,
    /// Current local quality rating for an assistant response.
    pub(crate) rating: Option<ResponseRating>,
    /// Ordered retained files associated with this user message.
    pub(crate) attachments: Vec<StoredAttachment>,
    /// Persisted creation time.
    pub(crate) created_at_ms: i64,
}

/// Path-free retained attachment metadata reconstructed with a durable message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredAttachment {
    /// Opaque native attachment identity used by narrow association commands.
    pub(crate) id: String,
    /// Sanitized leaf name safe for inert presentation.
    pub(crate) display_name: String,
    /// MIME type inferred from retained content.
    pub(crate) mime_type: String,
    /// Exact retained byte count.
    pub(crate) byte_size: u64,
    /// Lowercase SHA-256 content identity.
    #[serde(skip_serializing)]
    pub(crate) sha256: String,
    /// Native-only extraction status; extracted content is deliberately omitted.
    pub(crate) extraction: StoredAttachmentExtraction,
    /// Readiness for later native text indexing; indexed content does not exist yet.
    pub(crate) indexing: StoredAttachmentIndexing,
    /// Native-only image normalization status; derivative bytes and paths are omitted.
    pub(crate) normalization: StoredImageNormalization,
}

/// Complete durable conversation returned for reopening.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredConversation {
    /// Stable conversation identity.
    pub(crate) id: String,
    /// Human-readable title.
    pub(crate) title: String,
    /// Selected durable branch identity.
    pub(crate) current_branch_id: String,
    /// Available branches ordered by creation.
    pub(crate) branches: Vec<ConversationBranch>,
    /// Ordered retained files shared by every branch and future request in this conversation.
    pub(crate) attachments: Vec<StoredAttachment>,
    /// Ordered messages on the selected branch lineage.
    pub(crate) messages: Vec<StoredMessage>,
}

/// One selectable conversation branch exposed without its storage internals.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationBranch {
    /// Stable opaque branch identity.
    pub(crate) id: String,
    /// Human-readable branch label.
    pub(crate) name: String,
}

/// Result of atomically forking one user request onto a new selected branch.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForkedConversation {
    /// Reconstructed conversation on the newly selected branch.
    pub(crate) conversation: StoredConversation,
    /// New durable user request that should start provider generation.
    pub(crate) request_message_id: String,
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
