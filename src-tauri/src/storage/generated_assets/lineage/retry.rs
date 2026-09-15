//! Exact native edit-source reconstruction for durable retries.

use rusqlite::Connection;

use super::{
    ConversationStore, GeneratedImageSourceFormat, GeneratedImageSourceReference,
    GeneratedImageSourceType, SourceMetadata, StorageError, ValidatedGeneratedImageSource,
    load_generated_asset_sources,
};

impl ConversationStore {
    /// Reopens one failed edit's identical per-output source snapshot for an exact native retry.
    pub(in crate::storage::generated_assets) fn load_generated_image_retry_sources(
        &self,
        connection: &Connection,
        message_id: &str,
    ) -> Result<Vec<ValidatedGeneratedImageSource>, StorageError> {
        let mut statement = connection
            .prepare("SELECT id FROM generated_assets WHERE message_id = ?1 ORDER BY ordinal")?;
        let asset_ids = statement
            .query_map([message_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let first_id = asset_ids.first().ok_or_else(StorageError::internal)?;
        let first = load_generated_asset_sources(connection, first_id)?;
        for asset_id in asset_ids.iter().skip(1) {
            if load_generated_asset_sources(connection, asset_id)? != first {
                return Err(StorageError::invalid(
                    "That image response cannot be retried.",
                ));
            }
        }
        let metadata = first
            .into_iter()
            .map(|source| {
                Ok(SourceMetadata {
                    reference: match source.source_type {
                        GeneratedImageSourceType::Attachment => {
                            GeneratedImageSourceReference::Attachment(source.source_id)
                        }
                        GeneratedImageSourceType::GeneratedAsset => {
                            GeneratedImageSourceReference::GeneratedAsset(source.source_id)
                        }
                    },
                    format: match source.media_type.as_str() {
                        "image/jpeg" => GeneratedImageSourceFormat::Jpeg,
                        "image/png" => GeneratedImageSourceFormat::Png,
                        _ => return Err(StorageError::internal()),
                    },
                    width: source.width,
                    height: source.height,
                    byte_size: source.byte_size,
                    sha256: source.sha256,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.load_exact_source_bytes(metadata)
    }
}
