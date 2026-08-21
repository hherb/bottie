//! Bounded attachment streaming, content sniffing, and safe-name policy.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use sha2::{Digest, Sha256};

use super::{StorageError, docx};

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const MIME_SNIFF_BYTES: usize = 8 * 1024;
const MAX_DISPLAY_NAME_CHARACTERS: usize = 120;
const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;
const MAX_ATTACHMENT_MEBIBYTES: u64 = 25;

/// Maximum bytes accepted for one selected attachment.
pub(super) const MAX_ATTACHMENT_BYTES: u64 = MAX_ATTACHMENT_MEBIBYTES * BYTES_PER_MEBIBYTE;

/// Prepared content and safe metadata awaiting one database transaction.
pub(super) struct PreparedAttachment {
    /// Sanitized display name derived from the selected leaf name.
    pub(super) display_name: String,
    /// MIME inferred from retained content.
    pub(super) mime_type: String,
    /// Exact retained byte size.
    pub(super) byte_size: u64,
    /// Lowercase SHA-256 content identity.
    pub(super) sha256: String,
}

/// Rejects non-files, empty files, and files over the native policy ceiling.
pub(super) fn validate_source(source_path: &Path) -> Result<(), StorageError> {
    let metadata = fs::metadata(source_path).map_err(|_| StorageError::attachment_read())?;
    if !metadata.is_file() {
        return Err(StorageError::invalid("Choose a regular file to attach."));
    }
    if metadata.len() == 0 {
        return Err(StorageError::invalid("Empty files cannot be attached."));
    }
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Err(attachment_too_large());
    }
    Ok(())
}

/// Copies and hashes one source through a bounded streaming buffer.
pub(super) fn prepare_blob(
    source_path: &Path,
    temporary_path: &Path,
    display_name: &str,
) -> Result<PreparedAttachment, StorageError> {
    let mut source = File::open(source_path).map_err(|_| StorageError::attachment_read())?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary_path)?;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut sniffed = Vec::with_capacity(MIME_SNIFF_BYTES);
    let mut hasher = Sha256::new();
    let mut byte_size = 0_u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| StorageError::attachment_read())?;
        if read == 0 {
            break;
        }
        byte_size = byte_size
            .checked_add(read as u64)
            .ok_or_else(attachment_too_large)?;
        if byte_size > MAX_ATTACHMENT_BYTES {
            return Err(attachment_too_large());
        }
        let sniff_remaining = MIME_SNIFF_BYTES.saturating_sub(sniffed.len());
        sniffed.extend_from_slice(&buffer[..read.min(sniff_remaining)]);
        hasher.update(&buffer[..read]);
        destination.write_all(&buffer[..read])?;
    }
    destination.sync_all()?;
    if byte_size == 0 {
        return Err(StorageError::invalid("Empty files cannot be attached."));
    }
    drop(destination);
    let sniffed_mime_type = detect_mime_type(&sniffed, display_name);
    let mime_type =
        if sniffed_mime_type == "application/zip" && docx::is_docx_package(temporary_path) {
            docx::DOCX_MIME_TYPE
        } else {
            sniffed_mime_type
        };
    Ok(PreparedAttachment {
        display_name: display_name.into(),
        mime_type: mime_type.into(),
        byte_size,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

/// Infers MIME from content signatures, then falls back to inert text or binary.
pub(super) fn detect_mime_type(bytes: &[u8], _display_name: &str) -> &'static str {
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type();
    }
    if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok() {
        return "text/plain";
    }
    "application/octet-stream"
}

/// Removes path separators, controls, bidi overrides, excess whitespace, and unsafe length.
pub(crate) fn safe_display_name(value: &str) -> String {
    let filtered: String = value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(character, '/' | '\\')
                && !matches!(*character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect();
    let normalized = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded: String = normalized
        .trim_matches(|character: char| character == '.' || character.is_whitespace())
        .chars()
        .take(MAX_DISPLAY_NAME_CHARACTERS)
        .collect();
    if bounded.is_empty() {
        "attachment".into()
    } else {
        bounded
    }
}

/// Creates the stable rejection used by both metadata and streaming size checks.
fn attachment_too_large() -> StorageError {
    StorageError::invalid(format!(
        "Attachments must be {MAX_ATTACHMENT_MEBIBYTES} MiB or smaller."
    ))
}
