//! Opt-in resolution and lifecycle for packaged native Python resources.
//!
//! The default package has no Python evidence marker and therefore creates no runner. A marked
//! development bundle must contain every fixed platform resource or startup fails closed. Native
//! paths remain in Rust and are never exposed through a Tauri command.

#![allow(
    dead_code,
    reason = "provider mapping into the injected runner remains intentionally deferred"
)]

use std::{ffi::OsString, fs, path::Path, path::PathBuf, sync::Arc};

#[cfg(target_os = "windows")]
use std::process::{Command, Stdio};

use tauri::{App, Manager, Runtime};

#[cfg(target_os = "linux")]
use crate::python_execution::LinuxContainedPythonRunner;
#[cfg(target_os = "macos")]
use crate::python_execution::MacosXpcPythonRunner;
use crate::python_execution::PythonRunner;
#[cfg(target_os = "windows")]
use crate::python_execution::WindowsAppContainerPythonRunner;

const EVIDENCE_FILENAME: &str = "python-runtime-evidence.json";
const RUNTIME_DIRECTORY: &str = "python-runtime";
const RUNNER_BASENAME: &str = "bottie-python-runner";
const MACOS_CLIENT_BASENAME: &str = "bottie-python-xpc-client";
const MACOS_SERVICE_BUNDLE: &str = "com.bottie.python-runner.xpc";
const MACOS_SERVICE_EXECUTABLE: &str = "bottie-python-xpc-service";
const WINDOWS_CONTROLLER_BASENAME: &str = "bottie-python-appcontainer.exe";
const WINDOWS_RUNNER_BASENAME: &str = "bottie-python-runner.exe";
const WINDOWS_PROFILE_MONIKER_PREFIX: &str = "com.bottie.python.runner";
const INCOMPLETE_BUNDLE_MESSAGE: &str = "The packaged Python runtime is incomplete.";

/// Supported packaged layouts, kept platform-independent for contract tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PythonBundlePlatform {
    /// Linux DEB layout.
    Linux,
    /// macOS application plus nested XPC service layout.
    Macos,
    /// Windows MSI application-directory layout.
    Windows,
}

/// Native-only locations required to construct one platform runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PythonBundlePaths {
    /// Linux helper and read-only runtime directory.
    Linux { runner: PathBuf, runtime: PathBuf },
    /// macOS bridge; the fixed service is resolved by the containing application bundle.
    Macos { client: PathBuf },
    /// Windows controller, contained helper, and runtime directory.
    Windows {
        controller: PathBuf,
        runner: PathBuf,
        runtime: PathBuf,
    },
}

/// Fixed path-free failure for an incomplete opt-in bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PythonRuntimeError;

impl PythonRuntimeError {
    /// Returns the fixed startup-safe explanation.
    pub(crate) fn message(&self) -> &'static str {
        INCOMPLETE_BUNDLE_MESSAGE
    }
}

/// Retains the injected runner and platform lifecycle guard for the native process.
pub(crate) struct PythonRuntimeState {
    runner: Arc<dyn PythonRunner>,
    #[cfg(target_os = "windows")]
    profile: WindowsAppContainerProfile,
}

/// Resolves and constructs a runner only for the explicitly marked development bundle.
pub(crate) fn initialize_python_runtime<R: Runtime>(
    app: &App<R>,
) -> Result<Option<PythonRuntimeState>, PythonRuntimeError> {
    let executable = std::env::current_exe().map_err(|_| PythonRuntimeError)?;
    let executable_directory = executable.parent().ok_or(PythonRuntimeError)?;
    let resource_directory = app.path().resource_dir().map_err(|_| PythonRuntimeError)?;
    let Some(platform) = current_platform() else {
        return Ok(None);
    };
    let Some(paths) =
        resolve_python_bundle_paths(platform, executable_directory, &resource_directory)?
    else {
        return Ok(None);
    };

    construct_runtime(paths).map(Some)
}

/// Resolves fixed native paths while treating a missing evidence marker as opt-out.
pub(crate) fn resolve_python_bundle_paths(
    platform: PythonBundlePlatform,
    executable_directory: &Path,
    resource_directory: &Path,
) -> Result<Option<PythonBundlePaths>, PythonRuntimeError> {
    let evidence = resource_directory.join(EVIDENCE_FILENAME);
    match fs::symlink_metadata(&evidence) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(PythonRuntimeError),
    }
    require_file(&evidence)?;

    let paths = match platform {
        PythonBundlePlatform::Linux => {
            let runner = executable_directory.join(RUNNER_BASENAME);
            let runtime = resource_directory.join(RUNTIME_DIRECTORY);
            require_file(&runner)?;
            require_directory(&runtime)?;
            PythonBundlePaths::Linux { runner, runtime }
        }
        PythonBundlePlatform::Macos => {
            let client = executable_directory.join(MACOS_CLIENT_BASENAME);
            let contents = resource_directory.parent().ok_or(PythonRuntimeError)?;
            let service = contents.join("XPCServices").join(MACOS_SERVICE_BUNDLE);
            let service_contents = service.join("Contents");
            require_file(&client)?;
            require_file(&service_contents.join("Info.plist"))?;
            require_file(
                &service_contents
                    .join("MacOS")
                    .join(MACOS_SERVICE_EXECUTABLE),
            )?;
            require_file(&service_contents.join("Helpers").join(RUNNER_BASENAME))?;
            require_file(&service_contents.join("Resources").join(EVIDENCE_FILENAME))?;
            require_directory(&service_contents.join("Resources").join(RUNTIME_DIRECTORY))?;
            PythonBundlePaths::Macos { client }
        }
        PythonBundlePlatform::Windows => {
            let controller = executable_directory.join(WINDOWS_CONTROLLER_BASENAME);
            let runner = executable_directory.join(WINDOWS_RUNNER_BASENAME);
            let runtime = resource_directory.join(RUNTIME_DIRECTORY);
            require_file(&controller)?;
            require_file(&runner)?;
            require_directory(&runtime)?;
            PythonBundlePaths::Windows {
                controller,
                runner,
                runtime,
            }
        }
    };
    Ok(Some(paths))
}

/// Requires one ordinary file and rejects directory or symlink substitution.
fn require_file(path: &Path) -> Result<(), PythonRuntimeError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PythonRuntimeError)?;
    if metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(PythonRuntimeError)
    }
}

/// Requires one ordinary directory and rejects file or symlink substitution.
fn require_directory(path: &Path) -> Result<(), PythonRuntimeError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PythonRuntimeError)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(PythonRuntimeError)
    }
}

/// Returns one controller-safe profile moniker owned by the given native process.
pub(crate) fn windows_profile_moniker(process_id: u32) -> String {
    format!("{WINDOWS_PROFILE_MONIKER_PREFIX}.{process_id}")
}

/// Returns one fixed profile lifecycle command for a native-owned moniker.
pub(crate) fn windows_profile_arguments(prepare: bool, profile_moniker: &str) -> Vec<OsString> {
    vec![
        if prepare { "prepare" } else { "cleanup" }.into(),
        profile_moniker.into(),
    ]
}

#[cfg(target_os = "linux")]
fn current_platform() -> Option<PythonBundlePlatform> {
    Some(PythonBundlePlatform::Linux)
}

#[cfg(target_os = "macos")]
fn current_platform() -> Option<PythonBundlePlatform> {
    Some(PythonBundlePlatform::Macos)
}

#[cfg(target_os = "windows")]
fn current_platform() -> Option<PythonBundlePlatform> {
    Some(PythonBundlePlatform::Windows)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn current_platform() -> Option<PythonBundlePlatform> {
    None
}

#[cfg(target_os = "linux")]
fn construct_runtime(paths: PythonBundlePaths) -> Result<PythonRuntimeState, PythonRuntimeError> {
    let PythonBundlePaths::Linux { runner, runtime } = paths else {
        return Err(PythonRuntimeError);
    };
    Ok(PythonRuntimeState {
        runner: Arc::new(LinuxContainedPythonRunner::new(runner, runtime)),
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn construct_runtime(_paths: PythonBundlePaths) -> Result<PythonRuntimeState, PythonRuntimeError> {
    Err(PythonRuntimeError)
}

#[cfg(target_os = "macos")]
fn construct_runtime(paths: PythonBundlePaths) -> Result<PythonRuntimeState, PythonRuntimeError> {
    let PythonBundlePaths::Macos { client } = paths else {
        return Err(PythonRuntimeError);
    };
    Ok(PythonRuntimeState {
        runner: Arc::new(MacosXpcPythonRunner::new(client)),
    })
}

#[cfg(target_os = "windows")]
fn construct_runtime(paths: PythonBundlePaths) -> Result<PythonRuntimeState, PythonRuntimeError> {
    let PythonBundlePaths::Windows {
        controller,
        runner,
        runtime,
    } = paths
    else {
        return Err(PythonRuntimeError);
    };
    let profile_moniker = windows_profile_moniker(std::process::id());
    let profile = WindowsAppContainerProfile::prepare(&controller, profile_moniker.clone())?;
    Ok(PythonRuntimeState {
        runner: Arc::new(WindowsAppContainerPythonRunner::new(
            controller,
            profile_moniker,
            runner,
            runtime,
        )),
        profile,
    })
}

/// Owns one process-specific Windows profile from preparation through native shutdown.
#[cfg(target_os = "windows")]
struct WindowsAppContainerProfile {
    controller: PathBuf,
    moniker: String,
}

#[cfg(target_os = "windows")]
impl WindowsAppContainerProfile {
    /// Provisions one process-specific zero-capability profile through the bundled controller.
    fn prepare(controller: &Path, moniker: String) -> Result<Self, PythonRuntimeError> {
        if run_profile_command(controller, true, &moniker)? {
            Ok(Self {
                controller: controller.to_owned(),
                moniker,
            })
        } else {
            Err(PythonRuntimeError)
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsAppContainerProfile {
    fn drop(&mut self) {
        let _ = run_profile_command(&self.controller, false, &self.moniker);
    }
}

/// Runs one fixed path-free profile lifecycle command with no inherited environment or output.
#[cfg(target_os = "windows")]
fn run_profile_command(
    controller: &Path,
    prepare: bool,
    profile_moniker: &str,
) -> Result<bool, PythonRuntimeError> {
    let status = Command::new(controller)
        .args(windows_profile_arguments(prepare, profile_moniker))
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| PythonRuntimeError)?;
    Ok(status.success())
}
