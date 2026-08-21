//! Bounded native previews for ready normalized image attachments.

use std::{fs::File, io::BufReader};

use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Limits,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use rusqlite::OptionalExtension;

use super::{
    ConversationStore, StorageError,
    image_codec::{MAX_DECODED_IMAGE_BYTES, MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS},
    image_normalization::NormalizedImageFormat,
};

const PREVIEW_MAX_AXIS: u32 = 320;
const PREVIEW_MAX_BYTES: usize = 2 * 1024 * 1024;
const PREVIEW_JPEG_QUALITY: u8 = 82;

/// One path-free, metadata-free image preview response.
pub(crate) struct AttachmentPreview {
    /// Exact media type selected from trusted normalization state.
    pub(crate) mime_type: &'static str,
    /// Bounded re-encoded preview pixels.
    pub(crate) bytes: Vec<u8>,
}

impl ConversationStore {
    /// Loads one ready image preview by opaque attachment identity.
    pub(crate) fn load_attachment_preview(
        &self,
        attachment_id: &str,
    ) -> Result<Option<AttachmentPreview>, StorageError> {
        let derivative = self
            .open()?
            .query_row(
                "SELECT attachment_image_normalizations.normalized_sha256,
                        attachment_image_normalizations.format
                 FROM attachments
                 JOIN attachment_image_normalizations
                   ON attachment_image_normalizations.attachment_id = attachments.id
                 WHERE attachments.id = ?1
                   AND attachment_image_normalizations.state = 'ready'",
                [attachment_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        derivative
            .map(|(sha256, format)| self.encode_preview(&sha256, &format))
            .transpose()
    }

    /// Reads one trusted derivative and re-encodes only bounded thumbnail pixels.
    fn encode_preview(
        &self,
        sha256: &str,
        stored_format: &str,
    ) -> Result<AttachmentPreview, StorageError> {
        let format = NormalizedImageFormat::from_database(stored_format)?;
        let image_format = match format {
            NormalizedImageFormat::Jpeg => ImageFormat::Jpeg,
            NormalizedImageFormat::Png => ImageFormat::Png,
        };
        let source = File::open(self.normalized_image_path(sha256, format)?)?;
        let reader = ImageReader::with_format(BufReader::new(source), image_format);
        let mut decoder = reader
            .into_decoder()
            .map_err(|_| StorageError::internal())?;
        let (width, height) = decoder.dimensions();
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(StorageError::internal)?;
        if width == 0
            || height == 0
            || width > MAX_IMAGE_DIMENSION
            || height > MAX_IMAGE_DIMENSION
            || pixels > MAX_IMAGE_PIXELS
        {
            return Err(StorageError::internal());
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
        limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
        limits.max_alloc = Some(MAX_DECODED_IMAGE_BYTES);
        decoder
            .set_limits(limits)
            .map_err(|_| StorageError::internal())?;
        if decoder.total_bytes() > MAX_DECODED_IMAGE_BYTES {
            return Err(StorageError::internal());
        }
        let image = DynamicImage::from_decoder(decoder).map_err(|_| StorageError::internal())?;
        let preview = image.thumbnail(PREVIEW_MAX_AXIS, PREVIEW_MAX_AXIS);
        let mut bytes = Vec::new();
        match format {
            NormalizedImageFormat::Jpeg => {
                let pixels = preview.into_rgb8();
                JpegEncoder::new_with_quality(&mut bytes, PREVIEW_JPEG_QUALITY)
                    .write_image(
                        pixels.as_raw(),
                        pixels.width(),
                        pixels.height(),
                        ExtendedColorType::Rgb8,
                    )
                    .map_err(|_| StorageError::internal())?;
            }
            NormalizedImageFormat::Png => {
                let pixels = preview.into_rgba8();
                PngEncoder::new(&mut bytes)
                    .write_image(
                        pixels.as_raw(),
                        pixels.width(),
                        pixels.height(),
                        ExtendedColorType::Rgba8,
                    )
                    .map_err(|_| StorageError::internal())?;
            }
        }
        if bytes.len() > PREVIEW_MAX_BYTES {
            return Err(StorageError::internal());
        }
        Ok(AttachmentPreview {
            mime_type: match format {
                NormalizedImageFormat::Jpeg => "image/jpeg",
                NormalizedImageFormat::Png => "image/png",
            },
            bytes,
        })
    }
}
