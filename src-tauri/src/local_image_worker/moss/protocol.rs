//! Bounded model-bound result decoding independent of processes and filesystem access.
use super::MossError;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
pub(super) const MAX_PNG_BYTES: usize = 25 * 1024 * 1024;
pub(super) const MAX_EVENT_BYTES: usize = 34 * 1024 * 1024;

/// A worker cannot complete another model, revision, seed, size, or output format successfully.
pub(super) fn decode_complete(
    event: &Value,
    model: &str,
    revision: &str,
    size: (u32, u32),
    seed: u64,
) -> Result<Vec<u8>, MossError> {
    if event["event"] != "complete"
        || event["modelId"] != model
        || event["revision"] != revision
        || event["width"].as_u64() != Some(u64::from(size.0))
        || event["height"].as_u64() != Some(u64::from(size.1))
        || event["seed"].as_u64() != Some(seed)
    {
        return Err(MossError::InvalidOutput);
    }
    let encoded = event["png"].as_str().ok_or(MossError::InvalidOutput)?;
    if encoded.len() > MAX_PNG_BYTES * 4 / 3 + 4 {
        return Err(MossError::InvalidOutput);
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| MossError::InvalidOutput)?;
    if bytes.len() > MAX_PNG_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(MossError::InvalidOutput);
    }
    Ok(bytes)
}
