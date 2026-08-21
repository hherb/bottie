//! Bounded native preparation of durable image context for provider requests.

use std::{collections::HashSet, fs::File, io::Read};

use rusqlite::Connection;

use super::{
    ConversationStore, MessageState, StorageError, StoredRole,
    image_normalization::NormalizedImageFormat, load_conversation_from_connection,
};

const BYTES_PER_MEBIBYTE: u64 = 1_024 * 1_024;
const MAX_PROVIDER_IMAGE_COUNT: usize = 8;
const MAX_PROVIDER_IMAGE_MEBIBYTES: u64 = 50;
const MAX_PROVIDER_IMAGE_BYTES: u64 = MAX_PROVIDER_IMAGE_MEBIBYTES * BYTES_PER_MEBIBYTE;

/// Encoding of one normalized image prepared for a provider-specific wire shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderImageFormat {
    /// Metadata-free JPEG derivative.
    Jpeg,
    /// Metadata-free PNG derivative.
    Png,
}

/// Bounded native image content associated with one durable user turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderContextImage {
    /// Derivative encoding used by the provider request serializer.
    pub(crate) format: ProviderImageFormat,
    /// Native-only derivative identity used only for deferred bounded loading.
    pub(crate) sha256: String,
    /// Trusted encoded size used to enforce the complete request ceiling before reads.
    pub(crate) byte_size: u64,
    /// Normalized bytes populated only after vision capability is confirmed.
    pub(crate) bytes: Option<Vec<u8>>,
}

/// One durable selected-lineage turn prepared as provider context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderContextMessage {
    /// Durable participant role.
    pub(crate) role: StoredRole,
    /// Exact stored message text.
    pub(crate) text: String,
    /// Ordered normalized images associated with this user turn.
    pub(crate) images: Vec<ProviderContextImage>,
}

/// Complete native-selected context plus current-request image policy metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderAttachmentContext {
    /// Ordered successful selected-lineage messages.
    pub(crate) messages: Vec<ProviderContextMessage>,
    /// Whether the latest request requires a vision-capable model.
    pub(crate) current_request_has_image: bool,
}

#[derive(Clone)]
struct ReadyImage {
    format: NormalizedImageFormat,
    sha256: String,
    byte_size: u64,
}

impl ConversationStore {
    /// Loads exact durable text plus bounded normalized image bytes for one accepted request.
    pub(crate) fn provider_attachment_context(
        &self,
        conversation_id: &str,
        request_message_id: &str,
    ) -> Result<ProviderAttachmentContext, StorageError> {
        let connection = self.open()?;
        let conversation = load_conversation_from_connection(&connection, conversation_id)?;
        if conversation
            .messages
            .last()
            .map(|message| message.id.as_str())
            != Some(request_message_id)
        {
            return Err(StorageError::invalid(
                "The provider request no longer matches the selected conversation branch.",
            ));
        }
        let conversation_attachment_ids = conversation
            .attachments
            .iter()
            .map(|attachment| attachment.id.clone())
            .collect::<HashSet<_>>();
        let mut current_request_has_image = conversation
            .attachments
            .iter()
            .any(|attachment| attachment.mime_type.starts_with("image/"));
        let conversation_images = ready_conversation_images(&connection, conversation_id)?;
        let mut messages = Vec::new();
        for message in conversation.messages {
            if !include_in_provider_context(message.role, message.state, &message.text) {
                continue;
            }
            let is_current = message.id == request_message_id;
            let mut ready = ready_message_images(
                &connection,
                &message.id,
                is_current,
                &conversation_attachment_ids,
            )?;
            if is_current {
                current_request_has_image |= message
                    .attachments
                    .iter()
                    .any(|attachment| attachment.mime_type.starts_with("image/"));
                ready.extend(conversation_images.iter().cloned());
            }
            messages.push(ProviderContextMessage {
                role: message.role,
                text: message.text,
                images: ready.into_iter().map(ProviderContextImage::from).collect(),
            });
        }
        Ok(ProviderAttachmentContext {
            messages,
            current_request_has_image,
        })
    }

    /// Enforces the vision-request ceiling, then reads each trusted derivative exactly once.
    pub(crate) fn load_provider_images(
        &self,
        mut context: ProviderAttachmentContext,
    ) -> Result<ProviderAttachmentContext, StorageError> {
        let mut image_count = 0_usize;
        let mut image_bytes = 0_u64;
        for message in &context.messages {
            for image in &message.images {
                image_count += 1;
                image_bytes = image_bytes
                    .checked_add(image.byte_size)
                    .ok_or_else(provider_image_limit_error)?;
            }
        }
        if image_count > MAX_PROVIDER_IMAGE_COUNT || image_bytes > MAX_PROVIDER_IMAGE_BYTES {
            return Err(provider_image_limit_error());
        }
        for message in &mut context.messages {
            for image in &mut message.images {
                let path = self.normalized_image_path(&image.sha256, image.format.into())?;
                let mut bytes = Vec::with_capacity(image.byte_size as usize);
                File::open(path)?
                    .take(image.byte_size.saturating_add(1))
                    .read_to_end(&mut bytes)?;
                if bytes.len() as u64 != image.byte_size {
                    return Err(StorageError::internal());
                }
                image.bytes = Some(bytes);
            }
        }
        Ok(context)
    }
}

impl From<ReadyImage> for ProviderContextImage {
    fn from(image: ReadyImage) -> Self {
        Self {
            format: match image.format {
                NormalizedImageFormat::Jpeg => ProviderImageFormat::Jpeg,
                NormalizedImageFormat::Png => ProviderImageFormat::Png,
            },
            sha256: image.sha256,
            byte_size: image.byte_size,
            bytes: None,
        }
    }
}

impl From<ProviderImageFormat> for NormalizedImageFormat {
    fn from(format: ProviderImageFormat) -> Self {
        match format {
            ProviderImageFormat::Jpeg => Self::Jpeg,
            ProviderImageFormat::Png => Self::Png,
        }
    }
}

/// Mirrors the presentation's provider-context filtering using durable states.
fn include_in_provider_context(role: StoredRole, state: MessageState, text: &str) -> bool {
    !text.trim().is_empty()
        && !(role == StoredRole::Assistant
            && matches!(state, MessageState::Partial | MessageState::Failed))
}

/// Resolves ready derivatives and enforces explicit current-request image states.
fn ready_message_images(
    connection: &Connection,
    message_id: &str,
    is_current: bool,
    excluded_attachment_ids: &HashSet<String>,
) -> Result<Vec<ReadyImage>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT attachments.id, attachments.mime_type, attachment_image_normalizations.state,
                attachment_image_normalizations.format,
                attachment_image_normalizations.normalized_sha256,
                attachment_image_normalizations.byte_size
         FROM message_attachments
         JOIN attachments ON attachments.id = message_attachments.attachment_id
         JOIN attachment_image_normalizations
           ON attachment_image_normalizations.attachment_id = attachments.id
         WHERE message_attachments.message_id = ?1
         ORDER BY message_attachments.ordinal",
    )?;
    let rows = statement.query_map([message_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })?;
    let mut ready = Vec::new();
    for row in rows {
        let (attachment_id, mime_type, state, format, sha256, byte_size) = row?;
        if !mime_type.starts_with("image/") {
            continue;
        }
        if excluded_attachment_ids.contains(&attachment_id) {
            continue;
        }
        if is_current && state == "pending" {
            return Err(StorageError::invalid(
                "Wait for image normalization to finish before sending.",
            ));
        }
        if is_current && state != "ready" {
            return Err(StorageError::invalid(
                "Remove images that could not be normalized as JPEG or PNG before sending.",
            ));
        }
        if state != "ready" {
            continue;
        }
        let format = format
            .as_deref()
            .map(NormalizedImageFormat::from_database)
            .transpose()?
            .ok_or_else(StorageError::internal)?;
        ready.push(ReadyImage {
            format,
            sha256: sha256.ok_or_else(StorageError::internal)?,
            byte_size: byte_size
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(StorageError::internal)?,
        });
    }
    Ok(ready)
}

/// Resolves conversation-scoped images as required context for the current request.
fn ready_conversation_images(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Vec<ReadyImage>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT attachments.id, attachments.mime_type, attachment_image_normalizations.state,
                attachment_image_normalizations.format,
                attachment_image_normalizations.normalized_sha256,
                attachment_image_normalizations.byte_size
         FROM conversation_attachments
         JOIN attachments ON attachments.id = conversation_attachments.attachment_id
         JOIN attachment_image_normalizations
           ON attachment_image_normalizations.attachment_id = attachments.id
         WHERE conversation_attachments.conversation_id = ?1
         ORDER BY conversation_attachments.ordinal",
    )?;
    let rows = statement.query_map([conversation_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })?;
    let mut ready = Vec::new();
    for row in rows {
        let (_attachment_id, mime_type, state, format, sha256, byte_size) = row?;
        if !mime_type.starts_with("image/") {
            continue;
        }
        if state == "pending" {
            return Err(StorageError::invalid(
                "Wait for image normalization to finish before sending.",
            ));
        }
        if state != "ready" {
            return Err(StorageError::invalid(
                "Remove images that could not be normalized as JPEG or PNG before sending.",
            ));
        }
        ready.push(ReadyImage {
            format: format
                .as_deref()
                .map(NormalizedImageFormat::from_database)
                .transpose()?
                .ok_or_else(StorageError::internal)?,
            sha256: sha256.ok_or_else(StorageError::internal)?,
            byte_size: byte_size
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(StorageError::internal)?,
        });
    }
    Ok(ready)
}

/// Returns a stable, path-free request ceiling failure.
fn provider_image_limit_error() -> StorageError {
    StorageError::invalid(format!(
        concat!(
            "Send at most {} images and {} MiB ",
            "of normalized image data per request."
        ),
        MAX_PROVIDER_IMAGE_COUNT, MAX_PROVIDER_IMAGE_MEBIBYTES
    ))
}
