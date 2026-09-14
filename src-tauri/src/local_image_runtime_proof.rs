#![deny(missing_docs)]
//! Explicit Apple-silicon runtime proof for Bottie's pinned local Qwen Image candidate.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{self, Stdio},
    time::{Duration, Instant},
};

use image::GenericImageView;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[path = "local_image_worker.rs"]
mod local_image_worker;

use local_image_worker::{
    manager::ManagerEvent,
    model_cache::begin_cached_model_load,
    model_package::{
        MLX_GEN_RUNTIME_REVISION, QWEN_IMAGE_2512_GENERATION_PROFILE,
        QWEN_IMAGE_2512_HARDWARE_PROFILE, QWEN_IMAGE_2512_PACKAGE_REVISION,
        qwen_image_2512_q4_candidate,
    },
    runtime_proof::{hash_worker_bundle, lifetime_peak_memory},
    transport::{TransportTimeouts, WorkerProcessSpec, WorkerTransport},
};

const ACKNOWLEDGEMENT_FLAG: &str = "--runtime-revision";
const SANDBOX_EXECUTABLE: &str = "/usr/bin/sandbox-exec";
const NETWORK_PROBE_EXECUTABLE: &str = "/usr/bin/nc";
const NETWORKLESS_PROFILE: &str = "(version 1) (allow default) (deny network*)";
const WORKER_EXECUTABLE: &str = "bottie-local-image-mlx-worker";
const PROOF_PROMPT: &str =
    "A violet glass robot tending a tiny greenhouse, studio light, detailed botanical illustration";
const PROOF_WIDTH: u32 = 512;
const PROOF_HEIGHT: u32 = 512;
const COMPLETE_SEED: u64 = 42;
const CANCEL_SEED: u64 = 43;
const MAX_PNG_BYTES: u64 = 64 * 1024 * 1024;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct ProofPaths {
    cache_root: PathBuf,
    worker_bundle: PathBuf,
    output_directory: PathBuf,
    measurements: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProofMeasurements {
    package_revision: String,
    runtime_revision: String,
    hardware_profile: String,
    generation_profile: String,
    generated_rgb_sha256: String,
    worker_sha256: String,
    worker_byte_size: u64,
    worker_bundle_sha256: String,
    worker_bundle_byte_size: u64,
    peak_memory_bytes: u64,
    cancellation_latency_ms: u64,
    network_sandbox_proved: bool,
    visual_reviewed: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("local image runtime proof failed: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), &'static str> {
    let paths = parse_arguments(std::env::args_os())?;
    prove_network_sandbox()?;
    prepare_output_directory(&paths.output_directory)?;
    let worker_executable = paths.worker_bundle.join(WORKER_EXECUTABLE);
    let bundle = hash_worker_bundle(&paths.worker_bundle, &worker_executable)
        .map_err(|_| "worker bundle hashing failed")?;
    let candidate = qwen_image_2512_q4_candidate();
    let plan = candidate
        .proof_source_plan(128 * 1024 * 1024 * 1024)
        .map_err(|_| "proof package plan is invalid")?;
    let spec = WorkerProcessSpec::new(
        PathBuf::from(SANDBOX_EXECUTABLE),
        vec![
            OsString::from("-p"),
            OsString::from(NETWORKLESS_PROFILE),
            worker_executable.into_os_string(),
            paths.output_directory.clone().into_os_string(),
        ],
    );
    let timeouts = TransportTimeouts::new(
        HANDSHAKE_TIMEOUT,
        EVENT_TIMEOUT,
        WRITE_TIMEOUT,
        SHUTDOWN_TIMEOUT,
    );
    let mut transport =
        WorkerTransport::spawn_with_timeouts(spec, env!("CARGO_PKG_VERSION"), timeouts)
            .await
            .map_err(|_| "worker handshake failed")?;
    let process_id = transport
        .process_id()
        .ok_or("worker process identity unavailable")?;

    begin_cached_model_load(
        &mut transport,
        "proof-load",
        &paths.cache_root,
        plan.manifest().clone(),
    )
    .await
    .map_err(|_| "verified model load could not start")?;
    wait_for_model_load(&mut transport).await?;
    run_complete_generation(&mut transport).await?;
    let generated_path = paths.output_directory.join("generated.png");
    let review_path = paths.output_directory.join("proof-output.png");
    let generated_rgb_sha256 = validate_generated_png(&generated_path)?;
    fs::rename(generated_path, review_path).map_err(|_| "review PNG could not be retained")?;
    let cancellation_latency_ms = run_cancellation_proof(&mut transport).await?;
    let peak_memory_bytes =
        lifetime_peak_memory(process_id).map_err(|_| "peak memory measurement failed")?;
    transport
        .shutdown()
        .await
        .map_err(|_| "worker shutdown failed")?;

    let measurements = RuntimeProofMeasurements {
        package_revision: QWEN_IMAGE_2512_PACKAGE_REVISION.into(),
        runtime_revision: MLX_GEN_RUNTIME_REVISION.into(),
        hardware_profile: QWEN_IMAGE_2512_HARDWARE_PROFILE.into(),
        generation_profile: QWEN_IMAGE_2512_GENERATION_PROFILE.into(),
        generated_rgb_sha256,
        worker_sha256: bundle.executable_sha256,
        worker_byte_size: bundle.executable_byte_size,
        worker_bundle_sha256: bundle.bundle_sha256,
        worker_bundle_byte_size: bundle.bundle_byte_size,
        peak_memory_bytes,
        cancellation_latency_ms,
        network_sandbox_proved: true,
        visual_reviewed: false,
    };
    write_measurements(&paths.measurements, &measurements)?;
    println!("local image runtime measurements completed; visual review remains required");
    Ok(())
}

async fn wait_for_model_load(transport: &mut WorkerTransport) -> Result<(), &'static str> {
    loop {
        match transport
            .next_event()
            .await
            .map_err(|_| "model load transport failed")?
        {
            ManagerEvent::Progress => {}
            ManagerEvent::ModelLoaded => return Ok(()),
            _ => return Err("model load did not complete"),
        }
    }
}

async fn run_complete_generation(transport: &mut WorkerTransport) -> Result<(), &'static str> {
    transport
        .begin_generation(
            "proof-complete",
            PROOF_PROMPT,
            PROOF_WIDTH,
            PROOF_HEIGHT,
            1,
            Some(COMPLETE_SEED),
        )
        .await
        .map_err(|_| "complete generation could not start")?;
    loop {
        match transport
            .next_event()
            .await
            .map_err(|_| "complete generation transport failed")?
        {
            ManagerEvent::Progress => {}
            ManagerEvent::Generated(outputs)
                if outputs.len() == 1 && outputs[0].output_name == "generated.png" =>
            {
                return Ok(());
            }
            _ => return Err("complete generation did not produce the exact output"),
        }
    }
}

async fn run_cancellation_proof(transport: &mut WorkerTransport) -> Result<u64, &'static str> {
    transport
        .begin_generation(
            "proof-cancel",
            PROOF_PROMPT,
            PROOF_WIDTH,
            PROOF_HEIGHT,
            1,
            Some(CANCEL_SEED),
        )
        .await
        .map_err(|_| "cancellation generation could not start")?;
    for _ in 0..2 {
        if transport
            .next_event()
            .await
            .map_err(|_| "cancellation setup transport failed")?
            != ManagerEvent::Progress
        {
            return Err("generation ended before active-step cancellation");
        }
    }
    let started = Instant::now();
    transport
        .cancel_active()
        .await
        .map_err(|_| "active cancellation could not be sent")?;
    loop {
        match transport
            .next_event()
            .await
            .map_err(|_| "active cancellation exceeded its transport grace")?
        {
            ManagerEvent::Progress => {}
            ManagerEvent::Cancelled => {
                let millis = started.elapsed().as_millis();
                return u64::try_from(millis.max(1))
                    .map_err(|_| "cancellation measurement overflow");
            }
            _ => return Err("active generation did not cooperatively cancel"),
        }
    }
}

fn validate_generated_png(path: &Path) -> Result<String, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "generated PNG is missing")?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_PNG_BYTES {
        return Err("generated PNG has an invalid native file shape");
    }
    let bytes = fs::read(path).map_err(|_| "generated PNG could not be read")?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|_| "generated PNG did not decode")?;
    if image.dimensions() != (PROOF_WIDTH, PROOF_HEIGHT) {
        return Err("generated PNG dimensions did not match the proof profile");
    }
    Ok(format!("{:x}", Sha256::digest(image.to_rgb8().as_raw())))
}

fn prove_network_sandbox() -> Result<(), &'static str> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|_| "network sandbox probe could not bind")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "network sandbox probe could not become nonblocking")?;
    let port = listener
        .local_addr()
        .map_err(|_| "network sandbox probe address unavailable")?
        .port()
        .to_string();
    let control = process::Command::new(NETWORK_PROBE_EXECUTABLE)
        .args(["-z", "127.0.0.1", &port])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "network sandbox control could not run")?;
    if !control.success() || listener.accept().is_err() {
        return Err("network sandbox control did not reach the listener");
    }
    let status = process::Command::new(SANDBOX_EXECUTABLE)
        .args([
            "-p",
            NETWORKLESS_PROFILE,
            NETWORK_PROBE_EXECUTABLE,
            "-z",
            "127.0.0.1",
            &port,
        ])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "network sandbox probe could not run")?;
    if status.success() {
        return Err("network sandbox unexpectedly allowed a connection");
    }
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(()),
        _ => Err("network sandbox probe reached the listener"),
    }
}

fn prepare_output_directory(path: &Path) -> Result<(), &'static str> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path).map_err(|_| "output directory is invalid")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("output directory is invalid");
        }
    } else {
        fs::create_dir_all(path).map_err(|_| "output directory could not be created")?;
    }
    if fs::read_dir(path)
        .map_err(|_| "output directory could not be inspected")?
        .next()
        .is_some()
    {
        return Err("output directory must be empty");
    }
    Ok(())
}

fn write_measurements(
    path: &Path,
    measurements: &RuntimeProofMeasurements,
) -> Result<(), &'static str> {
    let parent = path.parent().ok_or("measurement parent is required")?;
    if !parent.is_dir() {
        return Err("measurement parent is invalid");
    }
    let temporary = path.with_extension("json.tmp");
    let bytes =
        serde_json::to_vec_pretty(measurements).map_err(|_| "measurement encoding failed")?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| "measurement temporary file could not be opened")?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|_| "measurement write failed")?;
    fs::rename(temporary, path).map_err(|_| "measurement promotion failed")
}

fn parse_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ProofPaths, &'static str> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some(ACKNOWLEDGEMENT_FLAG.as_ref())
        || arguments.next().as_deref() != Some(MLX_GEN_RUNTIME_REVISION.as_ref())
    {
        return Err("exact runtime revision acknowledgement is required");
    }
    let paths = ProofPaths {
        cache_root: next_absolute_path(&mut arguments)?,
        worker_bundle: next_absolute_path(&mut arguments)?,
        output_directory: next_absolute_path(&mut arguments)?,
        measurements: next_absolute_path(&mut arguments)?,
    };
    if arguments.next().is_some() {
        return Err("unexpected proof arguments");
    }
    Ok(paths)
}

fn next_absolute_path(
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<PathBuf, &'static str> {
    let path = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("proof path is required")?;
    if !path.is_absolute() {
        return Err("proof paths must be absolute");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_requires_the_exact_runtime_revision_and_absolute_paths() {
        let root = std::env::temp_dir().join("bottie-runtime-proof");
        let valid = [
            OsString::from("proof"),
            OsString::from(ACKNOWLEDGEMENT_FLAG),
            OsString::from(MLX_GEN_RUNTIME_REVISION),
            root.join("cache").into_os_string(),
            root.join("worker").into_os_string(),
            root.join("output").into_os_string(),
            root.join("measurements.json").into_os_string(),
        ];
        assert!(parse_arguments(valid).is_ok());

        let relative = [
            OsString::from("proof"),
            OsString::from(ACKNOWLEDGEMENT_FLAG),
            OsString::from(MLX_GEN_RUNTIME_REVISION),
            OsString::from("cache"),
            root.join("worker").into_os_string(),
            root.join("output").into_os_string(),
            root.join("measurements.json").into_os_string(),
        ];
        assert!(parse_arguments(relative).is_err());
    }

    #[test]
    fn output_directory_must_be_empty_before_a_proof() {
        let root = std::env::temp_dir().join(format!(
            "bottie-runtime-proof-output-{}",
            uuid::Uuid::new_v4()
        ));
        prepare_output_directory(&root).unwrap();
        fs::write(root.join("stale.png"), b"stale").unwrap();
        assert_eq!(
            prepare_output_directory(&root).unwrap_err(),
            "output directory must be empty"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_png_evidence_hashes_decoded_rgb_pixels() {
        let root =
            std::env::temp_dir().join(format!("bottie-runtime-proof-png-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("proof.png");
        let pixels = vec![37_u8; (PROOF_WIDTH * PROOF_HEIGHT * 3) as usize];
        let image = image::RgbImage::from_raw(PROOF_WIDTH, PROOF_HEIGHT, pixels.clone()).unwrap();
        image
            .save_with_format(&path, image::ImageFormat::Png)
            .unwrap();

        assert_eq!(
            validate_generated_png(&path).unwrap(),
            format!("{:x}", Sha256::digest(&pixels))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
