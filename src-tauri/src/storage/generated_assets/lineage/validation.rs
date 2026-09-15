//! Selected-lineage source and exact hosted-provenance validation.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    GeneratedImageProvenance, GeneratedImageSourceFormat, GeneratedImageSourceReference,
    HOSTED_EDIT_MODEL_ID, HOSTED_EDIT_PROVIDER_ID, MAX_EDIT_SOURCE_COUNT,
    MAX_EDIT_SOURCE_MEBIBYTES, MIN_EDIT_SOURCE_COUNT, SourceMetadata,
};
use crate::storage::{
    GeneratedAssetExecution, StorageError, image_normalization::NormalizedImageFormat,
};

/// Resolves source metadata only from the exact durable current request and its ancestry.
pub(super) fn load_source_metadata(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    request_message_id: &str,
    references: &[GeneratedImageSourceReference],
) -> Result<Vec<SourceMetadata>, StorageError> {
    let metadata = references
        .iter()
        .map(|reference| match reference {
            GeneratedImageSourceReference::Attachment(id) => {
                load_attachment_source(transaction, request_message_id, id)
            }
            GeneratedImageSourceReference::GeneratedAsset(id) => {
                load_generated_source(transaction, conversation_id, request_message_id, id)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut identities = HashSet::new();
    if metadata
        .iter()
        .any(|source| !identities.insert(source.sha256.clone()))
    {
        return Err(invalid_source_error());
    }
    Ok(metadata)
}

/// Rejects malformed, duplicate, empty, or excessive source selections before mutation.
pub(super) fn validate_source_references(
    references: &[GeneratedImageSourceReference],
) -> Result<(), StorageError> {
    if !(MIN_EDIT_SOURCE_COUNT..=MAX_EDIT_SOURCE_COUNT).contains(&references.len()) {
        return Err(invalid_source_error());
    }
    let mut unique = HashSet::new();
    for reference in references {
        validate_reference(reference)?;
        if !unique.insert(reference.clone()) {
            return Err(invalid_source_error());
        }
    }
    Ok(())
}

/// Requires the one hosted editing model and forbids local or seeded provenance substitution.
pub(super) fn validate_edit_provenance(
    provenance: &GeneratedImageProvenance,
) -> Result<(), StorageError> {
    if provenance.provider_id != HOSTED_EDIT_PROVIDER_ID
        || provenance.model_id != HOSTED_EDIT_MODEL_ID
        || provenance.execution != GeneratedAssetExecution::Cloud
        || provenance.seed.is_some()
    {
        return Err(StorageError::invalid(
            "Image editing requires exact hosted Qwen-Image-2.0 provenance.",
        ));
    }
    Ok(())
}

/// Creates one fixed path-free aggregate byte-limit error.
pub(super) fn source_limit_error() -> StorageError {
    StorageError::invalid(format!(
        "Each image editing source may contain at most {MAX_EDIT_SOURCE_MEBIBYTES} MiB of normalized bytes."
    ))
}

/// Resolves one ready derivative explicitly associated with the exact current request.
fn load_attachment_source(
    transaction: &Transaction<'_>,
    request_message_id: &str,
    attachment_id: &str,
) -> Result<SourceMetadata, StorageError> {
    transaction
        .query_row(
            "SELECT normalization.format, normalization.width, normalization.height,
                    normalization.byte_size, normalization.normalized_sha256
             FROM message_attachments
             JOIN attachment_image_normalizations AS normalization
               ON normalization.attachment_id = message_attachments.attachment_id
             WHERE message_attachments.message_id = ?1
               AND message_attachments.attachment_id = ?2 AND normalization.state = 'ready'",
            params![request_message_id, attachment_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .map(|row| {
            Ok::<SourceMetadata, StorageError>(SourceMetadata {
                reference: GeneratedImageSourceReference::Attachment(attachment_id.to_owned()),
                format: GeneratedImageSourceFormat::from_normalized(
                    NormalizedImageFormat::from_database(&row.0)?,
                ),
                width: row.1,
                height: row.2,
                byte_size: u64::try_from(row.3).map_err(|_| StorageError::internal())?,
                sha256: row.4,
            })
        })
        .transpose()?
        .ok_or_else(invalid_source_error)
}

/// Resolves one completed generated image that precedes the exact current request.
fn load_generated_source(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    request_message_id: &str,
    asset_id: &str,
) -> Result<SourceMetadata, StorageError> {
    transaction
        .query_row(
            "WITH RECURSIVE ancestry(id) AS (
                 SELECT parent_message_id FROM messages WHERE id = ?1
                 UNION ALL
                 SELECT messages.parent_message_id
                 FROM messages JOIN ancestry ON messages.id = ancestry.id
                 WHERE ancestry.id IS NOT NULL
             )
             SELECT generated_assets.width, generated_assets.height, generated_assets.byte_size,
                    generated_assets.sha256
             FROM generated_assets
             JOIN messages ON messages.id = generated_assets.message_id
             WHERE generated_assets.id = ?2 AND generated_assets.status = 'completed'
               AND messages.conversation_id = ?3
               AND EXISTS (SELECT 1 FROM ancestry WHERE ancestry.id = messages.id)",
            params![request_message_id, asset_id, conversation_id],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .map(|row| {
            Ok::<SourceMetadata, StorageError>(SourceMetadata {
                reference: GeneratedImageSourceReference::GeneratedAsset(asset_id.to_owned()),
                format: GeneratedImageSourceFormat::Png,
                width: row.0,
                height: row.1,
                byte_size: u64::try_from(row.2).map_err(|_| StorageError::internal())?,
                sha256: row.3,
            })
        })
        .transpose()?
        .ok_or_else(invalid_source_error)
}

/// Requires canonical UUID-shaped native identities rather than path-shaped strings.
pub(super) fn validate_reference(
    reference: &GeneratedImageSourceReference,
) -> Result<(), StorageError> {
    let id = reference.id();
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| invalid_source_error())?;
    if parsed.to_string() != id {
        return Err(invalid_source_error());
    }
    Ok(())
}

/// Creates one fixed path-free invalid-source error.
fn invalid_source_error() -> StorageError {
    StorageError::invalid(
        "Choose between one and three valid images from the selected request lineage.",
    )
}
