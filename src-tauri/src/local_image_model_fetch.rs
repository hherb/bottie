#![deny(missing_docs)]
//! Explicit, feature-gated acquisition tool for the pinned local-image runtime proof.

use std::{ffi::OsString, path::PathBuf, process, time::Duration};

#[path = "local_image_worker.rs"]
mod local_image_worker;

use local_image_worker::{
    model_acquisition::ModelAcquisition,
    model_download::{DownloadError, ModelDownloadCancellation, ModelDownloader},
    model_package::qwen_image_2512_q4_candidate,
};

const ACKNOWLEDGEMENT_FLAG: &str = "--acknowledge-bytes";
const EXPECTED_DOWNLOAD_BYTES: u64 = 17_442_350_812;
const PROOF_MEMORY_CEILING_BYTES: u64 = 128 * 1_024 * 1_024 * 1_024;
const RETRY_DELAY: Duration = Duration::from_secs(10);
const MAX_RETRY_ATTEMPTS: u32 = 24;
const PROGRESS_REPORT_BYTES: u64 = 256 * 1024 * 1024;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("local image model fetch failed: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), &'static str> {
    let cache_root = parse_arguments(std::env::args_os())?;
    let candidate = qwen_image_2512_q4_candidate();
    let plan = candidate
        .proof_source_plan(PROOF_MEMORY_CEILING_BYTES)
        .map_err(|_| "approved proof plan is invalid")?;
    let mut acquisition = ModelAcquisition::new(plan.manifest().clone())
        .map_err(|_| "approved manifest is invalid")?;
    acquisition
        .begin_download()
        .map_err(|_| "approval state is invalid")?;
    let downloader = ModelDownloader::new().map_err(|_| "downloader initialization failed")?;
    let cancellation = ModelDownloadCancellation::default();
    let mut last_reported_bytes = 0_u64;
    let mut last_completed_files = 0_u32;

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        let result = downloader
            .download_to_cache(
                &plan,
                &mut acquisition,
                &cache_root,
                &cancellation,
                |progress| {
                    if progress.completed_files == last_completed_files
                        && progress.downloaded_bytes
                            < last_reported_bytes.saturating_add(PROGRESS_REPORT_BYTES)
                        && progress.downloaded_bytes != progress.total_bytes
                    {
                        return;
                    }
                    last_reported_bytes = progress.downloaded_bytes;
                    last_completed_files = progress.completed_files;
                    println!(
                        "model download: {}/{} files, {}/{} bytes durably retained",
                        progress.completed_files,
                        progress.total_files,
                        progress.downloaded_bytes,
                        progress.total_bytes
                    );
                },
            )
            .await;
        match result {
            Ok(_) => {
                println!("model download verified and promoted");
                return Ok(());
            }
            Err(error) if retryable(error) && attempt < MAX_RETRY_ATTEMPTS => {
                eprintln!(
                    "model download interrupted; retrying resumable cache ({attempt}/{MAX_RETRY_ATTEMPTS})"
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(error) => {
                eprintln!("model download stopped with path-free status: {error:?}");
                return Err("download or cache verification failed");
            }
        }
    }
    Err("download retry limit reached")
}

fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<PathBuf, &'static str> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some(ACKNOWLEDGEMENT_FLAG.as_ref()) {
        return Err("exact byte acknowledgement is required");
    }
    let acknowledged = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if acknowledged != Some(EXPECTED_DOWNLOAD_BYTES) {
        return Err("exact byte acknowledgement is required");
    }
    let cache_root = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("cache root is required")?;
    if arguments.next().is_some() || !cache_root.is_absolute() {
        return Err("one absolute cache root is required");
    }
    Ok(cache_root)
}

fn retryable(error: DownloadError) -> bool {
    matches!(
        error,
        DownloadError::Transport | DownloadError::Interrupted | DownloadError::Timeout
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_byte_acknowledgement_and_absolute_cache_root_are_required() {
        let root = std::env::temp_dir().join("bottie-proof-cache");
        let valid = [
            OsString::from("fetch"),
            OsString::from(ACKNOWLEDGEMENT_FLAG),
            OsString::from(EXPECTED_DOWNLOAD_BYTES.to_string()),
            root.clone().into_os_string(),
        ];
        assert_eq!(parse_arguments(valid).unwrap(), root);

        let wrong_size = [
            OsString::from("fetch"),
            OsString::from(ACKNOWLEDGEMENT_FLAG),
            OsString::from("1"),
            OsString::from("/tmp/cache"),
        ];
        assert!(parse_arguments(wrong_size).is_err());
    }
}
