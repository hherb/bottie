//! Portable conversation document bundles containing deduplicated retained attachment bytes.

use std::{collections::BTreeMap, fmt::Write as _, fs, io::Write, path::Path};

use serde::Serialize;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::{
    ConversationStore, GeneratedAssetExecution, GeneratedAssetStatus, GeneratedImageSourceFormat,
    GeneratedImageSourceType, StorageError, StoredAttachment, StoredConversation,
    StoredGeneratedAsset, StoredGeneratedImageSource,
};

const ATTACHMENT_ARCHIVE_DIRECTORY: &str = "attachments";
const GENERATED_ASSET_ARCHIVE_DIRECTORY: &str = "generated-images";
const EDIT_SOURCE_ARCHIVE_DIRECTORY: &str = "image-edit-sources";
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

/// Path-free generated-image metadata shared by JSON and Markdown documents.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PortableGeneratedAssetReference {
    /// Stable zero-based order within the assistant response.
    pub(super) ordinal: u8,
    /// Durable output state at export time.
    pub(super) status: GeneratedAssetStatus,
    /// Completed MIME type, absent for non-completed output.
    pub(super) media_type: Option<String>,
    /// Completed decoded width, absent for non-completed output.
    pub(super) width: Option<u32>,
    /// Completed decoded height, absent for non-completed output.
    pub(super) height: Option<u32>,
    /// Completed exact byte size, absent for non-completed output.
    pub(super) byte_size: Option<u64>,
    /// Stable Bottie provider identity.
    pub(super) provider_id: String,
    /// Exact provider-owned model identity.
    pub(super) model_id: String,
    /// Explicit local or cloud execution class.
    pub(super) execution: GeneratedAssetExecution,
    /// Deterministic seed when supported by the backend.
    pub(super) seed: Option<i64>,
    /// Stable path-free terminal error category.
    pub(super) error_code: Option<String>,
    /// Native creation time as Unix milliseconds.
    pub(super) created_at_ms: i64,
    /// Portable content identity, present only for a completed output.
    pub(super) sha256: Option<String>,
    /// Safe relative ZIP member, present only for a completed output.
    pub(super) file: Option<String>,
    /// Ordered exact edit-source snapshots without native database identities.
    pub(super) sources: Vec<PortableGeneratedImageSourceReference>,
}

/// Portable exact-byte reference for one ordered generated-image editing source.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PortableGeneratedImageSourceReference {
    /// Stable zero-based order supplied to the hosted model.
    pub(super) ordinal: u8,
    /// Whether the source originated as an attachment or generated image.
    pub(super) source_type: GeneratedImageSourceType,
    /// Exact normalized source MIME type.
    pub(super) media_type: String,
    /// Exact decoded source width.
    pub(super) width: u32,
    /// Exact decoded source height.
    pub(super) height: u32,
    /// Exact encoded source byte count.
    pub(super) byte_size: u64,
    /// Portable exact-byte content identity.
    pub(super) sha256: String,
    /// Safe relative ZIP member containing the exact provider source bytes.
    pub(super) file: String,
}

impl ConversationStore {
    /// Upgrades one plain export to a ZIP only when its selected data references retained files.
    pub(super) fn bundle_export(
        &self,
        mut export: ConversationFileExport,
        conversations: &[&StoredConversation],
    ) -> Result<ConversationFileExport, StorageError> {
        let mut attachments = BTreeMap::new();
        for attachment in conversations.iter().flat_map(|conversation| {
            conversation.attachments.iter().chain(
                conversation
                    .messages
                    .iter()
                    .flat_map(|message| message.attachments.iter()),
            )
        }) {
            let archive_path = attachment_archive_path(attachment);
            attachments
                .entry(archive_path.clone())
                .or_insert_with(|| PortableAttachmentFile {
                    archive_path,
                    source_path: self.attachment_blob_path(&attachment.sha256),
                    byte_size: attachment.byte_size,
                    sha256: attachment.sha256.clone(),
                });
        }
        for asset in conversations
            .iter()
            .flat_map(|conversation| conversation.messages.iter())
            .flat_map(|message| message.generated_assets.iter())
        {
            for source in &asset.sources {
                let archive_path = generated_edit_source_archive_path(source);
                let source_path = match source.source_type {
                    GeneratedImageSourceType::Attachment => self.normalized_image_path(
                        &source.sha256,
                        generated_source_format(source)?.normalized(),
                    )?,
                    GeneratedImageSourceType::GeneratedAsset => {
                        self.generated_asset_blob_path(&source.sha256)?
                    }
                };
                attachments
                    .entry(archive_path.clone())
                    .or_insert(PortableAttachmentFile {
                        archive_path,
                        source_path,
                        byte_size: source.byte_size,
                        sha256: source.sha256.clone(),
                    });
            }
            let (sha256, byte_size) = match (asset.status, asset.sha256.as_ref(), asset.byte_size) {
                (GeneratedAssetStatus::Completed, Some(sha256), Some(byte_size))
                    if is_sha256(sha256) =>
                {
                    (sha256, byte_size)
                }
                (GeneratedAssetStatus::Completed, _, _) => return Err(StorageError::export()),
                _ => continue,
            };
            let archive_path = generated_asset_archive_path(sha256);
            let source_path = self
                .generated_asset_blob_path(sha256)
                .map_err(|_| StorageError::export())?;
            attachments
                .entry(archive_path.clone())
                .or_insert(PortableAttachmentFile {
                    archive_path,
                    source_path,
                    byte_size,
                    sha256: sha256.clone(),
                });
        }
        if attachments.is_empty() {
            return Ok(export);
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
        Ok(export)
    }
}

/// Builds stable generated-image metadata without exposing opaque database identities.
pub(super) fn portable_generated_asset_reference(
    asset: &StoredGeneratedAsset,
) -> PortableGeneratedAssetReference {
    let sha256 = asset.sha256.as_deref().filter(|value| is_sha256(value));
    PortableGeneratedAssetReference {
        ordinal: asset.ordinal,
        status: asset.status,
        media_type: asset.media_type.clone(),
        width: asset.width,
        height: asset.height,
        byte_size: asset.byte_size,
        provider_id: asset.provider_id.clone(),
        model_id: asset.model_id.clone(),
        execution: asset.execution,
        seed: asset.seed,
        error_code: asset.error_code.clone(),
        created_at_ms: asset.created_at_ms,
        sha256: sha256.map(str::to_owned),
        file: sha256.map(generated_asset_archive_path),
        sources: asset
            .sources
            .iter()
            .map(portable_generated_image_source_reference)
            .collect(),
    }
}

/// Builds one exact portable source snapshot without its opaque native identifier.
fn portable_generated_image_source_reference(
    source: &StoredGeneratedImageSource,
) -> PortableGeneratedImageSourceReference {
    PortableGeneratedImageSourceReference {
        ordinal: source.ordinal,
        source_type: source.source_type,
        media_type: source.media_type.clone(),
        width: source.width,
        height: source.height,
        byte_size: source.byte_size,
        sha256: source.sha256.clone(),
        file: generated_edit_source_archive_path(source),
    }
}

/// Writes completed and terminal generated-image metadata using archive-relative links only.
pub(super) fn write_generated_asset_markdown_section(
    markdown: &mut String,
    assets: &[StoredGeneratedAsset],
) {
    if assets.is_empty() {
        return;
    }
    markdown.push_str("### Generated images\n\n");
    for asset in assets {
        let reference = portable_generated_asset_reference(asset);
        if let (Some(file), Some(sha256), Some(byte_size), Some(width), Some(height)) = (
            reference.file.as_deref(),
            reference.sha256.as_deref(),
            reference.byte_size,
            reference.width,
            reference.height,
        ) {
            writeln!(
                markdown,
                "- [Generated image {}](<{}>) — `{}`, {}×{}, {} bytes, SHA-256 `{}`",
                u16::from(reference.ordinal) + 1,
                file,
                reference.media_type.as_deref().unwrap_or("image/png"),
                width,
                height,
                byte_size,
                sha256,
            )
            .expect("writing to a string cannot fail");
        } else {
            writeln!(
                markdown,
                "- Generated image {} — {}",
                u16::from(reference.ordinal) + 1,
                generated_asset_status_label(reference.status),
            )
            .expect("writing to a string cannot fail");
        }
        for source in &reference.sources {
            writeln!(
                markdown,
                "  - [Edit source {}](<{}>) — `{}`, {}×{}, {} bytes, SHA-256 `{}`",
                u16::from(source.ordinal) + 1,
                source.file,
                source.media_type,
                source.width,
                source.height,
                source.byte_size,
                source.sha256,
            )
            .expect("writing to a string cannot fail");
        }
    }
    markdown.push('\n');
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

/// Produces a collision-resistant generated PNG member from its verified content identity.
fn generated_asset_archive_path(sha256: &str) -> String {
    format!("{GENERATED_ASSET_ARCHIVE_DIRECTORY}/{sha256}.png")
}

/// Produces a collision-resistant portable member for exact normalized editing bytes.
fn generated_edit_source_archive_path(source: &StoredGeneratedImageSource) -> String {
    match source.source_type {
        GeneratedImageSourceType::GeneratedAsset => generated_asset_archive_path(&source.sha256),
        GeneratedImageSourceType::Attachment => {
            let extension = if source.media_type == "image/jpeg" {
                "jpeg"
            } else {
                "png"
            };
            format!(
                "{EDIT_SOURCE_ARCHIVE_DIRECTORY}/{}.{}",
                source.sha256, extension
            )
        }
    }
}

/// Validates the closed source MIME contract before resolving a native derivative path.
fn generated_source_format(
    source: &StoredGeneratedImageSource,
) -> Result<GeneratedImageSourceFormat, StorageError> {
    match source.media_type.as_str() {
        "image/jpeg" => Ok(GeneratedImageSourceFormat::Jpeg),
        "image/png" => Ok(GeneratedImageSourceFormat::Png),
        _ => Err(StorageError::export()),
    }
}

/// Returns a stable human-readable generated output state.
fn generated_asset_status_label(status: GeneratedAssetStatus) -> &'static str {
    match status {
        GeneratedAssetStatus::Pending => "Pending",
        GeneratedAssetStatus::Completed => "Completed bytes unavailable",
        GeneratedAssetStatus::Cancelled => "Cancelled",
        GeneratedAssetStatus::Failed => "Failed",
    }
}

/// Accepts only the lowercase content identities Bottie itself creates.
fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Escapes Markdown punctuation inside a generated link label.
fn escape_markdown_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}
