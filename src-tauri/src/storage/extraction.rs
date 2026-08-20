//! Bounded native text extraction for retained attachment blobs.

use std::{
    fs::File,
    io::{Read, Take},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::{ConversationStore, StorageError, now_ms};

const BYTES_PER_MEBIBYTE: usize = 1024 * 1024;
const MAX_EXTRACTED_TEXT_MEBIBYTES: usize = 2;
const EXTRACTION_READ_LIMIT: u64 = (MAX_EXTRACTED_TEXT_BYTES + 1) as u64;
const ERROR_CONTENT_TOO_LARGE: &str = "content_too_large";
const ERROR_INVALID_UTF8: &str = "invalid_utf8";
const ERROR_MISSING_CONTENT: &str = "missing_content";
const ERROR_READ_FAILED: &str = "read_failed";

/// Maximum retained UTF-8 bytes accepted into SQLite for one attachment.
pub(crate) const MAX_EXTRACTED_TEXT_BYTES: usize =
    MAX_EXTRACTED_TEXT_MEBIBYTES * BYTES_PER_MEBIBYTE;

/// Current durable state of native attachment text extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttachmentExtractionState {
    /// Native extraction has not yet reached a terminal state.
    Pending,
    /// Bounded UTF-8 text is available inside the native store.
    Ready,
    /// This slice does not support the retained content type.
    Unsupported,
    /// Supported-looking content could not be extracted within policy.
    Failed,
}

impl AttachmentExtractionState {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        }
    }

    /// Parses a trusted state constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "pending" => Ok(Self::Pending),
            "ready" => Ok(Self::Ready),
            "unsupported" => Ok(Self::Unsupported),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Native text representation retained for one successfully extracted attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttachmentExtractionFormat {
    /// Plain UTF-8 source text.
    PlainText,
    /// Markdown source retained without rendering or HTML interpretation.
    Markdown,
}

impl AttachmentExtractionFormat {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::PlainText => "plain_text",
            Self::Markdown => "markdown",
        }
    }

    /// Parses a trusted format constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "plain_text" => Ok(Self::PlainText),
            "markdown" => Ok(Self::Markdown),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Path-free extraction metadata safe to expose without extracted content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredAttachmentExtraction {
    /// Current native extraction state.
    pub(crate) state: AttachmentExtractionState,
    /// Ready text representation, absent for every other state.
    pub(crate) format: Option<AttachmentExtractionFormat>,
    /// Unicode scalar count for ready text without exposing the text itself.
    pub(crate) character_count: Option<u64>,
    /// Stable path-free failure category for failed extraction.
    pub(crate) error_code: Option<String>,
}

impl ConversationStore {
    /// Completes every extraction left pending by migration or an interrupted process.
    pub(super) fn process_pending_attachment_extractions(&self) -> Result<(), StorageError> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT attachment_id FROM attachment_extractions
             WHERE state = 'pending' ORDER BY updated_at_ms, attachment_id",
        )?;
        let attachment_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        for attachment_id in attachment_ids {
            self.process_attachment_extraction(&attachment_id)?;
        }
        Ok(())
    }

    /// Resolves one pending attachment into ready, unsupported, or failed native state.
    pub(super) fn process_attachment_extraction(
        &self,
        attachment_id: &str,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        let attachment = connection
            .query_row(
                "SELECT attachments.display_name, attachments.mime_type, attachments.sha256,
                        attachment_extractions.state
                 FROM attachments
                 JOIN attachment_extractions
                   ON attachment_extractions.attachment_id = attachments.id
                 WHERE attachments.id = ?1",
                [attachment_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        drop(connection);
        let Some((display_name, mime_type, sha256, state)) = attachment else {
            return Err(StorageError::internal());
        };
        if state != AttachmentExtractionState::Pending.as_str() {
            return Ok(());
        }
        if mime_type != "text/plain" {
            return self.persist_extraction(
                attachment_id,
                AttachmentExtractionState::Unsupported,
                None,
                None,
                None,
            );
        }

        let blob_path = self.attachment_blob_path(&sha256);
        let file = match File::open(blob_path) {
            Ok(file) => file,
            Err(_) => return self.persist_extraction_failure(attachment_id, ERROR_MISSING_CONTENT),
        };
        let bytes = match read_bounded(file.take(EXTRACTION_READ_LIMIT)) {
            Ok(bytes) => bytes,
            Err(_) => return self.persist_extraction_failure(attachment_id, ERROR_READ_FAILED),
        };
        if bytes.len() > MAX_EXTRACTED_TEXT_BYTES {
            return self.persist_extraction_failure(attachment_id, ERROR_CONTENT_TOO_LARGE);
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned(),
            Err(_) => return self.persist_extraction_failure(attachment_id, ERROR_INVALID_UTF8),
        };
        let format = extraction_format(&display_name);
        let character_count = text.chars().count() as u64;
        self.persist_extraction(
            attachment_id,
            AttachmentExtractionState::Ready,
            Some(format),
            Some(text),
            Some(character_count),
        )
    }

    /// Persists one path-free failed state without retaining partial content.
    fn persist_extraction_failure(
        &self,
        attachment_id: &str,
        error_code: &str,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE attachment_extractions
             SET state = 'failed', format = NULL, text_content = NULL,
                 character_count = NULL, error_code = ?1, updated_at_ms = ?2
             WHERE attachment_id = ?3 AND state = 'pending'",
            params![error_code, now_ms()?, attachment_id],
        )?;
        Ok(())
    }

    /// Persists one non-failed extraction transition under the schema invariant.
    fn persist_extraction(
        &self,
        attachment_id: &str,
        state: AttachmentExtractionState,
        format: Option<AttachmentExtractionFormat>,
        text: Option<String>,
        character_count: Option<u64>,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE attachment_extractions
             SET state = ?1, format = ?2, text_content = ?3, character_count = ?4,
                 error_code = NULL, updated_at_ms = ?5
             WHERE attachment_id = ?6 AND state = 'pending'",
            params![
                state.as_str(),
                format.map(AttachmentExtractionFormat::as_str),
                text,
                character_count.map(|count| count as i64),
                now_ms()?,
                attachment_id
            ],
        )?;
        Ok(())
    }

    /// Loads native-only extracted content for storage contract tests.
    #[cfg(test)]
    pub(super) fn extracted_text_for_test(
        &self,
        attachment_id: &str,
    ) -> Result<Option<String>, StorageError> {
        self.open()?
            .query_row(
                "SELECT text_content FROM attachment_extractions WHERE attachment_id = ?1",
                [attachment_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}

/// Classifies safe Markdown leaf extensions while treating other UTF-8 input as plain text.
pub(crate) fn extraction_format(display_name: &str) -> AttachmentExtractionFormat {
    let extension = display_name
        .rsplit_once('.')
        .map(|(_, extension)| extension);
    match extension.map(str::to_ascii_lowercase).as_deref() {
        Some("md" | "markdown") => AttachmentExtractionFormat::Markdown,
        _ => AttachmentExtractionFormat::PlainText,
    }
}

/// Reads at most the policy ceiling plus one byte so oversize content stays bounded.
fn read_bounded(mut source: Take<File>) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    source.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{AttachmentExtractionFormat, extraction_format};

    #[test]
    fn classifies_markdown_from_safe_leaf_extensions_only() {
        assert_eq!(
            extraction_format("notes.md"),
            AttachmentExtractionFormat::Markdown
        );
        assert_eq!(
            extraction_format("GUIDE.Markdown"),
            AttachmentExtractionFormat::Markdown
        );
        assert_eq!(
            extraction_format("notes.md.txt"),
            AttachmentExtractionFormat::PlainText
        );
    }
}
