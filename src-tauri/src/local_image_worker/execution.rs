//! Verified, network-denied execution through one reusable private local-image worker.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use tokio::sync::{Mutex, watch};

#[path = "execution/output.rs"]
mod output;

pub(crate) use output::LocalGeneratedOutput;
use output::{
    cleanup_output_directory, create_output_directory, prepare_empty_output_directory,
    validate_absolute_path, validate_outputs,
};

use super::{
    manager::{ManagerEvent, WorkerReadiness},
    model_acquisition::ModelPackageManifest,
    model_cache::begin_cached_model_load,
    transport::{TransportTimeouts, WorkerProcessSpec, WorkerTransport},
};

/// Exact fixed product profile accepted by the first local Qwen Image runtime proof.
pub(crate) const LOCAL_GENERATION_DIMENSIONS: (u32, u32) = (512, 512);
/// The accepted first local runtime produces one image per operation.
pub(crate) const LOCAL_GENERATION_OUTPUT_COUNT: u8 = 1;

const NETWORKLESS_PROFILE: &str = "(version 1) (allow default) (deny network*)";
const SANDBOX_EXECUTABLE: &str = "/usr/bin/sandbox-exec";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// Stable path-free failures from the verified local execution boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImageExecutionError {
    /// An application-owned native directory was relative, traversing, or unsafe.
    InvalidLayout,
    /// This build target cannot apply the accepted runtime's operating-system sandbox.
    UnsupportedPlatform,
    /// The exact worker could not start or negotiate the private protocol.
    WorkerUnavailable,
    /// The worker's negotiated runtime identity differed from the verified package.
    RuntimeMismatch,
    /// The promoted model changed or could not be loaded at the worker boundary.
    ModelUnavailable,
    /// The worker rejected or failed one bounded generation.
    GenerationFailed,
    /// The operation cooperatively cancelled or its forced teardown completed.
    Cancelled,
    /// Worker-declared output bytes or filesystem shape failed closed.
    InvalidOutput,
    /// A formerly warm worker could not complete bounded shutdown.
    ShutdownFailed,
}

/// Fresh native capability produced only after the availability service re-verifies every gate.
#[derive(Debug)]
pub(crate) struct VerifiedLocalImageInstallation {
    worker_executable: PathBuf,
    model_cache_root: PathBuf,
    manifest: ModelPackageManifest,
}

impl VerifiedLocalImageInstallation {
    /// Creates a verified installation token for path-backed adapter tests only.
    #[cfg(test)]
    pub(crate) fn for_test(
        worker_executable: PathBuf,
        model_cache_root: PathBuf,
        manifest: ModelPackageManifest,
    ) -> Self {
        Self {
            worker_executable,
            model_cache_root,
            manifest,
        }
    }

    /// Creates a fresh token after the sibling availability service has completed verification.
    pub(super) fn verified(
        worker_executable: PathBuf,
        model_cache_root: PathBuf,
        manifest: ModelPackageManifest,
    ) -> Self {
        Self {
            worker_executable,
            model_cache_root,
            manifest,
        }
    }

    fn key(&self) -> InstallationKey {
        InstallationKey {
            worker_executable: self.worker_executable.clone(),
            model_cache_root: self.model_cache_root.clone(),
            model_id: self.manifest.model_id.clone(),
            model_revision: self.manifest.source_revision.clone(),
            runtime_id: self.manifest.runtime_id.clone(),
        }
    }

    /// Returns the exact verified local model identity without exposing native locations.
    pub(crate) fn model_id(&self) -> &str {
        &self.manifest.model_id
    }
}

/// One exact native-only local text-to-image request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalGenerationRequest {
    prompt: String,
    width: u32,
    height: u32,
    count: u8,
    seed: u64,
}

impl LocalGenerationRequest {
    /// Accepts only the measured first-runtime generation shape and a non-empty bounded prompt.
    pub(crate) fn new(
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
        seed: u64,
    ) -> Result<Self, LocalImageExecutionError> {
        let prompt = prompt.into();
        if prompt.trim().is_empty()
            || prompt.chars().count() > 1_000
            || (width, height) != LOCAL_GENERATION_DIMENSIONS
            || count != LOCAL_GENERATION_OUTPUT_COUNT
        {
            return Err(LocalImageExecutionError::GenerationFailed);
        }
        Ok(Self {
            prompt,
            width,
            height,
            count,
            seed,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InstallationKey {
    worker_executable: PathBuf,
    model_cache_root: PathBuf,
    model_id: String,
    model_revision: String,
    runtime_id: String,
}

enum WorkerLauncher {
    Production,
    #[cfg(test)]
    Fixture {
        executable: PathBuf,
        mode: String,
    },
}

struct WorkerSession {
    key: InstallationKey,
    output_directory: PathBuf,
    transport: WorkerTransport,
}

/// Serialized owner of one warm, private local-image worker process.
pub(crate) struct LocalImageWorkerRuntime {
    output_parent: PathBuf,
    launcher: WorkerLauncher,
    session: Mutex<Option<WorkerSession>>,
    worker_starts: AtomicUsize,
}

impl LocalImageWorkerRuntime {
    /// Creates a lazy production runtime without touching native storage or starting a process.
    pub(crate) fn new(output_parent: PathBuf) -> Result<Self, LocalImageExecutionError> {
        validate_absolute_path(&output_parent)?;
        Ok(Self {
            output_parent,
            launcher: WorkerLauncher::Production,
            session: Mutex::new(None),
            worker_starts: AtomicUsize::new(0),
        })
    }

    /// Creates a direct fixture runtime whose output directory is appended to fixed arguments.
    #[cfg(test)]
    pub(crate) fn for_fixture(
        output_parent: PathBuf,
        executable: PathBuf,
        mode: impl Into<String>,
    ) -> Result<Self, LocalImageExecutionError> {
        validate_absolute_path(&output_parent)?;
        validate_absolute_path(&executable)?;
        Ok(Self {
            output_parent,
            launcher: WorkerLauncher::Fixture {
                executable,
                mode: mode.into(),
            },
            session: Mutex::new(None),
            worker_starts: AtomicUsize::new(0),
        })
    }

    /// Generates one bounded result only from a freshly verified installation capability.
    pub(crate) async fn generate(
        &self,
        installation: VerifiedLocalImageInstallation,
        request: LocalGenerationRequest,
        mut cancellation: watch::Receiver<bool>,
        mut on_progress: impl FnMut(),
    ) -> Result<Vec<LocalGeneratedOutput>, LocalImageExecutionError> {
        if *cancellation.borrow() {
            return Err(LocalImageExecutionError::Cancelled);
        }
        let key = installation.key();
        let mut owned = self.session.lock().await;
        self.replace_mismatched_session(&mut owned, &key).await?;
        if owned.is_none() {
            *owned = Some(self.start_session(&installation, key).await?);
        }
        let output_directory = owned
            .as_ref()
            .expect("session was started")
            .output_directory
            .clone();
        if let Err(error) = prepare_empty_output_directory(&output_directory) {
            let failed = owned.take().expect("unsafe output has an owned session");
            let _ = failed.transport.shutdown().await;
            return Err(error);
        }
        let session = owned.as_mut().expect("session was started");
        if session.transport.readiness() != WorkerReadiness::Loaded {
            let load_id = uuid::Uuid::new_v4().to_string();
            begin_cached_model_load(
                &mut session.transport,
                load_id,
                &installation.model_cache_root,
                installation.manifest.clone(),
            )
            .await
            .map_err(|_| LocalImageExecutionError::ModelUnavailable)?;
            match wait_for_terminal(&mut session.transport, &mut cancellation, &mut on_progress)
                .await?
            {
                ManagerEvent::ModelLoaded => {}
                ManagerEvent::Cancelled => return Err(LocalImageExecutionError::Cancelled),
                _ => return Err(LocalImageExecutionError::ModelUnavailable),
            }
        }
        if *cancellation.borrow() {
            return Err(LocalImageExecutionError::Cancelled);
        }
        session
            .transport
            .begin_generation(
                uuid::Uuid::new_v4().to_string(),
                request.prompt,
                request.width,
                request.height,
                request.count,
                Some(request.seed),
            )
            .await
            .map_err(|_| LocalImageExecutionError::GenerationFailed)?;
        let terminal =
            wait_for_terminal(&mut session.transport, &mut cancellation, &mut on_progress).await?;
        let ManagerEvent::Generated(outputs) = terminal else {
            cleanup_output_directory(&session.output_directory);
            return match terminal {
                ManagerEvent::Cancelled => Err(LocalImageExecutionError::Cancelled),
                _ => Err(LocalImageExecutionError::GenerationFailed),
            };
        };
        match validate_outputs(&session.output_directory, outputs) {
            Ok(outputs) => Ok(outputs),
            Err(error) => {
                cleanup_output_directory(&session.output_directory);
                let failed = owned.take().expect("invalid output has an owned session");
                let _ = failed.transport.shutdown().await;
                Err(error)
            }
        }
    }

    /// Shuts down and reaps the warm worker, if one was started.
    pub(crate) async fn shutdown(&self) -> Result<(), LocalImageExecutionError> {
        let session = self.session.lock().await.take();
        let Some(session) = session else {
            return Ok(());
        };
        let output_directory = session.output_directory.clone();
        let result = session
            .transport
            .shutdown()
            .await
            .map_err(|_| LocalImageExecutionError::ShutdownFailed);
        cleanup_output_directory(&output_directory);
        let _ = fs::remove_dir(&output_directory);
        result
    }

    #[cfg(test)]
    /// Returns the number of worker processes started by this runtime.
    pub(crate) async fn worker_start_count_for_test(&self) -> usize {
        self.worker_starts.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    /// Returns whether the fixture worker remains loaded after a completed operation.
    pub(crate) async fn has_loaded_worker_for_test(&self) -> bool {
        self.session
            .lock()
            .await
            .as_ref()
            .is_some_and(|session| session.transport.readiness() == WorkerReadiness::Loaded)
    }

    async fn replace_mismatched_session(
        &self,
        owned: &mut Option<WorkerSession>,
        key: &InstallationKey,
    ) -> Result<(), LocalImageExecutionError> {
        let replace = owned.as_ref().is_some_and(|session| {
            session.key != *key || session.transport.readiness() == WorkerReadiness::Stopped
        });
        if !replace {
            return Ok(());
        }
        let session = owned
            .take()
            .expect("replacement checked an existing session");
        let output_directory = session.output_directory.clone();
        if session.transport.readiness() != WorkerReadiness::Stopped {
            session
                .transport
                .shutdown()
                .await
                .map_err(|_| LocalImageExecutionError::ShutdownFailed)?;
        }
        cleanup_output_directory(&output_directory);
        let _ = fs::remove_dir(output_directory);
        Ok(())
    }

    async fn start_session(
        &self,
        installation: &VerifiedLocalImageInstallation,
        key: InstallationKey,
    ) -> Result<WorkerSession, LocalImageExecutionError> {
        let output_directory = create_output_directory(&self.output_parent)?;
        let spec = self.process_spec(installation, &output_directory)?;
        let timeouts = TransportTimeouts::new(
            HANDSHAKE_TIMEOUT,
            EVENT_TIMEOUT,
            WRITE_TIMEOUT,
            SHUTDOWN_TIMEOUT,
        );
        let transport =
            match WorkerTransport::spawn_with_timeouts(spec, env!("CARGO_PKG_VERSION"), timeouts)
                .await
            {
                Ok(transport) => transport,
                Err(_) => {
                    let _ = fs::remove_dir(&output_directory);
                    return Err(LocalImageExecutionError::WorkerUnavailable);
                }
            };
        self.worker_starts.fetch_add(1, Ordering::Relaxed);
        if transport.runtime_id() != Some(installation.manifest.runtime_id.as_str()) {
            let _ = transport.shutdown().await;
            let _ = fs::remove_dir(&output_directory);
            return Err(LocalImageExecutionError::RuntimeMismatch);
        }
        Ok(WorkerSession {
            key,
            output_directory,
            transport,
        })
    }

    fn process_spec(
        &self,
        installation: &VerifiedLocalImageInstallation,
        output_directory: &Path,
    ) -> Result<WorkerProcessSpec, LocalImageExecutionError> {
        match &self.launcher {
            WorkerLauncher::Production => production_process_spec(installation, output_directory),
            #[cfg(test)]
            WorkerLauncher::Fixture { executable, mode } => Ok(WorkerProcessSpec::new(
                executable.clone(),
                vec![
                    OsString::from(mode),
                    output_directory.as_os_str().to_owned(),
                ],
            )),
        }
    }
}

async fn wait_for_terminal(
    transport: &mut WorkerTransport,
    cancellation: &mut watch::Receiver<bool>,
    on_progress: &mut impl FnMut(),
) -> Result<ManagerEvent, LocalImageExecutionError> {
    let mut cancellation_open = true;
    let mut cancellation_sent = false;
    loop {
        if *cancellation.borrow() && !cancellation_sent {
            transport
                .cancel_active()
                .await
                .map_err(|_| LocalImageExecutionError::Cancelled)?;
            cancellation_sent = true;
        }
        tokio::select! {
            event = transport.next_event() => {
                let event = event.map_err(|_| {
                    if cancellation_sent {
                        LocalImageExecutionError::Cancelled
                    } else {
                        LocalImageExecutionError::WorkerUnavailable
                    }
                })?;
                if event == ManagerEvent::Progress {
                    on_progress();
                } else {
                    return Ok(event);
                }
            }
            changed = cancellation.changed(), if cancellation_open && !cancellation_sent => {
                cancellation_open = changed.is_ok();
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn production_process_spec(
    installation: &VerifiedLocalImageInstallation,
    output_directory: &Path,
) -> Result<WorkerProcessSpec, LocalImageExecutionError> {
    validate_absolute_path(&installation.worker_executable)?;
    Ok(WorkerProcessSpec::new(
        PathBuf::from(SANDBOX_EXECUTABLE),
        vec![
            OsString::from("-p"),
            OsString::from(NETWORKLESS_PROFILE),
            installation.worker_executable.as_os_str().to_owned(),
            output_directory.as_os_str().to_owned(),
        ],
    ))
}

#[cfg(not(target_os = "macos"))]
fn production_process_spec(
    _installation: &VerifiedLocalImageInstallation,
    _output_directory: &Path,
) -> Result<WorkerProcessSpec, LocalImageExecutionError> {
    Err(LocalImageExecutionError::UnsupportedPlatform)
}
