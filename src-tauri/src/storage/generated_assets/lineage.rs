//! Exact ordered native source snapshots for hosted generated-image editing.

#![cfg_attr(not(test), allow(dead_code))]

mod retry;
mod validation;

use std::fs;

use rusqlite::{Connection, Transaction, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GeneratedImageProvenance, GeneratedImageRequestOptions, insert_generated_image_message,
    load_generated_message, selected_branch_without_active_generation, selected_image_prompt,
    validate_output_count,
};
use crate::storage::{ConversationStore, StorageError, image_normalization::NormalizedImageFormat};
#[cfg(test)]
use validation::validate_reference;
use validation::{
    load_source_metadata, source_limit_error, validate_edit_provenance, validate_source_references,
};

const MIN_EDIT_SOURCE_COUNT: usize = 1;
const MAX_EDIT_SOURCE_COUNT: usize = 3;
const BYTES_PER_MEBIBYTE: u64 = 1_024 * 1_024;
const MAX_EDIT_SOURCE_MEBIBYTES: u64 = 10;
const MAX_EDIT_SOURCE_BYTES: u64 = MAX_EDIT_SOURCE_MEBIBYTES * BYTES_PER_MEBIBYTE;
const MAX_TOTAL_EDIT_SOURCE_BYTES: u64 = 3 * MAX_EDIT_SOURCE_BYTES;
const HOSTED_EDIT_PROVIDER_ID: &str = "qwen-image";
const HOSTED_EDIT_MODEL_ID: &str = "qwen-image-2.0-2026-03-03";

/// Preflights path-free source count, identities, and duplicates before privileged lookup.
pub(crate) fn validate_generated_image_source_references(
    references: &[GeneratedImageSourceReference],
) -> Result<(), StorageError> {
    validate_source_references(references)
}

/// Native opaque identity selected as one ordered image-editing source.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) enum GeneratedImageSourceReference {
    /// A normalized retained attachment associated with the exact current user request.
    Attachment(String),
    /// A completed generated image on the exact current request ancestry.
    GeneratedAsset(String),
}

impl GeneratedImageSourceReference {
    /// Returns the stable path-free source category.
    fn source_type(&self) -> GeneratedImageSourceType {
        match self {
            Self::Attachment(_) => GeneratedImageSourceType::Attachment,
            Self::GeneratedAsset(_) => GeneratedImageSourceType::GeneratedAsset,
        }
    }

    /// Returns the opaque native identifier without interpreting it as a path.
    fn id(&self) -> &str {
        match self {
            Self::Attachment(id) | Self::GeneratedAsset(id) => id,
        }
    }
}

/// Stable category of one generated-image edit source.
#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GeneratedImageSourceType {
    /// A normalized user-retained attachment.
    Attachment,
    /// An earlier completed assistant-generated image.
    GeneratedAsset,
}

impl GeneratedImageSourceType {
    /// Returns the SQLite representation used by the closed schema.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Attachment => "attachment",
            Self::GeneratedAsset => "generated_asset",
        }
    }

    /// Parses a trusted source category constrained by SQLite.
    fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "attachment" => Ok(Self::Attachment),
            "generated_asset" => Ok(Self::GeneratedAsset),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Encoding of exact native bytes retained for one edit source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GeneratedImageSourceFormat {
    /// Metadata-free JPEG attachment derivative.
    Jpeg,
    /// Metadata-free PNG attachment derivative or generated image.
    Png,
}

impl GeneratedImageSourceFormat {
    /// Returns the stable MIME type retained in lineage and provider-neutral requests.
    pub(crate) fn media_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }

    /// Maps an attachment derivative format into the editing contract.
    fn from_normalized(format: NormalizedImageFormat) -> Self {
        match format {
            NormalizedImageFormat::Jpeg => Self::Jpeg,
            NormalizedImageFormat::Png => Self::Png,
        }
    }

    /// Maps the editing contract back to an attachment derivative path extension.
    pub(crate) fn normalized(self) -> NormalizedImageFormat {
        match self {
            Self::Jpeg => NormalizedImageFormat::Jpeg,
            Self::Png => NormalizedImageFormat::Png,
        }
    }
}

/// Path-free durable snapshot of one exact image-editing source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredGeneratedImageSource {
    /// Stable zero-based source order supplied to the model.
    pub(crate) ordinal: u8,
    /// Whether the source came from a retained attachment or earlier generated output.
    pub(crate) source_type: GeneratedImageSourceType,
    /// Opaque source identity used by future native preview selection.
    pub(crate) source_id: String,
    /// Exact normalized source MIME type.
    pub(crate) media_type: String,
    /// Exact decoded source width.
    pub(crate) width: u32,
    /// Exact decoded source height.
    pub(crate) height: u32,
    /// Exact encoded source byte count.
    pub(crate) byte_size: u64,
    /// Native-only content identity omitted from IPC serialization.
    #[serde(skip_serializing)]
    pub(crate) sha256: String,
}

/// Exact bounded source bytes ready for a provider-specific native serializer.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ValidatedGeneratedImageSource {
    /// Durable opaque source identity and category.
    reference: GeneratedImageSourceReference,
    /// Exact normalized encoding.
    format: GeneratedImageSourceFormat,
    /// Exact decoded width.
    width: u32,
    /// Exact decoded height.
    height: u32,
    /// Native-only validated encoded bytes.
    bytes: Vec<u8>,
    sha256: String,
}

impl std::fmt::Debug for ValidatedGeneratedImageSource {
    /// Formats only bounded path-free metadata, never source bytes or content identity.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedGeneratedImageSource")
            .field("source_type", &self.reference.source_type())
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("byte_size", &self.bytes.len())
            .finish()
    }
}

impl ValidatedGeneratedImageSource {
    /// Builds a validated source fixture without exposing a product constructor.
    #[cfg(test)]
    pub(crate) fn for_test(
        reference: GeneratedImageSourceReference,
        format: GeneratedImageSourceFormat,
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    ) -> Result<Self, StorageError> {
        validate_reference(&reference)?;
        if width == 0
            || height == 0
            || bytes.is_empty()
            || bytes.len() as u64 > MAX_EDIT_SOURCE_BYTES
        {
            return Err(StorageError::invalid(
                "The image editing source is invalid.",
            ));
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        Ok(Self {
            reference,
            format,
            width,
            height,
            bytes,
            sha256,
        })
    }

    /// Returns the exact native-only source bytes.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the exact native source encoding.
    pub(crate) fn format(&self) -> GeneratedImageSourceFormat {
        self.format
    }

    /// Returns the exact decoded source dimensions.
    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Accepted pending edit message plus exact native source bytes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StartedGeneratedImageEdit {
    /// Newly inserted pending assistant message with ordered source metadata.
    pub(crate) message: crate::storage::StoredMessage,
    /// Exact validated output count.
    pub(crate) output_count: u8,
    /// Exact durable user edit instruction.
    pub(crate) prompt: String,
    /// Accepted output dimensions and prompt-extension policy.
    pub(crate) options: GeneratedImageRequestOptions,
    /// Exact hosted provider provenance.
    pub(crate) provenance: GeneratedImageProvenance,
    /// Ordered validated source bytes retained only in Rust memory.
    pub(crate) sources: Vec<ValidatedGeneratedImageSource>,
}

/// Database-backed source metadata resolved before exact native bytes are reopened.
#[derive(Clone)]
struct SourceMetadata {
    reference: GeneratedImageSourceReference,
    format: GeneratedImageSourceFormat,
    width: u32,
    height: u32,
    byte_size: u64,
    sha256: String,
}

impl ConversationStore {
    /// Starts one atomic hosted edit from one-to-three exact selected-lineage image sources.
    pub(crate) fn start_generated_image_edit_message(
        &self,
        conversation_id: &str,
        request_message_id: &str,
        expected_prompt: &str,
        output_count: u8,
        provenance: &GeneratedImageProvenance,
        options: &GeneratedImageRequestOptions,
        source_references: &[GeneratedImageSourceReference],
    ) -> Result<StartedGeneratedImageEdit, StorageError> {
        validate_output_count(output_count)?;
        validate_source_references(source_references)?;
        validate_edit_provenance(provenance)?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let branch_id = selected_branch_without_active_generation(&transaction, conversation_id)?;
        let prompt = selected_image_prompt(
            &transaction,
            conversation_id,
            request_message_id,
            &branch_id,
        )?;
        if prompt != expected_prompt {
            return Err(StorageError::invalid(
                "The image prompt did not match the durable selected request.",
            ));
        }
        let metadata = load_source_metadata(
            &transaction,
            conversation_id,
            request_message_id,
            source_references,
        )?;
        let sources = self.load_exact_source_bytes(metadata)?;
        let message_id = insert_generated_image_message(
            &transaction,
            conversation_id,
            &branch_id,
            request_message_id,
            output_count,
            provenance,
            options,
        )?;
        insert_source_rows(&transaction, &message_id, &sources)?;
        let message = load_generated_message(&transaction, conversation_id, &message_id)?;
        transaction.commit()?;
        Ok(StartedGeneratedImageEdit {
            message,
            output_count,
            prompt,
            options: *options,
            provenance: provenance.clone(),
            sources,
        })
    }

    /// Revalidates exact snapshot bytes before returning them to native editing orchestration.
    fn load_exact_source_bytes(
        &self,
        metadata: Vec<SourceMetadata>,
    ) -> Result<Vec<ValidatedGeneratedImageSource>, StorageError> {
        let total = metadata.iter().try_fold(0_u64, |total, source| {
            total
                .checked_add(source.byte_size)
                .ok_or_else(source_limit_error)
        })?;
        if metadata
            .iter()
            .any(|source| source.byte_size > MAX_EDIT_SOURCE_BYTES)
            || total > MAX_TOTAL_EDIT_SOURCE_BYTES
        {
            return Err(source_limit_error());
        }
        metadata
            .into_iter()
            .map(|source| {
                let path = match source.reference.source_type() {
                    GeneratedImageSourceType::Attachment => {
                        self.normalized_image_path(&source.sha256, source.format.normalized())?
                    }
                    GeneratedImageSourceType::GeneratedAsset => {
                        self.generated_asset_blob_path(&source.sha256)?
                    }
                };
                let bytes = fs::read(path).map_err(|_| StorageError::generated_image())?;
                if bytes.len() as u64 != source.byte_size
                    || format!("{:x}", Sha256::digest(&bytes)) != source.sha256
                {
                    return Err(StorageError::generated_image());
                }
                Ok(ValidatedGeneratedImageSource {
                    reference: source.reference,
                    format: source.format,
                    width: source.width,
                    height: source.height,
                    bytes,
                    sha256: source.sha256,
                })
            })
            .collect()
    }
}

/// Loads ordered path-free source snapshots for one generated output.
pub(super) fn load_generated_asset_sources(
    connection: &Connection,
    asset_id: &str,
) -> Result<Vec<StoredGeneratedImageSource>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT ordinal, source_type, COALESCE(attachment_id, source_generated_asset_id),
                media_type, width, height, byte_size, sha256
         FROM generated_asset_sources WHERE generated_asset_id = ?1 ORDER BY ordinal",
    )?;
    statement
        .query_map([asset_id], |row| {
            Ok((
                row.get::<_, u8>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            Ok(StoredGeneratedImageSource {
                ordinal: row.0,
                source_type: GeneratedImageSourceType::from_database(&row.1)?,
                source_id: row.2,
                media_type: row.3,
                width: row.4,
                height: row.5,
                byte_size: u64::try_from(row.6).map_err(|_| StorageError::internal())?,
                sha256: row.7,
            })
        })
        .collect()
}

/// Copies every output-specific source snapshot to corresponding outputs during exact retry.
pub(super) fn clone_source_rows(
    transaction: &Transaction<'_>,
    source_message_id: &str,
    destination_message_id: &str,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO generated_asset_sources
         (generated_asset_id, ordinal, source_type, attachment_id, source_generated_asset_id,
          sha256, media_type, width, height, byte_size)
         SELECT destination.id, sources.ordinal, sources.source_type, sources.attachment_id,
                sources.source_generated_asset_id, sources.sha256, sources.media_type,
                sources.width, sources.height, sources.byte_size
         FROM generated_assets AS source
         JOIN generated_asset_sources AS sources ON sources.generated_asset_id = source.id
         JOIN generated_assets AS destination
           ON destination.message_id = ?2 AND destination.ordinal = source.ordinal
         WHERE source.message_id = ?1",
        params![source_message_id, destination_message_id],
    )?;
    Ok(())
}

/// Inserts the same ordered edit-source contract beneath each requested output.
fn insert_source_rows(
    transaction: &Transaction<'_>,
    message_id: &str,
    sources: &[ValidatedGeneratedImageSource],
) -> Result<(), StorageError> {
    let mut statement = transaction
        .prepare("SELECT id FROM generated_assets WHERE message_id = ?1 ORDER BY ordinal")?;
    let asset_ids = statement
        .query_map([message_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for asset_id in asset_ids {
        for (ordinal, source) in sources.iter().enumerate() {
            let (attachment_id, generated_asset_id) = match &source.reference {
                GeneratedImageSourceReference::Attachment(id) => (Some(id), None),
                GeneratedImageSourceReference::GeneratedAsset(id) => (None, Some(id)),
            };
            transaction.execute(
                "INSERT INTO generated_asset_sources
                 (generated_asset_id, ordinal, source_type, attachment_id,
                  source_generated_asset_id, sha256, media_type, width, height, byte_size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    asset_id,
                    i64::try_from(ordinal).map_err(|_| StorageError::internal())?,
                    source.reference.source_type().as_str(),
                    attachment_id,
                    generated_asset_id,
                    source.sha256,
                    source.format.media_type(),
                    source.width,
                    source.height,
                    i64::try_from(source.bytes.len()).map_err(|_| StorageError::internal())?,
                ],
            )?;
        }
    }
    Ok(())
}
