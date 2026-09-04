//! Deterministic Markdown and JSON rendering for selected durable conversation lineages.

use std::fmt::Write;

use serde::Serialize;

use super::{
    ConversationLifecycle, ConversationStore, DEFAULT_PROFILE_ID, MessageState, ProviderRunState,
    ResponseRating, StorageError, StoredConversation, StoredMessage, StoredReasoningEffort,
    StoredRole, StoredUsage, load_conversation_from_connection,
    portable_export::{
        ConversationFileExport, PortableAttachmentReference, portable_attachment_reference,
        write_attachment_markdown_section,
    },
};

const MAX_EXPORT_FILENAME_SLUG_CHARACTERS: usize = 64;
const EXPORT_FILENAME_PREFIX: &str = "bottie-";
const MARKDOWN_FILENAME_EXTENSION: &str = ".md";
const JSON_FILENAME_EXTENSION: &str = ".json";
const JSON_EXPORT_FORMAT: &str = "bottie-conversation";
const JSON_EXPORT_VERSION: u8 = 5;
const BATCH_JSON_EXPORT_FILE_NAME: &str = "bottie-conversations.json";
const BATCH_JSON_EXPORT_FORMAT: &str = "bottie-conversation-batch";
const BATCH_JSON_EXPORT_VERSION: u8 = 5;

impl ConversationStore {
    /// Prepares the current visible lineage without changing the profile's open-conversation selection.
    pub(crate) fn prepare_markdown_export(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationFileExport, StorageError> {
        let connection = self.open()?;
        let conversation = load_conversation_from_connection(&connection, conversation_id)?;
        let export = markdown_export(&conversation);
        Ok(self.bundle_export(export, &[&conversation]))
    }

    /// Prepares portable JSON for the visible lineage without changing profile selection.
    pub(crate) fn prepare_json_export(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationFileExport, StorageError> {
        let connection = self.open()?;
        let conversation = load_conversation_from_connection(&connection, conversation_id)?;
        let export = json_export(&conversation)?;
        Ok(self.bundle_export(export, &[&conversation]))
    }

    /// Prepares every non-deleted conversation's selected lineage as one portable JSON document.
    pub(crate) fn prepare_batch_json_export(&self) -> Result<ConversationFileExport, StorageError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let rows = {
            let mut statement = transaction.prepare(
                "SELECT id, updated_at_ms,
                        CASE WHEN archived_at_ms IS NULL THEN 'active' ELSE 'archived' END
                 FROM conversations
                 WHERE profile_id = ?1 AND deleted_at_ms IS NULL
                 ORDER BY CASE WHEN archived_at_ms IS NULL THEN 0 ELSE 1 END,
                          updated_at_ms DESC, id DESC",
            )?;
            statement
                .query_map([DEFAULT_PROFILE_ID], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        if rows.is_empty() {
            return Err(StorageError::not_found(
                "There are no active or archived conversations to export.",
            ));
        }
        let conversations = rows
            .into_iter()
            .map(|(id, updated_at_ms, lifecycle)| {
                Ok(BatchConversationExport {
                    conversation: load_conversation_from_connection(&transaction, &id)?,
                    lifecycle: ConversationLifecycle::from_database(&lifecycle)?,
                    updated_at_ms,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let export = batch_json_export(&conversations)?;
        transaction.commit()?;
        let conversation_refs = conversations
            .iter()
            .map(|item| &item.conversation)
            .collect::<Vec<_>>();
        Ok(self.bundle_export(export, &conversation_refs))
    }
}

/// Builds one native-only Markdown export payload from a reconstructed conversation.
pub(super) fn markdown_export(conversation: &StoredConversation) -> ConversationFileExport {
    ConversationFileExport::document(
        export_file_name(&conversation.title, MARKDOWN_FILENAME_EXTENSION),
        render_conversation_markdown(conversation),
    )
}

/// Builds one native-only JSON export payload from a reconstructed conversation.
pub(super) fn json_export(
    conversation: &StoredConversation,
) -> Result<ConversationFileExport, StorageError> {
    Ok(ConversationFileExport::document(
        export_file_name(&conversation.title, JSON_FILENAME_EXTENSION),
        render_conversation_json(conversation)?,
    ))
}

/// Builds one native-only JSON payload from ordered non-deleted selected lineages.
fn batch_json_export(
    conversations: &[BatchConversationExport],
) -> Result<ConversationFileExport, StorageError> {
    Ok(ConversationFileExport::document(
        BATCH_JSON_EXPORT_FILE_NAME.into(),
        render_conversation_batch_json(conversations)?,
    ))
}

/// Native-only conversation metadata required by the batch export contract.
struct BatchConversationExport {
    /// Reconstructed selected lineage.
    conversation: StoredConversation,
    /// Active or archived lifecycle retained for portable organization.
    lifecycle: ConversationLifecycle,
    /// Last persisted conversation activity time.
    updated_at_ms: i64,
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
    /// Branch-independent conversation attachment scope.
    attachments: Vec<PortableAttachmentReference>,
    /// Ordered messages reconstructed from the selected visible lineage.
    messages: Vec<JsonMessageExport<'a>>,
}

/// Versioned multi-conversation JSON document without database identities or trashed records.
#[derive(Serialize)]
struct JsonConversationBatchExport<'a> {
    /// Stable format discriminator distinct from a single-conversation document.
    format: &'static str,
    /// Portable batch contract version, independent from SQLite schema versions.
    version: u8,
    /// Active then archived conversations in deterministic recent-first order.
    conversations: Vec<JsonBatchConversationExport<'a>>,
}

/// One conversation inside the portable batch document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonBatchConversationExport<'a> {
    /// Human-readable conversation title.
    title: &'a str,
    /// Active or archived lifecycle at export time.
    lifecycle: ConversationLifecycle,
    /// Last persisted activity time as Unix milliseconds.
    updated_at_ms: i64,
    /// Branch-independent conversation attachment scope.
    attachments: Vec<PortableAttachmentReference>,
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
    /// Ordered retained files associated with this selected-lineage message.
    attachments: Vec<PortableAttachmentReference>,
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
    /// Ordered tool calls and appended results without native or provider call identities.
    tool_invocations: &'a [super::tools::StoredToolInvocation],
}

/// Renders deterministic pretty JSON and a trailing newline for portable text tooling.
pub(super) fn render_conversation_json(
    conversation: &StoredConversation,
) -> Result<String, StorageError> {
    let export = JsonConversationExport {
        format: JSON_EXPORT_FORMAT,
        version: JSON_EXPORT_VERSION,
        title: &conversation.title,
        attachments: conversation
            .attachments
            .iter()
            .map(portable_attachment_reference)
            .collect(),
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

/// Renders deterministic pretty JSON for all eligible conversations and appends one trailing newline.
fn render_conversation_batch_json(
    conversations: &[BatchConversationExport],
) -> Result<String, StorageError> {
    let export = JsonConversationBatchExport {
        format: BATCH_JSON_EXPORT_FORMAT,
        version: BATCH_JSON_EXPORT_VERSION,
        conversations: conversations
            .iter()
            .map(|item| JsonBatchConversationExport {
                title: &item.conversation.title,
                lifecycle: item.lifecycle,
                updated_at_ms: item.updated_at_ms,
                attachments: item
                    .conversation
                    .attachments
                    .iter()
                    .map(portable_attachment_reference)
                    .collect(),
                messages: item
                    .conversation
                    .messages
                    .iter()
                    .map(JsonMessageExport::from)
                    .collect(),
            })
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
            attachments: message
                .attachments
                .iter()
                .map(portable_attachment_reference)
                .collect(),
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
            tool_invocations: &run.tool_invocations,
        }
    }
}

/// Renders user content verbatim and assistant Markdown with explicit reasoning and response sections.
pub(super) fn render_conversation_markdown(conversation: &StoredConversation) -> String {
    let mut markdown = format!("# {}\n", conversation.title);
    write_attachment_markdown_section(
        &mut markdown,
        &conversation.attachments,
        "Conversation attachments",
    );
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
            write_attachment_markdown_section(&mut markdown, &message.attachments, "Attachments");
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
    }
    let tools = message
        .provider_run
        .as_ref()
        .map(|run| run.tool_invocations.as_slice())
        .unwrap_or_default();
    write_tool_activity(markdown, tools);
    if !message.text.is_empty() && (message.reasoning.is_some() || !tools.is_empty()) {
        markdown.push_str("### Response\n\n");
    }
    markdown.push_str(&message.text);
    markdown.push('\n');
}

/// Writes ordered tool arguments and outcomes as inert structured JSON.
fn write_tool_activity(markdown: &mut String, tools: &[super::tools::StoredToolInvocation]) {
    if tools.is_empty() {
        return;
    }
    markdown.push_str("### Tool activity\n\n");
    for tool in tools {
        writeln!(markdown, "#### {}\n", inline_code(&tool.tool_name))
            .expect("writing to a string cannot fail");
        write_tool_audit(markdown, &tool.audit);
        markdown.push_str("**Arguments**\n\n");
        write_json_fence(markdown, &tool.arguments);
        match &tool.result {
            Some(result) => {
                markdown.push_str(if result.is_error {
                    "**Error result**\n\n"
                } else {
                    "**Result**\n\n"
                });
                write_json_fence(markdown, &result.output);
            }
            None => markdown.push_str("**Result:** Pending\n\n"),
        }
    }
}

/// Writes one compact provider-neutral audit summary without opaque call identity.
fn write_tool_audit(markdown: &mut String, audit: &super::tools::StoredToolAudit) {
    let policy = match audit.policy {
        super::tools::ToolAuditPolicy::Legacy => "Legacy record",
        super::tools::ToolAuditPolicy::Safe => "Read-only",
        super::tools::ToolAuditPolicy::ApprovalRequired => "Approval required",
        super::tools::ToolAuditPolicy::Unregistered => "Unregistered",
    };
    let outcome = match audit.outcome {
        None => "Pending",
        Some(super::tools::ToolAuditOutcome::Success) => "Succeeded",
        Some(super::tools::ToolAuditOutcome::UnsupportedTool) => "Unsupported tool",
        Some(super::tools::ToolAuditOutcome::InvalidArguments) => "Invalid arguments",
        Some(super::tools::ToolAuditOutcome::ApprovalRequired) => "Approval required",
        Some(super::tools::ToolAuditOutcome::Unavailable) => "Source unavailable",
        Some(super::tools::ToolAuditOutcome::ExecutionFailed) => "Execution failed",
        Some(super::tools::ToolAuditOutcome::OutputTooLarge) => "Output too large",
        Some(super::tools::ToolAuditOutcome::LegacyError) => "Legacy error",
    };
    writeln!(markdown, "**Audit:** {policy} · {outcome}").expect("writing to a string cannot fail");
    if let Some(approval) = &audit.approval {
        let decision = match approval.decision {
            super::ToolApprovalDecision::Approved => "Approved once",
            super::ToolApprovalDecision::Denied => "Denied",
        };
        writeln!(markdown, "**Decision:** {decision}").expect("writing to a string cannot fail");
    }
    if let Some(duration_ms) = audit.duration_ms {
        writeln!(markdown, "**Native execution:** {duration_ms} ms")
            .expect("writing to a string cannot fail");
    }
    markdown.push('\n');
}

/// Writes deterministic pretty JSON inside a Markdown fence safe for embedded backticks.
fn write_json_fence(markdown: &mut String, value: &serde_json::Value) {
    let json = serde_json::to_string_pretty(value).expect("retained JSON values always serialize");
    let longest_run = json
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat((longest_run + 1).max(3));
    writeln!(markdown, "{fence}json\n{json}\n{fence}\n").expect("writing to a string cannot fail");
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
