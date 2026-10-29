//! Native-only registration of exact model snapshots and explicitly built runtime artifacts.
use super::{MossError, is_moss_model};
use crate::local_image_worker::{
    availability::LocalImageAvailability, availability_service::LocalImageAvailabilityMetadata,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const BUFFER_BYTES: usize = 1024 * 1024;
const LOCK: &str = include_str!("../../../../scripts/moss-image/models.lock.json");

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    runtime: String,
    model_id: String,
    revision: String,
    executable: PathBuf,
    model_directory: PathBuf,
    executable_sha256: String,
    artifacts: Vec<Artifact>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct ModelLock {
    models: Vec<Model>,
}
#[derive(Clone, Debug, Deserialize)]
struct Model {
    id: String,
    revision: String,
    runtime: String,
    files: Vec<Artifact>,
}

/// Native capability bound to an exact compiled-in model inventory and deliberate local registration.
#[derive(Clone, Debug)]
pub(crate) struct MossInstallation {
    config: Configuration,
    model: Model,
    worker_root: PathBuf,
    identity: String,
}

impl MossInstallation {
    /// Missing config permits the previous local routes; invalid configured MOSS fails closed.
    pub(crate) fn inspect(app_data: &Path) -> Result<Option<Self>, MossError> {
        let root = app_data.join("moss-image");
        let path = root.join("runtime.json");
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(MossError::Unavailable),
        };
        if metadata.len() > MAX_CONFIG_BYTES {
            return Err(MossError::Unavailable);
        }
        let config: Configuration =
            serde_json::from_slice(&fs::read(path).map_err(|_| MossError::Unavailable)?)
                .map_err(|_| MossError::Unavailable)?;
        let lock: ModelLock = serde_json::from_str(LOCK).map_err(|_| MossError::Unavailable)?;
        let model = lock
            .models
            .into_iter()
            .find(|m| {
                m.id == config.model_id
                    && m.revision == config.revision
                    && m.runtime == config.runtime
            })
            .ok_or(MossError::Unavailable)?;
        if !is_moss_model(&config.model_id)
            || !config.executable.is_absolute()
            || !config.executable.is_file()
            || !config.model_directory.is_absolute()
            || !config.model_directory.is_dir()
            || !platform_matches(&config.runtime)
        {
            return Err(MossError::Unavailable);
        }
        verify_file(&config.executable, None, &config.executable_sha256)?;
        let parent = config.executable.parent().ok_or(MossError::Unavailable)?;
        for file in &config.artifacts {
            verify_artifact(parent, file, true)?;
        }
        for file in &model.files {
            verify_artifact(&config.model_directory, file, false)?;
        }
        let identity = format!("{}@{}", model.id, model.revision);
        Ok(Some(Self {
            config,
            model,
            identity,
            worker_root: root.join("workers"),
        }))
    }
    /// Exact repository identity is retained in durable local provenance.
    pub(crate) fn model_id(&self) -> &str {
        &self.model.id
    }
    /// Persist the immutable revision as part of generated-image model identity.
    pub(crate) fn execution_id(&self) -> &str {
        &self.identity
    }
    /// New requests bind the disclosed package; durable retries additionally require its exact revision.
    pub(crate) fn matches_model(&self, id: &str) -> bool {
        id == self.model.id || id == self.identity
    }
    /// Readiness discloses configured assets, without inventing measured peak memory or model paths.
    pub(crate) fn metadata(&self) -> LocalImageAvailabilityMetadata {
        LocalImageAvailabilityMetadata {
            model_id: self.model.id.clone(),
            package_id: self.model.id.clone(),
            runtime_id: format!("moss-image-{}@external", self.config.runtime),
            license: "Apache-2.0".into(),
            source_revision: self.model.revision.clone(),
            expected_disk_bytes: self.model.files.iter().map(|f| f.size).sum(),
            worker_expected_disk_bytes: 0,
            required_memory_bytes: 0,
            availability: LocalImageAvailability::Ready,
        }
    }
    /// Hash every weight and local pipeline byte before an inference process is permitted to load them.
    pub(super) fn verify_assets(
        &self,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), MossError> {
        for file in &self.model.files {
            verify_file_checked(
                &self.config.model_directory.join(&file.path),
                Some(file.size),
                &file.sha256,
                || *cancel.borrow(),
            )?;
        }
        verify_file(
            &self.config.executable,
            None,
            &self.config.executable_sha256,
        )
    }
    /// Launch only registered native code and compiled-in CUDA glue; prompts travel on private stdin.
    pub(super) fn command(&self) -> Result<tokio::process::Command, MossError> {
        let mut command = tokio::process::Command::new(&self.config.executable);
        if self.config.runtime == "cuda" {
            let source = include_str!("../../../../scripts/moss-image/cuda_worker.py");
            let digest = format!("{:x}", Sha256::digest(source.as_bytes()));
            fs::create_dir_all(&self.worker_root).map_err(|_| MossError::Unavailable)?;
            let script = self.worker_root.join(format!("cuda-{digest}.py"));
            if fs::read(&script).ok().as_deref() != Some(source.as_bytes()) {
                let temporary = self
                    .worker_root
                    .join(format!("{}.partial", uuid::Uuid::new_v4()));
                fs::write(&temporary, source).map_err(|_| MossError::Unavailable)?;
                fs::rename(temporary, &script).map_err(|_| MossError::Unavailable)?;
            }
            command.args(["-I", "-u"]).arg(script);
        }
        crate::worker_env::apply_minimal_environment(&mut command)
            .arg("--model-directory")
            .arg(&self.config.model_directory)
            .arg("--model-id")
            .arg(&self.model.id)
            .arg("--revision")
            .arg(&self.model.revision)
            .current_dir(
                self.config
                    .executable
                    .parent()
                    .ok_or(MossError::Unavailable)?,
            )
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1");
        Ok(command)
    }
    /// Handshake and output must identify this exact immutable model revision.
    pub(super) fn revision(&self) -> &str {
        &self.model.revision
    }
    /// Require the selected hardware engine in the worker handshake.
    pub(super) fn runtime(&self) -> &str {
        &self.config.runtime
    }
}

/// The Swift port requires macOS 26+ and Metal; other supported hosts must deliberately provision CUDA.
fn platform_matches(runtime: &str) -> bool {
    match runtime {
        "mlx" if cfg!(all(target_os = "macos", target_arch = "aarch64")) => {
            std::process::Command::new("/usr/bin/sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.trim().split('.').next()?.parse::<u32>().ok())
                .is_some_and(|v| v >= 26)
        }
        "cuda" => cfg!(any(target_os = "linux", target_os = "windows")),
        _ => false,
    }
}
/// Relative inventories cannot traverse outside their deliberately registered native roots.
fn verify_artifact(root: &Path, artifact: &Artifact, hash: bool) -> Result<(), MossError> {
    let path = Path::new(&artifact.path);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(MossError::Unavailable);
    }
    let path = root.join(path);
    if hash {
        verify_file(&path, Some(artifact.size), &artifact.sha256)
    } else if fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == artifact.size) {
        Ok(())
    } else {
        Err(MossError::Unavailable)
    }
}
/// Stream large safetensors through a fixed buffer instead of allocating model-sized verification memory.
fn verify_file(path: &Path, size: Option<u64>, digest: &str) -> Result<(), MossError> {
    verify_file_checked(path, size, digest, || false)
}

/// Cooperatively stop large read-only hashing work before allowing the next process-wide run.
fn verify_file_checked(
    path: &Path,
    size: Option<u64>,
    digest: &str,
    cancel: impl Fn() -> bool,
) -> Result<(), MossError> {
    if cancel() {
        return Err(MossError::Cancelled);
    }
    let mut file = fs::File::open(path).map_err(|_| MossError::Unavailable)?;
    if size.is_some_and(|s| file.metadata().map_or(true, |m| m.len() != s)) {
        return Err(MossError::Unavailable);
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; BUFFER_BYTES];
    loop {
        if cancel() {
            return Err(MossError::Cancelled);
        }
        let count = file.read(&mut buffer).map_err(|_| MossError::Unavailable)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if format!("{:x}", hasher.finalize()) != digest {
        return Err(MossError::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn models_have_complete_unique_safe_pinned_inventories() {
        let lock: ModelLock = serde_json::from_str(LOCK).unwrap();
        assert_eq!(lock.models.len(), 4);
        for model in lock.models {
            assert!(super::super::is_moss_model(&model.id));
            assert_eq!(model.revision.len(), 40);
            let mut names = std::collections::HashSet::new();
            for file in &model.files {
                assert!(names.insert(file.path.to_lowercase()));
                assert!(
                    Path::new(&file.path)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
                );
                assert!(file.size > 0);
                assert_eq!(file.sha256.len(), 64);
            }
            for name in [
                "LICENSE",
                "model_index.json",
                "text_encoder/config.json",
                "transformer/config.json",
                "vae/config.json",
            ] {
                assert!(names.contains(&name.to_lowercase()));
            }
            if model.runtime == "cuda" {
                assert!(names.contains("pipeline_moss_image.py"));
            }
        }
    }
    #[test]
    fn same_length_weight_mutation_and_parent_traversal_are_rejected() {
        let root = std::env::temp_dir().join(format!("bottie-moss-hash-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let original = b"original";
        let entry = Artifact {
            path: "weight".into(),
            size: 8,
            sha256: format!("{:x}", Sha256::digest(original)),
        };
        std::fs::write(root.join("weight"), original).unwrap();
        verify_artifact(&root, &entry, true).unwrap();
        std::fs::write(root.join("weight"), b"modified").unwrap();
        assert!(verify_artifact(&root, &entry, true).is_err());
        assert!(
            verify_artifact(
                &root,
                &Artifact {
                    path: "../weight".into(),
                    ..entry
                },
                false
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    #[test]
    fn persisted_moss_identity_rejects_a_different_weight_revision() {
        let model = serde_json::from_str::<ModelLock>(LOCK)
            .unwrap()
            .models
            .remove(0);
        let identity = format!("{}@{}", model.id, model.revision);
        let worker = MossInstallation {
            config: Configuration {
                runtime: model.runtime.clone(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                executable: PathBuf::from("/not-executed"),
                model_directory: PathBuf::from("/unused"),
                executable_sha256: String::new(),
                artifacts: Vec::new(),
            },
            model,
            identity: identity.clone(),
            worker_root: PathBuf::new(),
        };
        assert_eq!(worker.execution_id(), identity);
        assert!(worker.matches_model(&identity));
        assert!(worker.matches_model(worker.model_id()));
        assert!(!worker.matches_model(&format!("{}@{}", worker.model_id(), "0".repeat(40))));
    }
}
