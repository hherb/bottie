//! Deterministic Markdown rendering for one selected durable conversation lineage.

use std::{fmt::Write, path::Path};

use super::{
    ConversationStore, MessageState, ResponseRating, StorageError, StoredConversation,
    StoredMessage, StoredRole, load_conversation_from_connection,
};

const MAX_EXPORT_FILENAME_SLUG_CHARACTERS: usize = 64;
const EXPORT_FILENAME_PREFIX: &str = "bottie-";
const EXPORT_FILENAME_EXTENSION: &str = ".md";

/// Native-only file payload prepared before Bottie opens a save dialog.
pub(crate) struct ConversationMarkdownExport {
    /// Safe suggested leaf filename that reveals no local directory.
    pub(crate) file_name: String,
    /// Complete UTF-8 Markdown document to write after user confirmation.
    pub(crate) contents: String,
}

impl ConversationMarkdownExport {
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
    ) -> Result<ConversationMarkdownExport, StorageError> {
        let connection = self.open()?;
        let conversation = load_conversation_from_connection(&connection, conversation_id)?;
        Ok(markdown_export(&conversation))
    }
}

/// Builds one native-only Markdown export payload from a reconstructed conversation.
pub(super) fn markdown_export(conversation: &StoredConversation) -> ConversationMarkdownExport {
    ConversationMarkdownExport {
        file_name: export_file_name(&conversation.title),
        contents: render_conversation_markdown(conversation),
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

/// Produces a bounded cross-platform Markdown filename from a user-controlled title.
fn export_file_name(title: &str) -> String {
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
    format!("{EXPORT_FILENAME_PREFIX}{slug}{EXPORT_FILENAME_EXTENSION}")
}
