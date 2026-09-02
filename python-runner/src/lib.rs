//! A narrow host for executing bounded Python inside CPython/WASI.
//!
//! The guest receives no inherited environment, stdin, sockets, or writable host filesystem. Each
//! request receives read-only `/work` and `/runtime` directories.

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use wasmtime::{
    Config, Engine, Error as WasmtimeError, Linker, Module, Store, StoreLimits, StoreLimitsBuilder,
};
use wasmtime_wasi::{
    DirPerms, FilePerms, I32Exit, WasiCtxBuilder,
    p1::{self, WasiP1Ctx},
    p2::pipe::MemoryOutputPipe,
};

/// Maximum UTF-8 size accepted for one generated Python program.
pub const MAX_CODE_BYTES: usize = 32 * 1_024;
/// Maximum Unicode scalar count accepted for the user-visible execution purpose.
pub const MAX_PURPOSE_CHARACTERS: usize = 512;
/// Maximum JSON request size accepted by the helper process.
pub const MAX_REQUEST_BYTES: u64 = 256 * 1_024;
/// Maximum bytes retained independently from stdout and stderr.
pub const OUTPUT_LIMIT_BYTES: usize = 32 * 1_024;
/// Maximum linear memory available to the CPython WebAssembly instance.
pub const MEMORY_LIMIT_BYTES: usize = 256 * 1_024 * 1_024;
/// Wall-clock execution deadline for one program, including interpreter startup.
pub const EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

const MAX_WASM_STACK_BYTES: usize = 2 * 1_024 * 1_024;
const MAX_TABLE_ELEMENTS: usize = 10_000;
const MAX_RANDOM_REQUEST_BYTES: u64 = 1_024 * 1_024;
const GUEST_RUNTIME: &str = "/runtime";
const GUEST_WORK: &str = "/work";
const GUEST_SCRIPT: &str = "/work/main.py";

/// One model-proposed execution request after it crosses Bottie's native boundary.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonExecutionRequest {
    /// Python source to execute without interpolation into a shell command.
    pub code: String,
    /// Short user-visible reason for requesting code execution.
    pub purpose: String,
}

impl PythonExecutionRequest {
    /// Rejects empty, oversized, or NUL-containing model output before creating a sandbox.
    pub fn validate(&self) -> Result<()> {
        if self.code.trim().is_empty() {
            return Err(anyhow!("Python code must not be empty."));
        }
        if self.code.len() > MAX_CODE_BYTES {
            return Err(anyhow!(
                "Python code exceeds the {MAX_CODE_BYTES}-byte limit."
            ));
        }
        if self.code.contains('\0') {
            return Err(anyhow!("Python code must not contain NUL characters."));
        }

        let purpose_characters = self.purpose.chars().count();
        if self.purpose.trim().is_empty() || purpose_characters > MAX_PURPOSE_CHARACTERS {
            return Err(anyhow!(
                "Execution purpose must contain 1 to {MAX_PURPOSE_CHARACTERS} characters."
            ));
        }
        if self.purpose.contains('\0') {
            return Err(anyhow!(
                "Execution purpose must not contain NUL characters."
            ));
        }
        Ok(())
    }
}

/// Stable native result classification; internal traps and host paths never cross this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Ok,
    PythonError,
    TimedOut,
    OutputLimit,
    ResourceLimit,
    InvalidRequest,
    InternalError,
}

/// Bounded output returned by the helper process.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonExecutionResult {
    pub status: ExecutionStatus,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

impl PythonExecutionResult {
    /// Produces a fixed failure without exposing internal errors or filesystem locations.
    pub fn fixed_failure(status: ExecutionStatus, message: &str) -> Self {
        Self {
            status,
            stdout: String::new(),
            stderr: message.to_owned(),
            duration_ms: 0,
        }
    }
}

struct RunnerState {
    limits: StoreLimits,
    wasi: WasiP1Ctx,
}

/// Compiled CPython/WASI runtime reusable across sequential helper requests.
pub struct PythonSandbox {
    engine: Engine,
    module: Module,
    runtime_directory: std::path::PathBuf,
}

impl PythonSandbox {
    /// Loads the pinned CPython/WASI module from a native-owned runtime directory.
    pub fn load(runtime_directory: &Path) -> Result<Self> {
        validate_runtime(runtime_directory)?;

        let mut config = Config::new();
        config.target("pulley64")?;
        config.epoch_interruption(true);
        config.max_wasm_stack(MAX_WASM_STACK_BYTES);
        let engine = Engine::new(&config)?;
        let module = Module::from_file(&engine, runtime_directory.join("python.wasm"))
            .map_err(|_| anyhow!("could not compile the CPython/WASI module"))?;

        Ok(Self {
            engine,
            module,
            runtime_directory: runtime_directory.to_owned(),
        })
    }

    /// Executes one validated program with no inherited ambient host capabilities.
    pub fn execute(&self, request: &PythonExecutionRequest) -> Result<PythonExecutionResult> {
        request.validate()?;
        let workspace = TempDir::new().context("could not create the isolated workspace")?;
        std::fs::write(workspace.path().join("main.py"), &request.code)
            .context("could not stage the Python program")?;

        self.execute_workspace(workspace.path())
    }

    fn execute_workspace(&self, workspace: &Path) -> Result<PythonExecutionResult> {
        let stdout = MemoryOutputPipe::new(OUTPUT_LIMIT_BYTES);
        let stderr = MemoryOutputPipe::new(OUTPUT_LIMIT_BYTES);
        let wasi = build_wasi_context(
            &self.runtime_directory,
            workspace,
            stdout.clone(),
            stderr.clone(),
        )?;
        let state = RunnerState {
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY_LIMIT_BYTES)
                .instances(1)
                .memories(1)
                .tables(2)
                .table_elements(MAX_TABLE_ELEMENTS)
                .trap_on_grow_failure(true)
                .build(),
            wasi,
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.set_epoch_deadline(1);

        let mut linker = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut RunnerState| &mut state.wasi)?;
        let instance = linker.instantiate(&mut store, &self.module)?;
        let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

        let timed_out = Arc::new(AtomicBool::new(false));
        let timer = start_deadline(self.engine.clone(), Arc::clone(&timed_out));
        let started = Instant::now();
        let execution = start.call(&mut store, ());
        timer.stop();

        let stdout = stdout.contents();
        let stderr = stderr.contents();
        let status = classify_execution(
            execution.as_ref().err(),
            timed_out.load(Ordering::Acquire),
            stdout.len(),
            stderr.len(),
        );
        Ok(PythonExecutionResult {
            status,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        })
    }
}

fn validate_runtime(runtime_directory: &Path) -> Result<()> {
    for required_path in ["python.wasm", "lib/python3.14/os.py", "LICENSE"] {
        if !runtime_directory.join(required_path).is_file() {
            return Err(anyhow!("the CPython/WASI runtime is incomplete"));
        }
    }
    Ok(())
}

fn build_wasi_context(
    runtime_directory: &Path,
    workspace: &Path,
    stdout: MemoryOutputPipe,
    stderr: MemoryOutputPipe,
) -> Result<WasiP1Ctx> {
    let mut wasi = WasiCtxBuilder::new();
    wasi.stdout(stdout)
        .stderr(stderr)
        .env("PYTHONHOME", GUEST_RUNTIME)
        .args(&["python.wasm", "-B", "-s", "-S", "-P", GUEST_SCRIPT])
        .allow_ip_name_lookup(false)
        .allow_tcp(false)
        .allow_udp(false)
        .max_random_size(MAX_RANDOM_REQUEST_BYTES)
        .preopened_dir(
            runtime_directory,
            GUEST_RUNTIME,
            DirPerms::READ,
            FilePerms::READ,
        )?
        .preopened_dir(workspace, GUEST_WORK, DirPerms::READ, FilePerms::READ)?;
    Ok(wasi.build_p1())
}

fn classify_execution(
    error: Option<&WasmtimeError>,
    timed_out: bool,
    stdout_bytes: usize,
    stderr_bytes: usize,
) -> ExecutionStatus {
    if timed_out {
        return ExecutionStatus::TimedOut;
    }
    if stdout_bytes >= OUTPUT_LIMIT_BYTES || stderr_bytes >= OUTPUT_LIMIT_BYTES {
        return ExecutionStatus::OutputLimit;
    }
    match error {
        None => ExecutionStatus::Ok,
        Some(error)
            if error
                .downcast_ref::<I32Exit>()
                .is_some_and(|exit| exit.0 == 0) =>
        {
            ExecutionStatus::Ok
        }
        Some(error) if error.downcast_ref::<I32Exit>().is_some() => ExecutionStatus::PythonError,
        Some(error) if error.to_string().contains("MemoryOutputPipe") => {
            ExecutionStatus::OutputLimit
        }
        Some(_) => ExecutionStatus::ResourceLimit,
    }
}

struct Deadline {
    stop_sender: mpsc::Sender<()>,
    timer: thread::JoinHandle<()>,
}

impl Deadline {
    fn stop(self) {
        let _ = self.stop_sender.send(());
        let _ = self.timer.join();
    }
}

fn start_deadline(engine: Engine, timed_out: Arc<AtomicBool>) -> Deadline {
    let (stop_sender, stop_receiver) = mpsc::channel();
    let timer = thread::spawn(move || {
        if stop_receiver.recv_timeout(EXECUTION_TIMEOUT).is_err() {
            timed_out.store(true, Ordering::Release);
            engine.increment_epoch();
        }
    });
    Deadline { stop_sender, timer }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(code: &str, purpose: &str) -> PythonExecutionRequest {
        PythonExecutionRequest {
            code: code.to_owned(),
            purpose: purpose.to_owned(),
        }
    }

    #[test]
    fn request_accepts_small_visible_program() {
        request("print(6 * 7)", "Calculate a deterministic result")
            .validate()
            .expect("small request should pass");
    }

    #[test]
    fn request_rejects_empty_oversized_and_nul_code() {
        assert!(request("  ", "Calculate").validate().is_err());
        assert!(
            request(&"x".repeat(MAX_CODE_BYTES + 1), "Calculate")
                .validate()
                .is_err()
        );
        assert!(request("print('\0')", "Calculate").validate().is_err());
    }

    #[test]
    fn request_rejects_invalid_purpose() {
        assert!(request("pass", " ").validate().is_err());
        assert!(
            request("pass", &"x".repeat(MAX_PURPOSE_CHARACTERS + 1))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn fixed_failure_contains_no_internal_detail() {
        let result = PythonExecutionResult::fixed_failure(
            ExecutionStatus::InternalError,
            "Python execution could not be started.",
        );
        assert_eq!(result.status, ExecutionStatus::InternalError);
        assert_eq!(result.duration_ms, 0);
        assert!(result.stderr.contains("could not be started"));
    }
}
