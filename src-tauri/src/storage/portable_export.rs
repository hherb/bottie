//! Portable conversation document bundles containing deduplicated retained attachment bytes.

use std::{collections::BTreeMap, fmt::Write as _, fs, io::Write, path::Path};

use serde::Serialize;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::{ConversationStore, StorageError, StoredAttachment, StoredConversation};

const ATTACHMENT_ARCHIVE_DIRECTORY: &str = "attachments";
const ZIP_FILENAME_EXTENSION: &str = "zip";

/// Native-only file payload prepared before Bottie opens a save dialog.
pub(crate) struct ConversationFileExport {
    /// Safe suggested leaf filename that reveals no local directory.
    pub(crate) file_name: String,
    /// Complete UTF-8 document to write after user confirmation.
    pub(crate) contents: String,
    document_file_name: Option<String>,
    attachments: Vec<PortableAttachmentFile>,
}

impl ConversationFileExport {
    /// Creates one plain UTF-8 export that remains unchanged when it has no attachments.
    pub(super) fn document(file_name: String, contents: String) -> Self {
        Self {
            file_name,
            contents,
            document_file_name: None,
            attachments: Vec::new(),
        }
    }

    /// Writes a plain document or a ZIP bundle while mapping paths to one redacted error.
    pub(crate) fn write_to(&self, path: &Path) -> Result<(), StorageError> {
        if self.attachments.is_empty() {
            return fs::write(path, &self.contents).map_err(|_| StorageError::export());
        }
        let parent = path.parent().ok_or_else(StorageError::export)?;
        let staging = parent.join(format!(".bottie-export-{}.tmp", uuid::Uuid::new_v4()));
        let result = self.write_bundle(&staging);
        if result.is_err() {
            let _ = fs::remove_file(&staging);
            return result;
        }
        if path.exists() && fs::remove_file(path).is_err() {
            let _ = fs::remove_file(&staging);
            return Err(StorageError::export());
        }
        fs::rename(&staging, path).map_err(|_| {
            let _ = fs::remove_file(&staging);
            StorageError::export()
        })?;
        Ok(())
    }

    /// Writes a complete archive to one native-only staging path.
    fn write_bundle(&self, path: &Path) -> Result<(), StorageError> {
        let file = fs::File::create(path).map_err(|_| StorageError::export())?;
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        archive
            .start_file(
                self.document_file_name
                    .as_deref()
                    .ok_or_else(StorageError::export)?,
                options,
            )
            .map_err(|_| StorageError::export())?;
        archive
            .write_all(self.contents.as_bytes())
            .map_err(|_| StorageError::export())?;
        for attachment in &self.attachments {
            let bytes = fs::read(&attachment.source_path).map_err(|_| StorageError::export())?;
            if bytes.len() as u64 != attachment.byte_size
                || format!("{:x}", Sha256::digest(&bytes)) != attachment.sha256
            {
                return Err(StorageError::export());
            }
            archive
                .start_file(&attachment.archive_path, options)
                .map_err(|_| StorageError::export())?;
            archive
                .write_all(&bytes)
                .map_err(|_| StorageError::export())?;
        }
        archive.finish().map_err(|_| StorageError::export())?;
        Ok(())
    }

    /// Returns whether the Save dialog should advertise the ZIP bundle format.
    pub(crate) fn is_bundle(&self) -> bool {
        !self.attachments.is_empty()
    }
}

/// Path-free portable attachment metadata shared by JSON and Markdown documents.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PortableAttachmentReference {
    /// Sanitized user-facing leaf name.
    pub(super) display_name: String,
    /// MIME type inferred from retained content.
    pub(super) mime_type: String,
    /// Exact original byte size.
    pub(super) byte_size: u64,
    /// Lowercase SHA-256 identity used for portable integrity checks.
    pub(super) sha256: String,
    /// Safe relative ZIP member containing the original bytes.
    pub(super) file: String,
}

impl ConversationStore {
    /// Upgrades one plain export to a ZIP only when its selected data references retained files.
    pub(super) fn bundle_export(
        &self,
        mut export: ConversationFileExport,
        conversations: &[&StoredConversation],
    ) -> ConversationFileExport {
        let mut attachments = BTreeMap::new();
        for attachment in conversations.iter().flat_map(|conversation| {
            conversation.attachments.iter().chain(
                conversation
                    .messages
                    .iter()
                    .flat_map(|message| message.attachments.iter()),
            )
        }) {
            attachments
                .entry(attachment.sha256.clone())
                .or_insert_with(|| PortableAttachmentFile {
                    archive_path: attachment_archive_path(attachment),
                    source_path: self.attachment_blob_path(&attachment.sha256),
                    byte_size: attachment.byte_size,
                    sha256: attachment.sha256.clone(),
                });
        }
        if attachments.is_empty() {
            return export;
        }
        let document_file_name = export.file_name.clone();
        export.file_name = format!(
            "{}.{}",
            Path::new(&document_file_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("bottie-conversation"),
            ZIP_FILENAME_EXTENSION
        );
        export.document_file_name = Some(document_file_name);
        export.attachments = attachments.into_values().collect();
        export
    }
}

/// Builds the stable path-free metadata rendered into portable documents.
pub(super) fn portable_attachment_reference(
    attachment: &StoredAttachment,
) -> PortableAttachmentReference {
    PortableAttachmentReference {
        display_name: attachment.display_name.clone(),
        mime_type: attachment.mime_type.clone(),
        byte_size: attachment.byte_size,
        sha256: attachment.sha256.clone(),
        file: attachment_archive_path(attachment),
    }
}

/// Writes one portable Markdown attachment section using only archive-relative paths.
pub(super) fn write_attachment_markdown_section(
    markdown: &mut String,
    attachments: &[StoredAttachment],
    heading: &str,
) {
    if attachments.is_empty() {
        return;
    }
    writeln!(markdown, "\n## {heading}\n").expect("writing to a string cannot fail");
    for attachment in attachments {
        let reference = portable_attachment_reference(attachment);
        writeln!(
            markdown,
            "- [{}](<{}>) — `{}`, {} bytes, SHA-256 `{}`",
            escape_markdown_label(&reference.display_name),
            reference.file,
            reference.mime_type,
            reference.byte_size,
            reference.sha256
        )
        .expect("writing to a string cannot fail");
    }
}

/// Native-only source record for one deduplicated ZIP member.
struct PortableAttachmentFile {
    archive_path: String,
    source_path: std::path::PathBuf,
    byte_size: u64,
    sha256: String,
}

/// Produces a safe collision-resistant relative ZIP path from trusted metadata.
fn attachment_archive_path(attachment: &StoredAttachment) -> String {
    format!("{ATTACHMENT_ARCHIVE_DIRECTORY}/{}", attachment.sha256)
}

/// Escapes Markdown punctuation inside a generated link label.
fn escape_markdown_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}
