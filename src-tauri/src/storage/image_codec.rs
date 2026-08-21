//! Resource-bounded JPEG and PNG decoding, orientation, and metadata-free encoding.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Write},
    path::Path,
};

use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder, ImageFormat,
    ImageReader, Limits,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use sha2::{Digest, Sha256};

use super::image_normalization::NormalizedImageFormat;

const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;
const MAX_DECODED_IMAGE_MEBIBYTES: u64 = 128;
const MAX_DECODED_IMAGE_BYTES: u64 = MAX_DECODED_IMAGE_MEBIBYTES * BYTES_PER_MEBIBYTE;
const MAX_NORMALIZED_IMAGE_MEBIBYTES: u64 = 25;
const MAX_NORMALIZED_IMAGE_BYTES: u64 = MAX_NORMALIZED_IMAGE_MEBIBYTES * BYTES_PER_MEBIBYTE;
const JPEG_QUALITY: u8 = 90;
const ERROR_IMAGE_DECODE_FAILED: &str = "image_decode_failed";
const ERROR_IMAGE_DECODE_LIMIT_EXCEEDED: &str = "image_decode_limit_exceeded";
const ERROR_IMAGE_DIMENSION_LIMIT_EXCEEDED: &str = "image_dimension_limit_exceeded";
pub(super) const ERROR_IMAGE_MISSING_CONTENT: &str = "image_missing_content";
const ERROR_IMAGE_OUTPUT_TOO_LARGE: &str = "image_output_too_large";
const ERROR_IMAGE_PIXEL_LIMIT_EXCEEDED: &str = "image_pixel_limit_exceeded";
pub(super) const ERROR_IMAGE_WRITE_FAILED: &str = "image_write_failed";

/// Maximum accepted width or height for one source image.
pub(super) const MAX_IMAGE_DIMENSION: u32 = 8_192;
/// Maximum accepted product of width and height for one source image.
pub(super) const MAX_IMAGE_PIXELS: u64 = 16_000_000;

/// Complete metadata needed to persist one successfully encoded derivative.
pub(super) struct NormalizedImage {
    pub(super) format: NormalizedImageFormat,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) byte_size: u64,
    pub(super) sha256: String,
}

/// Decodes, orients, and re-encodes one image without copying source metadata.
pub(super) fn normalize_image(
    source_path: &Path,
    destination_path: &Path,
    format: NormalizedImageFormat,
) -> Result<NormalizedImage, &'static str> {
    let source = File::open(source_path).map_err(|_| ERROR_IMAGE_MISSING_CONTENT)?;
    let image_format = match format {
        NormalizedImageFormat::Jpeg => ImageFormat::Jpeg,
        NormalizedImageFormat::Png => ImageFormat::Png,
    };
    let reader = ImageReader::with_format(BufReader::new(source), image_format);
    let mut decoder = reader.into_decoder().map_err(image_decode_error_code)?;
    let (width, height) = decoder.dimensions();
    validate_image_dimensions(width, height)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_IMAGE_BYTES);
    decoder
        .set_limits(limits)
        .map_err(image_decode_error_code)?;
    if decoder.total_bytes() > MAX_DECODED_IMAGE_BYTES {
        return Err(ERROR_IMAGE_DECODE_LIMIT_EXCEEDED);
    }
    let orientation = decoder.orientation().map_err(image_decode_error_code)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(image_decode_error_code)?;
    image.apply_orientation(orientation);
    let (width, height) = image.dimensions();
    validate_image_dimensions(width, height)?;
    encode_normalized_image(destination_path, image, format, width, height)
}

/// Moves a completed derivative into content-addressed storage without replacing an existing file.
pub(super) fn move_normalized_image(source: &Path, destination: &Path) -> Result<bool, ()> {
    let parent = destination.parent().ok_or(())?;
    fs::create_dir_all(parent).map_err(|_| ())?;
    if destination.exists() {
        fs::remove_file(source).map_err(|_| ())?;
        return Ok(false);
    }
    fs::rename(source, destination).map_err(|_| ())?;
    Ok(true)
}

/// Enforces independent dimension and total-pixel ceilings before pixel allocation.
fn validate_image_dimensions(width: u32, height: u32) -> Result<(), &'static str> {
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(ERROR_IMAGE_DIMENSION_LIMIT_EXCEEDED);
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(ERROR_IMAGE_PIXEL_LIMIT_EXCEEDED)?;
    if pixels > MAX_IMAGE_PIXELS {
        return Err(ERROR_IMAGE_PIXEL_LIMIT_EXCEEDED);
    }
    Ok(())
}

/// Re-encodes pixels through a capped hashing writer without forwarding source metadata.
fn encode_normalized_image(
    destination_path: &Path,
    image: DynamicImage,
    format: NormalizedImageFormat,
    width: u32,
    height: u32,
) -> Result<NormalizedImage, &'static str> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination_path)
        .map_err(|_| ERROR_IMAGE_WRITE_FAILED)?;
    let mut writer = BoundedHashWriter::new(file, MAX_NORMALIZED_IMAGE_BYTES);
    let encoded = match format {
        NormalizedImageFormat::Jpeg => {
            let pixels = image.into_rgb8();
            JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY).write_image(
                pixels.as_raw(),
                width,
                height,
                ExtendedColorType::Rgb8,
            )
        }
        NormalizedImageFormat::Png => {
            let pixels = image.into_rgba8();
            PngEncoder::new(&mut writer).write_image(
                pixels.as_raw(),
                width,
                height,
                ExtendedColorType::Rgba8,
            )
        }
    };
    if encoded.is_err() {
        return Err(if writer.exceeded {
            ERROR_IMAGE_OUTPUT_TOO_LARGE
        } else {
            ERROR_IMAGE_WRITE_FAILED
        });
    }
    writer.sync_all().map_err(|_| ERROR_IMAGE_WRITE_FAILED)?;
    let (byte_size, sha256) = writer.finish();
    Ok(NormalizedImage {
        format,
        width,
        height,
        byte_size,
        sha256,
    })
}

/// Maps decoder resource-limit failures separately from malformed image content.
fn image_decode_error_code(error: image::ImageError) -> &'static str {
    match error {
        image::ImageError::Limits(_) => ERROR_IMAGE_DECODE_LIMIT_EXCEEDED,
        _ => ERROR_IMAGE_DECODE_FAILED,
    }
}

/// Writer that rejects output beyond one exact ceiling while hashing accepted bytes.
struct BoundedHashWriter<W> {
    inner: W,
    hasher: Sha256,
    written: u64,
    limit: u64,
    exceeded: bool,
}

impl<W> BoundedHashWriter<W> {
    /// Wraps one destination with an exact byte ceiling.
    fn new(inner: W, limit: u64) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            written: 0,
            limit,
            exceeded: false,
        }
    }

    /// Returns the exact accepted byte count and lowercase SHA-256 identity.
    fn finish(self) -> (u64, String) {
        (self.written, format!("{:x}", self.hasher.finalize()))
    }
}

impl BoundedHashWriter<File> {
    /// Flushes the completed derivative to stable application-private storage.
    fn sync_all(&mut self) -> io::Result<()> {
        self.inner.flush()?;
        self.inner.sync_all()
    }
}

impl<W: Write> Write for BoundedHashWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let requested = buffer.len() as u64;
        if requested > self.limit.saturating_sub(self.written) {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "normalized image exceeds policy",
            ));
        }
        let written = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..written]);
        self.written += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::BoundedHashWriter;

    #[test]
    fn rejects_output_before_crossing_the_exact_byte_ceiling() {
        let mut writer = BoundedHashWriter::new(Vec::new(), 4);
        let error = writer
            .write_all(b"five!")
            .expect_err("oversized output should fail");

        assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge);
        assert!(writer.exceeded);
        assert_eq!(writer.written, 0);
    }
}
