//! Exact repository-root, immutable-revision, path, and validator source bindings.

use std::collections::HashSet;

use sha2::{Digest, Sha256};
use url::Url;

use super::DownloadError;
use crate::local_image_worker::model_acquisition::{ModelAcquisition, ModelPackageManifest};

/// Maximum approved repository-root URL length.
const MAX_REPOSITORY_ROOT_BYTES: usize = 2_048;
/// Maximum retained strong entity tag length.
const MAX_STRONG_ETAG_BYTES: usize = 512;

/// One exact source mapping whose validator was reviewed with the immutable manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelFileSource {
    /// Exact portable manifest path appended beneath the repository revision.
    pub(crate) relative_path: String,
    /// Exact strong HTTP entity tag required for full and resumed responses.
    pub(crate) strong_etag: String,
}

/// Rust-owned package source plan bound to one approved repository root and immutable revision.
#[derive(Clone, Debug)]
pub(crate) struct ModelSourcePlan {
    pub(super) manifest: ModelPackageManifest,
    repository_root: Url,
    pub(super) files: Vec<ModelFileSource>,
}

impl ModelSourcePlan {
    /// Validates one HTTPS repository root and exact one-to-one manifest source mapping.
    pub(crate) fn new(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
    ) -> Result<Self, DownloadError> {
        Self::build(manifest, repository_root, source_revision, files, false)
    }

    /// Accepts literal loopback HTTP only for isolated response fixtures.
    #[cfg(test)]
    pub(crate) fn for_loopback_fixture(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
    ) -> Result<Self, DownloadError> {
        Self::build(manifest, repository_root, source_revision, files, true)
    }

    fn build(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
        allow_loopback_http: bool,
    ) -> Result<Self, DownloadError> {
        ModelAcquisition::new(manifest.clone()).map_err(|_| DownloadError::InvalidPlan)?;
        if source_revision != manifest.source_revision {
            return Err(DownloadError::InvalidPlan);
        }
        let repository_root = validate_repository_root(repository_root, allow_loopback_http)?;
        if files.len() != manifest.files.len() {
            return Err(DownloadError::InvalidPlan);
        }
        let mut paths = HashSet::new();
        for (source, contract) in files.iter().zip(&manifest.files) {
            if source.relative_path != contract.relative_path
                || !paths.insert(source.relative_path.as_str())
                || !valid_strong_etag(&source.strong_etag)
            {
                return Err(DownloadError::InvalidPlan);
            }
        }
        let plan = Self {
            manifest,
            repository_root,
            files,
        };
        for source in &plan.files {
            plan.source_url(source)?;
        }
        Ok(plan)
    }

    /// Returns the exact native manifest without exposing its paths or hashes across IPC.
    pub(crate) fn manifest(&self) -> &ModelPackageManifest {
        &self.manifest
    }

    pub(super) fn source_url(&self, source: &ModelFileSource) -> Result<Url, DownloadError> {
        self.repository_root
            .join(&format!(
                "{}/{}",
                self.manifest.source_revision, source.relative_path
            ))
            .map_err(|_| DownloadError::InvalidPlan)
    }

    /// Derives a fixed opaque cache binding from the exact root, revision, paths, and validators.
    pub(super) fn resume_binding(&self) -> String {
        let mut hasher = Sha256::new();
        update_binding(&mut hasher, self.repository_root.as_str().as_bytes());
        update_binding(&mut hasher, self.manifest.source_revision.as_bytes());
        for source in &self.files {
            update_binding(&mut hasher, source.relative_path.as_bytes());
            update_binding(&mut hasher, source.strong_etag.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Returns the opaque binding for direct path-backed cache fixture setup.
    #[cfg(test)]
    pub(crate) fn resume_binding_for_test(&self) -> String {
        self.resume_binding()
    }
}

fn update_binding(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn validate_repository_root(root: &str, allow_loopback_http: bool) -> Result<Url, DownloadError> {
    if root.len() > MAX_REPOSITORY_ROOT_BYTES {
        return Err(DownloadError::InvalidPlan);
    }
    let parsed = Url::parse(root).map_err(|_| DownloadError::InvalidPlan)?;
    let allowed_scheme = parsed.scheme() == "https"
        || (allow_loopback_http
            && parsed.scheme() == "http"
            && parsed.host_str().is_some_and(is_loopback_host));
    if !allowed_scheme
        || parsed.cannot_be_a_base()
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.path().ends_with('/')
        || root.contains('%')
    {
        return Err(DownloadError::InvalidPlan);
    }
    Ok(parsed)
}

fn valid_strong_etag(value: &str) -> bool {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return false;
    };
    !inner.is_empty()
        && value.len() <= MAX_STRONG_ETAG_BYTES
        && inner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}
