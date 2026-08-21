//! Bounded metadata-free normalization for retained JPEG and PNG images.

use std::{fs, path::PathBuf};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::{
    ConversationStore, StorageError,
    image_codec::{
        ERROR_IMAGE_MISSING_CONTENT, ERROR_IMAGE_WRITE_FAILED, NormalizedImage,
        move_normalized_image, normalize_image,
    },
    now_ms,
};

const NORMALIZED_DIRECTORY_NAME: &str = "normalized-images";
const NORMALIZATION_TEMPORARY_DIRECTORY_NAME: &str = "normalization-temporary";

/// Current durable state of native image normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImageNormalizationState {
    /// Native normalization has not yet reached a terminal state.
    Pending,
    /// A bounded metadata-free derivative is available in application-private storage.
    Ready,
    /// The retained attachment is not a JPEG or PNG image.
    Unsupported,
    /// A supported-looking image could not be normalized within policy.
    Failed,
}

impl ImageNormalizationState {
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
    fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "pending" => Ok(Self::Pending),
            "ready" => Ok(Self::Ready),
            "unsupported" => Ok(Self::Unsupported),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Encoding used for one metadata-free normalized derivative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NormalizedImageFormat {
    /// Metadata-free JPEG with orientation applied to its pixels.
    Jpeg,
    /// Metadata-free lossless PNG.
    Png,
}

impl NormalizedImageFormat {
    /// Resolves the only source MIME types supported by this slice.
    fn from_mime_type(mime_type: &str) -> Option<Self> {
        match mime_type {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            _ => None,
        }
    }

    /// Returns the stable SQLite representation.
    fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
        }
    }

    /// Parses a trusted value constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "jpeg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Path-free normalization metadata safe to return without derivative identities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredImageNormalization {
    /// Current native normalization state.
    pub(crate) state: ImageNormalizationState,
    /// Ready derivative format, absent for every other state.
    pub(crate) format: Option<NormalizedImageFormat>,
    /// Oriented derivative width in pixels.
    pub(crate) width: Option<u64>,
    /// Oriented derivative height in pixels.
    pub(crate) height: Option<u64>,
    /// Exact derivative byte count without exposing its bytes or path.
    pub(crate) byte_size: Option<u64>,
    /// Stable path-free failure category for failed normalization.
    pub(crate) error_code: Option<String>,
}

impl StoredImageNormalization {
    /// Creates the initial state used by newly retained content.
    pub(super) fn pending_or_unsupported(mime_type: &str) -> Self {
        let state = if NormalizedImageFormat::from_mime_type(mime_type).is_some() {
            ImageNormalizationState::Pending
        } else {
            ImageNormalizationState::Unsupported
        };
        Self {
            state,
            format: None,
            width: None,
            height: None,
            byte_size: None,
            error_code: None,
        }
    }

    /// Decodes trusted path-free columns shared by attachment queries.
    pub(super) fn from_row(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Self> {
        let state = ImageNormalizationState::from_database(&row.get::<_, String>(offset)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let format = row
            .get::<_, Option<String>>(offset + 1)?
            .as_deref()
            .map(NormalizedImageFormat::from_database)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        Ok(Self {
            state,
            format,
            width: row
                .get::<_, Option<i64>>(offset + 2)?
                .map(|value| value as u64),
            height: row
                .get::<_, Option<i64>>(offset + 3)?
                .map(|value| value as u64),
            byte_size: row
                .get::<_, Option<i64>>(offset + 4)?
                .map(|value| value as u64),
            error_code: row.get(offset + 5)?,
        })
    }
}

impl ConversationStore {
    /// Resolves one pending image into ready, unsupported, or failed native state.
    pub(super) fn process_image_normalization(
        &self,
        attachment_id: &str,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        let attachment = connection
            .query_row(
                "SELECT attachments.mime_type, attachments.sha256,
                        attachment_image_normalizations.state
                 FROM attachments
                 JOIN attachment_image_normalizations
                   ON attachment_image_normalizations.attachment_id = attachments.id
                 WHERE attachments.id = ?1",
                [attachment_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        drop(connection);
        let Some((mime_type, source_sha256, state)) = attachment else {
            return Err(StorageError::internal());
        };
        if state != ImageNormalizationState::Pending.as_str() {
            return Ok(());
        }
        let Some(format) = NormalizedImageFormat::from_mime_type(&mime_type) else {
            return self.persist_unsupported_normalization(attachment_id);
        };
        let source_path = self.attachment_blob_path(&source_sha256);
        if !source_path.is_file() {
            return self.persist_normalization_failure(attachment_id, ERROR_IMAGE_MISSING_CONTENT);
        }
        let temporary_directory = self
            .attachment_root()
            .join(NORMALIZATION_TEMPORARY_DIRECTORY_NAME);
        if fs::create_dir_all(&temporary_directory).is_err() {
            return self.persist_normalization_failure(attachment_id, ERROR_IMAGE_WRITE_FAILED);
        }
        let temporary_path = temporary_directory.join(format!("{}.part", uuid::Uuid::new_v4()));
        let normalized = match normalize_image(&source_path, &temporary_path, format) {
            Ok(normalized) => normalized,
            Err(error_code) => {
                let _ = fs::remove_file(&temporary_path);
                return self.persist_normalization_failure(attachment_id, error_code);
            }
        };
        let destination = self.normalized_image_path(&normalized.sha256, normalized.format)?;
        let created = match move_normalized_image(&temporary_path, &destination) {
            Ok(created) => created,
            Err(()) => {
                let _ = fs::remove_file(&temporary_path);
                return self.persist_normalization_failure(attachment_id, ERROR_IMAGE_WRITE_FAILED);
            }
        };
        let persisted = self.persist_ready_normalization(attachment_id, &normalized);
        if persisted.is_err() && created {
            let _ = fs::remove_file(destination);
        }
        persisted
    }

    /// Persists one ready derivative while keeping its content identity native-only.
    fn persist_ready_normalization(
        &self,
        attachment_id: &str,
        normalized: &NormalizedImage,
    ) -> Result<(), StorageError> {
        self.open()?.execute(
            "UPDATE attachment_image_normalizations
             SET state = 'ready', format = ?1, width = ?2, height = ?3, byte_size = ?4,
                 normalized_sha256 = ?5, error_code = NULL, updated_at_ms = ?6
             WHERE attachment_id = ?7 AND state = 'pending'",
            params![
                normalized.format.as_str(),
                i64::from(normalized.width),
                i64::from(normalized.height),
                normalized.byte_size as i64,
                normalized.sha256,
                now_ms()?,
                attachment_id
            ],
        )?;
        Ok(())
    }

    /// Persists one unsupported transition under the schema invariant.
    fn persist_unsupported_normalization(&self, attachment_id: &str) -> Result<(), StorageError> {
        self.open()?.execute(
            "UPDATE attachment_image_normalizations
             SET state = 'unsupported', updated_at_ms = ?1
             WHERE attachment_id = ?2 AND state = 'pending'",
            params![now_ms()?, attachment_id],
        )?;
        Ok(())
    }

    /// Persists one path-free failed state without retaining a partial derivative.
    fn persist_normalization_failure(
        &self,
        attachment_id: &str,
        error_code: &str,
    ) -> Result<(), StorageError> {
        self.open()?.execute(
            "UPDATE attachment_image_normalizations
             SET state = 'failed', format = NULL, width = NULL, height = NULL,
                 byte_size = NULL, normalized_sha256 = NULL, error_code = ?1, updated_at_ms = ?2
             WHERE attachment_id = ?3 AND state = 'pending'",
            params![error_code, now_ms()?, attachment_id],
        )?;
        Ok(())
    }

    /// Resolves a normalized content hash to its private derivative location.
    pub(super) fn normalized_image_path(
        &self,
        sha256: &str,
        format: NormalizedImageFormat,
    ) -> Result<PathBuf, StorageError> {
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(StorageError::internal());
        }
        Ok(self
            .attachment_root()
            .join(NORMALIZED_DIRECTORY_NAME)
            .join(&sha256[..2])
            .join(format!("{}.{}", sha256, format.as_str())))
    }

    /// Loads normalized bytes only through the storage test boundary.
    #[cfg(test)]
    pub(super) fn normalized_image_bytes_for_test(
        &self,
        attachment_id: &str,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let connection = self.open()?;
        let derivative = connection
            .query_row(
                "SELECT normalized_sha256, format FROM attachment_image_normalizations
                 WHERE attachment_id = ?1 AND state = 'ready'",
                [attachment_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        derivative
            .map(|(sha256, format)| {
                let format = NormalizedImageFormat::from_database(&format)?;
                fs::read(self.normalized_image_path(&sha256, format)?).map_err(Into::into)
            })
            .transpose()
    }
}
