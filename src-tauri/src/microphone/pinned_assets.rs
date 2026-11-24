//! The pinned-asset gate for local speech models: Bottie's release manifest names each model's repository, revision,
//! size and SHA-256, and a model file is only used after its bytes match. Shared by the Whisper recognizer and Silero.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use hf_hub::{
    Repo, RepoType,
    api::sync::{ApiBuilder, ApiRepo},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::transcription::TranscriptionErrorCode;

const RUNTIME_ASSET_MANIFEST: &str = include_str!("../../../runtime-assets.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeAssetManifest {
    schema_version: u8,
    whisper_tiny_q5: PinnedModelContract,
    silero_vad_ggml: PinnedModelContract,
}

#[derive(Deserialize)]
struct PinnedModelContract {
    repository: String,
    revision: String,
    file: ModelFileContract,
}

#[derive(Deserialize)]
struct ModelFileContract {
    path: String,
    sha256: String,
    size: u64,
}

fn runtime_asset_contract() -> Result<RuntimeAssetManifest, TranscriptionErrorCode> {
    let contract: RuntimeAssetManifest = serde_json::from_str(RUNTIME_ASSET_MANIFEST)
        .map_err(|_| TranscriptionErrorCode::ModelIntegrity)?;
    if contract.schema_version != 1 {
        return Err(TranscriptionErrorCode::ModelIntegrity);
    }
    Ok(contract)
}

pub(super) fn prepare_pinned_model(cache_dir: &PathBuf) -> Result<PathBuf, TranscriptionErrorCode> {
    let contract = runtime_asset_contract()?;
    prepare_pinned_contract(cache_dir, &contract.whisper_tiny_q5)
}

/// Returns the verified Silero VAD ggml model path, using the same gate as the recognition model.
pub(super) fn prepare_pinned_vad_model(
    cache_dir: &PathBuf,
) -> Result<PathBuf, TranscriptionErrorCode> {
    let contract = runtime_asset_contract()?;
    prepare_pinned_contract(cache_dir, &contract.silero_vad_ggml)
}

fn prepare_pinned_contract(
    cache_dir: &PathBuf,
    contract: &PinnedModelContract,
) -> Result<PathBuf, TranscriptionErrorCode> {
    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir.clone())
        .with_endpoint("https://huggingface.co".into())
        .with_progress(false)
        .build()
        .map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
    let repository = api.repo(Repo::with_revision(
        contract.repository.clone(),
        RepoType::Model,
        contract.revision.clone(),
    ));
    verified_model_path(&repository, &contract.file)
}

fn verified_model_path(
    repository: &ApiRepo,
    expected: &ModelFileContract,
) -> Result<PathBuf, TranscriptionErrorCode> {
    let path = repository
        .get(&expected.path)
        .map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
    verify_cached_file(&path, expected.size, &expected.sha256)?;
    Ok(path)
}

/// Verifies a cached model file and removes it when its bytes are wrong. hf-hub serves a cached file without
/// downloading again, so a corrupt one left in place would fail every later retry on the same bytes.
fn verify_cached_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), TranscriptionErrorCode> {
    let file = File::open(path).map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
    let verified = verify_model_reader(file, expected_size, expected_sha256);
    if verified == Err(TranscriptionErrorCode::ModelIntegrity) {
        // Removing the snapshot entry makes the next `get` download the pinned revision again.
        let _ = std::fs::remove_file(path);
    }
    verified
}

fn verify_model_reader(
    mut reader: impl Read,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), TranscriptionErrorCode> {
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or(TranscriptionErrorCode::ModelIntegrity)?;
        hasher.update(&buffer[..count]);
    }
    if size != expected_size || format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err(TranscriptionErrorCode::ModelIntegrity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_model_bytes_and_rejects_size_or_digest_drift() {
        let manifest: RuntimeAssetManifest = serde_json::from_str(RUNTIME_ASSET_MANIFEST).unwrap();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.whisper_tiny_q5.file.path, "ggml-tiny-q5_1.bin");
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(verify_model_reader(&b"abc"[..], 3, digest), Ok(()));
        assert_eq!(
            verify_model_reader(&b"abc"[..], 4, digest),
            Err(TranscriptionErrorCode::ModelIntegrity)
        );
        assert_eq!(
            verify_model_reader(&b"abd"[..], 3, digest),
            Err(TranscriptionErrorCode::ModelIntegrity)
        );
    }

    #[test]
    fn a_corrupt_cached_file_is_removed_so_the_retry_downloads_it_again() {
        let path = std::env::temp_dir().join(format!("bottie-pinned-{}.bin", std::process::id()));
        std::fs::write(&path, b"abd").unwrap();
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(
            verify_cached_file(&path, 3, digest),
            Err(TranscriptionErrorCode::ModelIntegrity)
        );
        assert!(!path.exists());
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(verify_cached_file(&path, 3, digest), Ok(()));
        assert!(path.exists(), "a verified file stays cached");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            verify_cached_file(&path, 3, digest),
            Err(TranscriptionErrorCode::ModelUnavailable)
        );
    }
}
