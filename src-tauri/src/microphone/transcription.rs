//! Bounded local streaming speech recognition over session-only native PCM.

use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{SyncSender, sync_channel},
    },
    thread,
};

use hf_hub::{
    Repo, RepoType,
    api::sync::{ApiBuilder, ApiRepo},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, install_logging_hooks,
};

use super::{CaptureState, VoiceActivity, VoiceSegment, lock};

pub(super) const TRANSCRIPTION_INTERVAL_MS: u64 = 1_500;
const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
const MODEL_RUNTIME_THREADS: i32 = 2;
pub(super) const MAX_TRANSCRIPT_SEGMENTS: usize = 32;
pub(super) const MAX_TRANSCRIPT_TEXT_BYTES: usize = 4_000;
const MAX_SEGMENT_TEXT_BYTES: usize = 512;
const RUNTIME_ASSET_MANIFEST: &str = include_str!("../../../runtime-assets.json");

/// Path-free lifecycle for local speech recognition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TranscriptionPhase {
    /// No capture or recognition work exists.
    #[default]
    Idle,
    /// Capture is active but no bounded speech window is ready yet.
    Listening,
    /// The fixed local model is being verified or prepared after explicit capture.
    PreparingModel,
    /// A bounded native snapshot is currently being recognized.
    Transcribing,
    /// The stopped capture has a final bounded transcript, which may be empty.
    Ready,
    /// Local recognition failed with one stable redacted category.
    Error,
}

/// Stable local-recognition failure without cache, runtime, or model details.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TranscriptionErrorCode {
    /// The pinned model could not be obtained from or opened in the app-owned cache.
    ModelUnavailable,
    /// Cached model bytes did not match Bottie's immutable release contract.
    ModelIntegrity,
    /// The local inference runtime could not recognize the bounded PCM snapshot.
    RecognitionFailed,
}

/// One bounded path-free transcript range returned to the WebView.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TranscriptSegment {
    pub(super) text: String,
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
    pub(super) is_final: bool,
}

/// Untrusted raw recognizer output before text and range limits are applied.
pub(super) struct RawTranscriptSegment {
    pub(super) text: String,
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
}

/// One replaceable native-only recognition snapshot.
pub(super) struct TranscriptionJob {
    capture_id: u64,
    generation: u64,
    is_final: bool,
    samples: Vec<f32>,
    sample_rate_hz: u32,
    offset_ms: u64,
    end_ms: u64,
}

impl TranscriptionJob {
    /// Copies only the speech-containing PCM window for one bounded worker pass.
    pub(super) fn from_capture(
        capture_id: u64,
        generation: u64,
        is_final: bool,
        samples: &[f32],
        sample_rate_hz: u32,
        activity: &[VoiceSegment],
    ) -> Option<Self> {
        let (start_ms, end_ms) = speech_window(activity)?;
        let start_sample = milliseconds_to_samples(start_ms, sample_rate_hz).min(samples.len());
        let end_sample = milliseconds_to_samples(end_ms, sample_rate_hz).min(samples.len());
        (start_sample < end_sample).then(|| Self {
            capture_id,
            generation,
            is_final,
            samples: samples[start_sample..end_sample].to_vec(),
            sample_rate_hz,
            offset_ms: start_ms,
            end_ms,
        })
    }

    /// Returns the monotonic job generation for coalescing and stale-result rejection.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
}

/// Cheap wake handle for the one model-owning native recognition thread.
pub(super) struct TranscriptionWorker {
    wake: SyncSender<()>,
}

impl TranscriptionWorker {
    /// Starts a worker that remains idle and model-free until explicit capture produces speech.
    pub(super) fn start(model_cache_path: PathBuf, shared: Arc<Mutex<CaptureState>>) -> Self {
        let (wake, receiver) = sync_channel(1);
        thread::Builder::new()
            .name("bottie-speech-recognition".into())
            .spawn(move || {
                let mut transcriber = None;
                while receiver.recv().is_ok() {
                    if transcriber.is_none() && lock(&shared).pending_transcription.is_some() {
                        transcriber = match WhisperTranscriber::load(&model_cache_path) {
                            Ok(value) => Some(value),
                            Err(code) => {
                                let pending = { lock(&shared).take_pending_transcription() };
                                if let Some(job) = pending {
                                    lock(&shared).apply_transcription(
                                        job.capture_id,
                                        job.generation,
                                        job.is_final,
                                        Err(code),
                                    );
                                }
                                continue;
                            }
                        };
                    }
                    loop {
                        let pending = { lock(&shared).take_pending_transcription() };
                        let Some(job) = pending else {
                            break;
                        };
                        process_job(
                            &shared,
                            transcriber
                                .as_ref()
                                .expect("the local recognizer was initialized"),
                            job,
                        );
                    }
                }
            })
            .expect("the native speech-recognition worker should start");
        Self { wake }
    }

    /// Returns a bounded non-blocking wake sender for the audio owner.
    pub(super) fn wake_handle(&self) -> SyncSender<()> {
        self.wake.clone()
    }
}

struct WhisperTranscriber {
    context: WhisperContext,
}

impl WhisperTranscriber {
    fn load(cache_dir: &PathBuf) -> Result<Self, TranscriptionErrorCode> {
        std::fs::create_dir_all(cache_dir).map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
        let model_path = prepare_pinned_model(cache_dir)?;
        install_logging_hooks();
        let context =
            WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
                .map_err(|_| TranscriptionErrorCode::RecognitionFailed)?;
        Ok(Self { context })
    }

    fn transcribe(
        &self,
        job: &TranscriptionJob,
    ) -> Result<Vec<RawTranscriptSegment>, TranscriptionErrorCode> {
        let audio = resample_linear(&job.samples, job.sample_rate_hz, TARGET_SAMPLE_RATE_HZ);
        let mut state = self
            .context
            .create_state()
            .map_err(|_| TranscriptionErrorCode::RecognitionFailed)?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(None);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_n_threads(MODEL_RUNTIME_THREADS);
        state
            .full(params, &audio)
            .map_err(|_| TranscriptionErrorCode::RecognitionFailed)?;
        Ok(state
            .as_iter()
            .filter_map(|segment| {
                let start_ms = job
                    .offset_ms
                    .saturating_add(timestamp_to_milliseconds(segment.start_timestamp()))
                    .min(job.end_ms);
                let end_ms = job
                    .offset_ms
                    .saturating_add(timestamp_to_milliseconds(segment.end_timestamp()))
                    .min(job.end_ms);
                (end_ms >= start_ms).then(|| RawTranscriptSegment {
                    text: segment.to_string(),
                    start_ms,
                    end_ms,
                })
            })
            .collect())
    }
}

fn process_job(
    shared: &Arc<Mutex<CaptureState>>,
    transcriber: &WhisperTranscriber,
    job: TranscriptionJob,
) {
    lock(shared).mark_transcribing(job.capture_id, job.generation);
    let result = transcriber.transcribe(&job);
    lock(shared).apply_transcription(job.capture_id, job.generation, job.is_final, result);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeAssetManifest {
    schema_version: u8,
    whisper_tiny_q5: WhisperModelContract,
}

#[derive(Deserialize)]
struct WhisperModelContract {
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

fn prepare_pinned_model(cache_dir: &PathBuf) -> Result<PathBuf, TranscriptionErrorCode> {
    let contract: RuntimeAssetManifest = serde_json::from_str(RUNTIME_ASSET_MANIFEST)
        .map_err(|_| TranscriptionErrorCode::ModelIntegrity)?;
    if contract.schema_version != 1 {
        return Err(TranscriptionErrorCode::ModelIntegrity);
    }
    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir.clone())
        .with_endpoint("https://huggingface.co".into())
        .with_progress(false)
        .build()
        .map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
    let repository = api.repo(Repo::with_revision(
        contract.whisper_tiny_q5.repository,
        RepoType::Model,
        contract.whisper_tiny_q5.revision,
    ));
    verified_model_path(&repository, &contract.whisper_tiny_q5.file)
}

fn verified_model_path(
    repository: &ApiRepo,
    expected: &ModelFileContract,
) -> Result<PathBuf, TranscriptionErrorCode> {
    let path = repository
        .get(&expected.path)
        .map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
    let file = File::open(&path).map_err(|_| TranscriptionErrorCode::ModelUnavailable)?;
    verify_model_reader(file, expected.size, &expected.sha256)?;
    Ok(path)
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

/// Applies strict text, count, ordering, and timing limits to untrusted recognizer output.
pub(super) fn bounded_segments(
    segments: Vec<RawTranscriptSegment>,
    is_final: bool,
) -> Vec<TranscriptSegment> {
    let mut remaining = MAX_TRANSCRIPT_TEXT_BYTES;
    segments
        .into_iter()
        .filter_map(|segment| {
            let text = truncate_utf8(segment.text.trim(), remaining.min(MAX_SEGMENT_TEXT_BYTES));
            remaining = remaining.saturating_sub(text.len());
            (!text.is_empty() && segment.end_ms >= segment.start_ms).then_some(TranscriptSegment {
                text,
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                is_final,
            })
        })
        .take(MAX_TRANSCRIPT_SEGMENTS)
        .collect()
}

fn speech_window(activity: &[VoiceSegment]) -> Option<(u64, u64)> {
    let mut speech = activity
        .iter()
        .filter(|segment| segment.activity == VoiceActivity::Speech);
    let first = speech.next()?;
    let end_ms = speech.fold(first.end_ms, |end, segment| end.max(segment.end_ms));
    Some((first.start_ms, end_ms))
}

fn resample_linear(samples: &[f32], source_rate_hz: u32, target_rate_hz: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate_hz == 0 || target_rate_hz == 0 {
        return Vec::new();
    }
    if source_rate_hz == target_rate_hz {
        return samples.to_vec();
    }
    let output_len = samples
        .len()
        .saturating_mul(target_rate_hz as usize)
        .saturating_div(source_rate_hz as usize);
    (0..output_len)
        .map(|index| {
            let source_position =
                index as f64 * f64::from(source_rate_hz) / f64::from(target_rate_hz);
            let lower = source_position.floor() as usize;
            let upper = lower.saturating_add(1).min(samples.len() - 1);
            let fraction = (source_position - lower as f64) as f32;
            samples[lower] + (samples[upper] - samples[lower]) * fraction
        })
        .collect()
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_owned()
}

fn milliseconds_to_samples(milliseconds: u64, sample_rate_hz: u32) -> usize {
    usize::try_from(
        milliseconds
            .saturating_mul(u64::from(sample_rate_hz))
            .saturating_div(1_000),
    )
    .unwrap_or(usize::MAX)
}

fn timestamp_to_milliseconds(timestamp: i64) -> u64 {
    u64::try_from(timestamp).unwrap_or(0).saturating_mul(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_to_speech_and_resamples_without_exposing_silence() {
        let activity = vec![
            VoiceSegment {
                activity: VoiceActivity::Silence,
                start_ms: 0,
                end_ms: 100,
            },
            VoiceSegment {
                activity: VoiceActivity::Speech,
                start_ms: 100,
                end_ms: 300,
            },
            VoiceSegment {
                activity: VoiceActivity::Silence,
                start_ms: 300,
                end_ms: 400,
            },
        ];
        let job =
            TranscriptionJob::from_capture(1, 1, false, &[0.5; 400], 1_000, &activity).unwrap();
        assert_eq!(job.samples.len(), 200);
        assert_eq!(job.offset_ms, 100);
        assert_eq!(job.end_ms, 300);
        assert_eq!(resample_linear(&job.samples, 1_000, 16_000).len(), 3_200);
    }

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
    #[ignore = "downloads and executes Bottie's pinned local speech model"]
    fn live_pinned_model_loads_and_runs_a_bounded_native_snapshot() {
        let cache = std::env::temp_dir().join("bottie-live-speech-model");
        let transcriber = WhisperTranscriber::load(&cache).expect("pinned model should load");
        let sample_count = TARGET_SAMPLE_RATE_HZ as usize * 5;
        let samples = (0..sample_count)
            .map(|index| {
                let phase =
                    index as f32 * 440.0 * std::f32::consts::TAU / TARGET_SAMPLE_RATE_HZ as f32;
                phase.sin() * 0.1
            })
            .collect();
        let job = TranscriptionJob {
            capture_id: 1,
            generation: 1,
            is_final: true,
            samples,
            sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
            offset_ms: 0,
            end_ms: 5_000,
        };
        let result = transcriber
            .transcribe(&job)
            .expect("local inference should complete");
        assert!(result.iter().all(|segment| segment.end_ms <= job.end_ms));
    }
}
