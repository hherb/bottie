//! Bottie's one built-in embedding model: pinned download, file verification, and FastEmbed load.

use std::{env, fs::File, io::Read, path::PathBuf};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use hf_hub::{
    Cache, Repo, RepoType,
    api::sync::{ApiBuilder, ApiRepo},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::storage::SemanticEmbedder;

const EMBEDDING_RUNTIME_THREADS: usize = 2;
const RUNTIME_ASSET_MANIFEST: &str = include_str!("../../runtime-assets.json");

/// Compiled release contract for the only runtime-downloaded model.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeAssetManifest {
    schema_version: u8,
    embedding_gemma: EmbeddingGemmaContract,
}

/// Immutable repository revision, variant, and file identities for EmbeddingGemma; other manifest fields are ignored.
#[derive(Deserialize)]
struct EmbeddingGemmaContract {
    repository: String,
    revision: String,
    variant: String,
    files: Vec<ModelFileContract>,
}

/// One exact model-cache file expected by the built-in FastEmbed adapter.
#[derive(Deserialize)]
struct ModelFileContract {
    path: String,
    sha256: String,
    size: u64,
}

/// Production adapter around Bottie's one built-in FastEmbed model.
pub(crate) struct FastEmbedder {
    model: TextEmbedding,
}

impl FastEmbedder {
    /// Loads or downloads the Q4 EmbeddingGemma model through the app-owned cache directory, or
    /// through `HF_HOME` and `HF_ENDPOINT` for the pinned snapshot when those are set.
    ///
    /// Failures return path-free codes so diagnostics say which stage broke (#231):
    /// `model_contract`, `model_cache_dir`, `model_download` (client setup, fetch, or the hf-hub cache
    /// write, which share one I/O error), `model_cache_read`, `model_integrity`, `model_cache_ref`, and
    /// `model_runtime`. FastEmbed returns an untyped error whose text may hold cache paths, so
    /// `model_runtime` stays one code.
    pub(crate) fn load(cache_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&cache_dir).map_err(|_| "model_cache_dir")?;
        prepare_pinned_model_snapshot(&cache_dir)?;
        let options = TextInitOptions::new(EmbeddingModel::EmbeddingGemma300MQ4)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false)
            .with_intra_threads(EMBEDDING_RUNTIME_THREADS);
        TextEmbedding::try_new(options)
            .map(|model| Self { model })
            .map_err(|_| "model_runtime".into())
    }
}

/// Downloads only the compiled revision, verifies every file, then points FastEmbed's main cache ref at it.
fn prepare_pinned_model_snapshot(default_cache_dir: &PathBuf) -> Result<(), String> {
    let contract: RuntimeAssetManifest =
        serde_json::from_str(RUNTIME_ASSET_MANIFEST).map_err(|_| "model_contract")?;
    if contract.schema_version != 1 || contract.embedding_gemma.variant != "EmbeddingGemma300MQ4" {
        return Err("model_contract".into());
    }
    let cache_dir = env::var_os("HF_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_cache_dir.clone());
    let endpoint = env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".to_owned());
    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir.clone())
        .with_endpoint(endpoint)
        .with_progress(false)
        .build()
        .map_err(|_| "model_download")?;
    let revision = contract.embedding_gemma.revision.clone();
    let repository = contract.embedding_gemma.repository.clone();
    let pinned = api.repo(Repo::with_revision(
        repository.clone(),
        RepoType::Model,
        revision.clone(),
    ));
    verify_pinned_files(&pinned, &contract.embedding_gemma.files)?;
    Cache::new(cache_dir)
        .repo(Repo::new(repository, RepoType::Model))
        .create_ref(&revision)
        .map_err(|_| "model_cache_ref".to_owned())
}

/// Fetches and verifies the exact model files without retaining remote bodies or cache paths in errors.
fn verify_pinned_files(repo: &ApiRepo, files: &[ModelFileContract]) -> Result<(), String> {
    for expected in files {
        let path = repo.get(&expected.path).map_err(|_| "model_download")?;
        let file = File::open(path).map_err(|_| "model_cache_read")?;
        verify_model_reader(file, expected.size, &expected.sha256)?;
    }
    Ok(())
}

/// Verifies one bounded expected size and SHA-256 while streaming cache bytes.
fn verify_model_reader(
    mut reader: impl Read,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|_| "model_cache_read")?;
        if count == 0 {
            break;
        }
        size = size.checked_add(count as u64).ok_or("model_contract")?;
        hasher.update(&buffer[..count]);
    }
    if size != expected_size || format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err("model_integrity".into());
    }
    Ok(())
}

impl SemanticEmbedder for FastEmbedder {
    /// Produces normalized EmbeddingGemma vectors for versioned document inputs.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.model
            .embed(texts, None)
            .map_err(|_| "embedding_runtime".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_model_contract_is_complete_and_reader_verification_fails_closed() {
        let manifest: RuntimeAssetManifest = serde_json::from_str(RUNTIME_ASSET_MANIFEST)
            .expect("compiled release contract should parse");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.embedding_gemma.files.len(), 6);
        assert!(
            manifest
                .embedding_gemma
                .revision
                .chars()
                .all(|value| value.is_ascii_hexdigit())
        );

        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(verify_model_reader(&b"abc"[..], 3, digest), Ok(()));
        assert_eq!(
            verify_model_reader(&b"abc"[..], 4, digest),
            Err("model_integrity".into())
        );
        assert_eq!(
            verify_model_reader(&b"abd"[..], 3, digest),
            Err("model_integrity".into())
        );
    }

    #[test]
    fn unreadable_model_file_reports_a_read_code_not_a_generic_cache_code() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk error"))
            }
        }

        assert_eq!(
            verify_model_reader(FailingReader, 3, "00"),
            Err("model_cache_read".into())
        );
    }
}
