//! Deterministic Markdown and JSON rendering for one selected durable conversation lineage.

use std::{fmt::Write, path::Path};

use serde::Serialize;

use super::{
    ConversationStore, MessageState, ProviderRunState, ResponseRating, StorageError,
    StoredConversation, StoredMessage, StoredReasoningEffort, StoredRole, StoredUsage,
    load_conversation_from_connection,
};

const MAX_EXPORT_FILENAME_SLUG_CHARACTERS: usize = 64;
const EXPORT_FILENAME_PREFIX: &str = "bottie-";
const MARKDOWN_FILENAME_EXTENSION: &str = ".md";
const JSON_FILENAME_EXTENSION: &str = ".json";
const JSON_EXPORT_FORMAT: &str = "bottie-conversation";
const JSON_EXPORT_VERSION: u8 = 1;

/// Native-only file payload prepared before Bottie opens a save dialog.
pub(crate) struct ConversationFileExport {
    /// Safe suggested leaf filename that reveals no local directory.
    pub(crate) file_name: String,
    /// Complete UTF-8 document to write after user confirmation.
    pub(crate) contents: String,
}

impl ConversationFileExport {
    /// Writes the complete UTF-8 document while mapping filesystem details to a path-redacted error.
    pub(crate) fn write_to(&self, path: &Path) -> Result<(), StorageError> {
        std::fs::write(path, &self.contents).map_err(|_| StorageError::export())
    }
}

impl ConversationStore {
    /// Prepares the current visible lineage without changing the profile's open-conversation selection.
    pub(crate) fn prepare_markdown_export(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationFileExport, StorageError> {
        let connection = self.open()?;
        let conversation = load_conversation_from_connection(&connection, conversation_id)?;
        Ok(markdown_export(&conversation))
    }

    /// Prepares portable JSON for the visible lineage without changing profile selection.
    pub(crate) fn prepare_json_export(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationFileExport, StorageError> {
        let connection = self.open()?;
        let conversation = load_conversation_from_connection(&connection, conversation_id)?;
        json_export(&conversation)
    }
}

/// Builds one native-only Markdown export payload from a reconstructed conversation.
pub(super) fn markdown_export(conversation: &StoredConversation) -> ConversationFileExport {
    ConversationFileExport {
        file_name: export_file_name(&conversation.title, MARKDOWN_FILENAME_EXTENSION),
        contents: render_conversation_markdown(conversation),
    }
}

/// Builds one native-only JSON export payload from a reconstructed conversation.
pub(super) fn json_export(
    conversation: &StoredConversation,
) -> Result<ConversationFileExport, StorageError> {
    Ok(ConversationFileExport {
        file_name: export_file_name(&conversation.title, JSON_FILENAME_EXTENSION),
        contents: render_conversation_json(conversation)?,
    })
}

/// Versioned portable JSON document that deliberately excludes opaque storage identifiers.
#[derive(Serialize)]
struct JsonConversationExport<'a> {
    /// Stable format discriminator for future import compatibility.
    format: &'static str,
    /// Portable export contract version, independent from SQLite schema versions.
    version: u8,
    /// Human-readable conversation title.
    title: &'a str,
    /// Ordered messages reconstructed from the selected visible lineage.
    messages: Vec<JsonMessageExport<'a>>,
}

/// Portable representation of one selected-lineage message.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMessageExport<'a> {
    /// Conversation participant role.
    role: StoredRole,
    /// Exact user-authored or provider-generated text.
    text: &'a str,
    /// Separate provider reasoning, when retained.
    reasoning: Option<&'a str>,
    /// Durable completion state.
    state: MessageState,
    /// Provider identity for assistant output, when present.
    provider_id: Option<&'a str>,
    /// Provider-owned model identity for assistant output, when present.
    model_id: Option<&'a str>,
    /// Provider-run provenance excluding its opaque native identifier.
    generation: Option<JsonGenerationExport<'a>>,
    /// Local response rating, when present.
    rating: Option<ResponseRating>,
    /// Persisted creation time as Unix milliseconds.
    created_at_ms: i64,
}

/// Portable generation provenance without native run or request identifiers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonGenerationExport<'a> {
    /// Terminal provider-run state retained by Bottie.
    state: ProviderRunState,
    /// Reasoning setting applied to the request.
    reasoning_effort: StoredReasoningEffort,
    /// Native wall-clock start time as Unix milliseconds.
    started_at_ms: i64,
    /// Native wall-clock completion time as Unix milliseconds, when terminal.
    completed_at_ms: Option<i64>,
    /// Stable terminal failure category, when present.
    error_code: Option<&'a str>,
    /// Provider-reported token and cost totals, without estimates.
    usage: Option<&'a StoredUsage>,
}

/// Renders deterministic pretty JSON and a trailing newline for portable text tooling.
pub(super) fn render_conversation_json(
    conversation: &StoredConversation,
) -> Result<String, StorageError> {
    let export = JsonConversationExport {
        format: JSON_EXPORT_FORMAT,
        version: JSON_EXPORT_VERSION,
        title: &conversation.title,
        messages: conversation
            .messages
            .iter()
            .map(JsonMessageExport::from)
            .collect(),
    };
    let mut json = serde_json::to_string_pretty(&export).map_err(|_| StorageError::export())?;
    json.push('\n');
    Ok(json)
}

impl<'a> From<&'a StoredMessage> for JsonMessageExport<'a> {
    /// Removes opaque message and provider-run identities while preserving portable provenance.
    fn from(message: &'a StoredMessage) -> Self {
        Self {
            role: message.role,
            text: &message.text,
            reasoning: message.reasoning.as_deref(),
            state: message.state,
            provider_id: message.provider_id.as_deref(),
            model_id: message.model_id.as_deref(),
            generation: message
                .provider_run
                .as_ref()
                .map(JsonGenerationExport::from),
            rating: message.rating,
            created_at_ms: message.created_at_ms,
        }
    }
}

impl<'a> From<&'a super::StoredProviderRun> for JsonGenerationExport<'a> {
    /// Removes the native run ID while retaining its user-meaningful settings and outcome.
    fn from(run: &'a super::StoredProviderRun) -> Self {
        Self {
            state: run.state,
            reasoning_effort: run.reasoning_effort,
            started_at_ms: run.started_at_ms,
            completed_at_ms: run.completed_at_ms,
            error_code: run.error_code.as_deref(),
            usage: run.usage.as_ref(),
        }
    }
}

/// Renders user content verbatim and assistant Markdown with explicit reasoning and response sections.
pub(super) fn render_conversation_markdown(conversation: &StoredConversation) -> String {
    let mut markdown = format!("# {}\n", conversation.title);
    for message in &conversation.messages {
        markdown.push_str("\n## ");
        markdown.push_str(match message.role {
            StoredRole::User => "User",
            StoredRole::Assistant => "Assistant",
        });
        markdown.push_str("\n\n");
        if message.role == StoredRole::Assistant {
            write_assistant_metadata(&mut markdown, message);
            write_assistant_content(&mut markdown, message);
        } else {
            markdown.push_str(&message.text);
            markdown.push('\n');
        }
    }
    markdown
}

/// Writes provider, model, non-final state, and local rating without exposing internal identifiers.
fn write_assistant_metadata(markdown: &mut String, message: &StoredMessage) {
    let mut metadata = Vec::new();
    if let Some(provider_id) = message.provider_id.as_deref() {
        metadata.push(format!("Provider: {}", inline_code(provider_id)));
    }
    if let Some(model_id) = message.model_id.as_deref() {
        metadata.push(format!("Model: {}", inline_code(model_id)));
    }
    if let Some(status) = export_status(message.state) {
        metadata.push(format!("Status: {status}"));
    }
    if let Some(rating) = message.rating {
        metadata.push(format!(
            "Rating: {}",
            match rating {
                ResponseRating::Good => "Good",
                ResponseRating::Poor => "Poor",
            }
        ));
    }
    for (index, line) in metadata.iter().enumerate() {
        let hard_break = if index + 1 == metadata.len() {
            ""
        } else {
            "  "
        };
        writeln!(markdown, "> {line}{hard_break}").expect("writing to a string cannot fail");
    }
    if !metadata.is_empty() {
        markdown.push('\n');
    }
}

/// Writes reasoning separately from the assistant response while preserving their exact source text.
fn write_assistant_content(markdown: &mut String, message: &StoredMessage) {
    if let Some(reasoning) = message.reasoning.as_deref() {
        markdown.push_str("### Reasoning\n\n");
        markdown.push_str(reasoning);
        markdown.push_str("\n\n");
        if !message.text.is_empty() {
            markdown.push_str("### Response\n\n");
        }
    }
    markdown.push_str(&message.text);
    markdown.push('\n');
}

/// Maps retained incomplete outcomes to stable human-readable export labels.
fn export_status(state: MessageState) -> Option<&'static str> {
    match state {
        MessageState::Partial => Some("Interrupted"),
        MessageState::Cancelled => Some("Cancelled"),
        MessageState::Failed => Some("Failed"),
        MessageState::Final => None,
    }
}

/// Wraps one normalized metadata value in a code span that tolerates embedded backticks.
fn inline_code(value: &str) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let longest_run = value
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest_run + 1);
    if value.starts_with('`')
        || value.starts_with(' ')
        || value.ends_with('`')
        || value.ends_with(' ')
    {
        format!("{fence} {value} {fence}")
    } else {
        format!("{fence}{value}{fence}")
    }
}

/// Produces a bounded cross-platform filename from a user-controlled title and trusted extension.
fn export_file_name(title: &str, extension: &str) -> String {
    let mut slug = String::new();
    let mut separator_pending = false;
    for character in title.chars() {
        if character.is_alphanumeric() {
            if separator_pending && !slug.is_empty() {
                slug.push('-');
            }
            for lowercase in character.to_lowercase() {
                if slug.chars().count() >= MAX_EXPORT_FILENAME_SLUG_CHARACTERS {
                    break;
                }
                slug.push(lowercase);
            }
            separator_pending = false;
        } else if !slug.is_empty() {
            separator_pending = true;
        }
        if slug.chars().count() >= MAX_EXPORT_FILENAME_SLUG_CHARACTERS {
            break;
        }
    }
    if slug.is_empty() {
        slug.push_str("conversation");
    }
    format!("{EXPORT_FILENAME_PREFIX}{slug}{extension}")
}
