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
/// Exact public host used to resolve immutable Hugging Face model revisions.
const HUGGING_FACE_HOST: &str = "huggingface.co";
/// Path segment separating one Hugging Face repository identity from its revision.
const HUGGING_FACE_RESOLVE_SEGMENT: &str = "resolve";

/// Delivery policy selected for one immutable model source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceDelivery {
    /// The approved repository returns the exact file response directly.
    Direct,
    /// Hugging Face resolves the immutable file to one separately validated HTTPS location.
    HuggingFace,
}

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
    delivery: SourceDelivery,
    #[cfg(test)]
    allow_loopback_resolution: bool,
}

impl ModelSourcePlan {
    /// Validates one HTTPS repository root and exact one-to-one manifest source mapping.
    pub(crate) fn new(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
    ) -> Result<Self, DownloadError> {
        Self::build(
            manifest,
            repository_root,
            source_revision,
            files,
            SourceDelivery::Direct,
            false,
        )
    }

    /// Binds one immutable public Hugging Face repository to validated single-hop resolution.
    pub(crate) fn for_hugging_face(
        manifest: ModelPackageManifest,
        repository_id: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
    ) -> Result<Self, DownloadError> {
        if manifest.package_id != repository_id {
            return Err(DownloadError::InvalidPlan);
        }
        let repository_root = hugging_face_repository_root(repository_id)?;
        Self::build(
            manifest,
            repository_root.as_str(),
            source_revision,
            files,
            SourceDelivery::HuggingFace,
            false,
        )
    }

    /// Accepts literal loopback HTTP only for isolated response fixtures.
    #[cfg(test)]
    pub(crate) fn for_loopback_fixture(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
    ) -> Result<Self, DownloadError> {
        Self::build(
            manifest,
            repository_root,
            source_revision,
            files,
            SourceDelivery::Direct,
            true,
        )
    }

    /// Accepts loopback resolution only for isolated two-hop response fixtures.
    #[cfg(test)]
    pub(crate) fn for_loopback_hugging_face_fixture(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
    ) -> Result<Self, DownloadError> {
        Self::build(
            manifest,
            repository_root,
            source_revision,
            files,
            SourceDelivery::HuggingFace,
            true,
        )
    }

    fn build(
        manifest: ModelPackageManifest,
        repository_root: &str,
        source_revision: &str,
        files: Vec<ModelFileSource>,
        delivery: SourceDelivery,
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
            delivery,
            #[cfg(test)]
            allow_loopback_resolution: allow_loopback_http,
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

    /// Returns the reviewed delivery policy without exposing source URLs outside Rust.
    pub(super) fn delivery(&self) -> SourceDelivery {
        self.delivery
    }

    /// Validates a resolver-provided location before a second request is created.
    pub(crate) fn resolved_url(
        &self,
        source: &ModelFileSource,
        location: &str,
    ) -> Result<Url, DownloadError> {
        let source_url = self.source_url(source)?;
        let resolved = source_url
            .join(location)
            .map_err(|_| DownloadError::InvalidResponse)?;
        #[cfg(test)]
        if self.allow_loopback_resolution {
            if resolved.scheme() == "http"
                && resolved.host_str().is_some_and(is_loopback_host)
                && resolved.port_or_known_default() == source_url.port_or_known_default()
            {
                return Ok(resolved);
            }
        }
        if resolved.scheme() != "https"
            || !resolved.username().is_empty()
            || resolved.password().is_some()
            || resolved.fragment().is_some()
            || resolved.port().is_some()
        {
            return Err(DownloadError::InvalidResponse);
        }
        let host = resolved.host_str().ok_or(DownloadError::InvalidResponse)?;
        if host == HUGGING_FACE_HOST {
            let expected_path = format!(
                "/api/resolve-cache/models/{}/{}/{}",
                hugging_face_repository_id(&self.repository_root)?,
                self.manifest.source_revision,
                source.relative_path
            );
            if resolved.path() != expected_path {
                return Err(DownloadError::InvalidResponse);
            }
        } else if !host.ends_with(".cdn.hf.co") && !host.ends_with(".xethub.hf.co") {
            return Err(DownloadError::InvalidResponse);
        }
        Ok(resolved)
    }

    /// Derives a fixed opaque cache binding from the exact root, revision, paths, and validators.
    pub(super) fn resume_binding(&self) -> String {
        let mut hasher = Sha256::new();
        update_binding(
            &mut hasher,
            match self.delivery {
                SourceDelivery::Direct => b"direct",
                SourceDelivery::HuggingFace => b"hugging_face",
            },
        );
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

fn hugging_face_repository_root(repository_id: &str) -> Result<Url, DownloadError> {
    let Some((owner, repository)) = repository_id.split_once('/') else {
        return Err(DownloadError::InvalidPlan);
    };
    if repository.contains('/')
        || !valid_repository_segment(owner)
        || !valid_repository_segment(repository)
    {
        return Err(DownloadError::InvalidPlan);
    }
    Url::parse(&format!(
        "https://{HUGGING_FACE_HOST}/{owner}/{repository}/{HUGGING_FACE_RESOLVE_SEGMENT}/"
    ))
    .map_err(|_| DownloadError::InvalidPlan)
}

fn hugging_face_repository_id(repository_root: &Url) -> Result<String, DownloadError> {
    let segments = repository_root
        .path_segments()
        .ok_or(DownloadError::InvalidResponse)?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 3 || segments[2] != HUGGING_FACE_RESOLVE_SEGMENT {
        return Err(DownloadError::InvalidResponse);
    }
    Ok(format!("{}/{}", segments[0], segments[1]))
}

fn valid_repository_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
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
