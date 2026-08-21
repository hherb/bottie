//! Bounded native text extraction for retained attachment blobs.

use std::{
    fs::File,
    io::{Read, Take},
    path::Path,
};

use lopdf::{DecompressError, Document, Error as PdfError, LoadOptions};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::{
    ConversationStore, StorageError,
    docx::{self, DOCX_MIME_TYPE},
    now_ms,
};

const BYTES_PER_MEBIBYTE: usize = 1024 * 1024;
const MAX_EXTRACTED_TEXT_MEBIBYTES: usize = 2;
const EXTRACTION_READ_LIMIT: u64 = (MAX_EXTRACTED_TEXT_BYTES + 1) as u64;
const MAX_PDF_DECOMPRESSED_PAGE_MEBIBYTES: usize = 8;
const MAX_PDF_DECOMPRESSED_PAGE_BYTES: usize =
    MAX_PDF_DECOMPRESSED_PAGE_MEBIBYTES * BYTES_PER_MEBIBYTE;
pub(super) const ERROR_CONTENT_TOO_LARGE: &str = "content_too_large";
const ERROR_INVALID_UTF8: &str = "invalid_utf8";
const ERROR_MISSING_CONTENT: &str = "missing_content";
const ERROR_PDF_ENCRYPTED: &str = "pdf_encrypted";
const ERROR_PDF_EXTRACTION_FAILED: &str = "pdf_extraction_failed";
const ERROR_PDF_INVALID: &str = "pdf_invalid";
const ERROR_PDF_NO_TEXT: &str = "pdf_no_text";
const ERROR_PDF_PAGE_LIMIT_EXCEEDED: &str = "pdf_page_limit_exceeded";
const ERROR_READ_FAILED: &str = "read_failed";

/// Maximum retained UTF-8 bytes accepted into SQLite for one attachment.
pub(crate) const MAX_EXTRACTED_TEXT_BYTES: usize =
    MAX_EXTRACTED_TEXT_MEBIBYTES * BYTES_PER_MEBIBYTE;
/// Maximum PDF page count accepted by the synchronous extraction slice.
pub(crate) const MAX_PDF_PAGES: usize = 500;

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
    /// Text derived from a bounded page-aware PDF parse.
    Pdf,
    /// Text derived from a bounded DOCX package and WordprocessingML parse.
    Docx,
}

impl AttachmentExtractionFormat {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::PlainText => "plain_text",
            Self::Markdown => "markdown",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
        }
    }

    /// Parses a trusted format constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "plain_text" => Ok(Self::PlainText),
            "markdown" => Ok(Self::Markdown),
            "pdf" => Ok(Self::Pdf),
            "docx" => Ok(Self::Docx),
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
    /// PDF page count for ready PDF text, absent for every other format and state.
    pub(crate) page_count: Option<u64>,
    /// Stable path-free failure category for failed extraction.
    pub(crate) error_code: Option<String>,
}

impl ConversationStore {
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
        if mime_type == "application/pdf" {
            return self.process_pdf_extraction(attachment_id, &sha256);
        }
        if mime_type == DOCX_MIME_TYPE
            || (mime_type == "application/zip"
                && (display_name.to_ascii_lowercase().ends_with(".docx")
                    || docx::is_docx_package(&self.attachment_blob_path(&sha256))))
        {
            return self.process_docx_extraction(attachment_id, &sha256);
        }
        if mime_type != "text/plain" {
            return self.persist_extraction(
                attachment_id,
                AttachmentExtractionState::Unsupported,
                None,
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
            None,
        )
    }

    /// Extracts bounded DOCX text or records one stable path-free failure category.
    fn process_docx_extraction(
        &self,
        attachment_id: &str,
        sha256: &str,
    ) -> Result<(), StorageError> {
        let blob_path = self.attachment_blob_path(sha256);
        if !blob_path.is_file() {
            return self.persist_extraction_failure(attachment_id, ERROR_MISSING_CONTENT);
        }
        match docx::extract_docx(&blob_path) {
            Ok(extracted) => {
                let mut connection = self.open()?;
                let transaction = connection
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                transaction.execute(
                    "UPDATE attachments SET mime_type = ?1 WHERE id = ?2",
                    params![DOCX_MIME_TYPE, attachment_id],
                )?;
                transaction.execute(
                    "UPDATE attachment_extractions
                     SET state = 'ready', format = 'docx', text_content = ?1,
                         character_count = ?2, page_count = NULL, error_code = NULL,
                         updated_at_ms = ?3
                     WHERE attachment_id = ?4 AND state = 'pending'",
                    params![
                        extracted.text,
                        extracted.character_count as i64,
                        now_ms()?,
                        attachment_id
                    ],
                )?;
                transaction.commit()?;
                Ok(())
            }
            Err(error_code) => self.persist_extraction_failure(attachment_id, error_code),
        }
    }

    /// Extracts bounded PDF text or records one stable path-free failure category.
    fn process_pdf_extraction(
        &self,
        attachment_id: &str,
        sha256: &str,
    ) -> Result<(), StorageError> {
        let blob_path = self.attachment_blob_path(sha256);
        if !blob_path.is_file() {
            return self.persist_extraction_failure(attachment_id, ERROR_MISSING_CONTENT);
        }
        match extract_pdf(&blob_path) {
            Ok(extracted) => self.persist_extraction(
                attachment_id,
                AttachmentExtractionState::Ready,
                Some(AttachmentExtractionFormat::Pdf),
                Some(extracted.text),
                Some(extracted.character_count),
                Some(extracted.page_count),
            ),
            Err(error_code) => self.persist_extraction_failure(attachment_id, error_code),
        }
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
                 character_count = NULL, page_count = NULL, error_code = ?1, updated_at_ms = ?2
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
        page_count: Option<u64>,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE attachment_extractions
             SET state = ?1, format = ?2, text_content = ?3, character_count = ?4,
                 page_count = ?5, error_code = NULL, updated_at_ms = ?6
             WHERE attachment_id = ?7 AND state = 'pending'",
            params![
                state.as_str(),
                format.map(AttachmentExtractionFormat::as_str),
                text,
                character_count.map(|count| count as i64),
                page_count.map(|count| count as i64),
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

/// Successfully extracted page-aware PDF content retained only in native storage.
struct ExtractedPdf {
    text: String,
    character_count: u64,
    page_count: u64,
}

/// Extracts text from an untrusted retained PDF within page, stream, and output ceilings.
fn extract_pdf(path: &Path) -> Result<ExtractedPdf, &'static str> {
    let options = LoadOptions::with_max_decompressed_size(MAX_PDF_DECOMPRESSED_PAGE_BYTES);
    let document = Document::load_with_options(path, options).map_err(pdf_load_error_code)?;
    if document.was_encrypted() {
        return Err(ERROR_PDF_ENCRYPTED);
    }
    let page_numbers = document.get_pages().into_keys().collect::<Vec<_>>();
    if page_numbers.len() > MAX_PDF_PAGES {
        return Err(ERROR_PDF_PAGE_LIMIT_EXCEEDED);
    }
    let mut text = String::new();
    for page_number in &page_numbers {
        let page_text = document
            .extract_text_with_limit(&[*page_number], MAX_PDF_DECOMPRESSED_PAGE_BYTES)
            .map_err(pdf_extraction_error_code)?;
        let page_text = page_text.trim();
        if page_text.is_empty() {
            continue;
        }
        let separator_bytes = usize::from(!text.is_empty()) * 2;
        if text.len() + separator_bytes + page_text.len() > MAX_EXTRACTED_TEXT_BYTES {
            return Err(ERROR_CONTENT_TOO_LARGE);
        }
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(page_text);
    }
    if text.is_empty() {
        return Err(ERROR_PDF_NO_TEXT);
    }
    Ok(ExtractedPdf {
        character_count: text.chars().count() as u64,
        page_count: page_numbers.len() as u64,
        text,
    })
}

/// Maps PDF parser failures into stable categories that never expose paths or parser details.
fn pdf_load_error_code(error: PdfError) -> &'static str {
    match error {
        PdfError::InvalidPassword
        | PdfError::Decryption(_)
        | PdfError::UnsupportedSecurityHandler(_) => ERROR_PDF_ENCRYPTED,
        PdfError::IO(_) => ERROR_READ_FAILED,
        PdfError::Decompress(DecompressError::MemoryLimitExceeded { .. }) => {
            ERROR_CONTENT_TOO_LARGE
        }
        _ => ERROR_PDF_INVALID,
    }
}

/// Maps page text decoding failures without revealing untrusted document details.
fn pdf_extraction_error_code(error: PdfError) -> &'static str {
    match error {
        PdfError::Decompress(DecompressError::MemoryLimitExceeded { .. }) => {
            ERROR_CONTENT_TOO_LARGE
        }
        _ => ERROR_PDF_EXTRACTION_FAILED,
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
