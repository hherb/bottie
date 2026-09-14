//! Versioned, closed-schema messages and framing for private local image workers.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[path = "protocol/framing.rs"]
mod framing;
#[path = "protocol/validation.rs"]
mod validation;

use framing::encode_frame;
pub(crate) use framing::{FrameDecoder, MAX_FRAME_BYTES};
use validation::{validate_host_message, validate_worker_message};

/// The only local image-worker protocol version accepted by this build.
pub(crate) const CURRENT_PROTOCOL_VERSION: u16 = 1;
/// Maximum terminal request identities retained for one worker session.
const MAX_TERMINAL_RESULTS: usize = 1_024;

/// Stable failures that never retain malformed payload or worker error detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProtocolError {
    /// A zero-length or structurally impossible frame was supplied.
    MalformedFrame,
    /// The stream ended while a declared frame remained incomplete.
    TruncatedFrame,
    /// A declared payload exceeded the fixed protocol ceiling.
    FrameTooLarge,
    /// JSON or a closed message schema was malformed.
    MalformedMessage,
    /// The peer requested a protocol version this build does not implement.
    UnsupportedVersion,
    /// A message field violated its named protocol bound.
    InvalidField,
    /// A worker failure contained path-shaped or credential-shaped detail.
    UnsafeErrorDetail,
    /// A worker emitted more than one terminal result for a request.
    DuplicateTerminalResult,
    /// The bounded terminal-result ledger was exhausted.
    SessionLimitReached,
}

/// Native-only location of one previously acquired and verified model revision.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ModelLocation {
    /// Exact upstream model identity, distinct from hosted Qwen-Image-2.0.
    pub(crate) model_id: String,
    /// Immutable source revision whose files were verified before activation.
    pub(crate) model_revision: String,
    /// App-owned native directory never exposed to the WebView or provider.
    pub(crate) model_directory: String,
}

/// Commands sent by Rust to one private, networkless local image worker.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum HostMessage {
    /// Begins exact-version negotiation without loading a model.
    Hello {
        /// Requested protocol version.
        protocol_version: u16,
        /// Bottie build version for diagnostic compatibility only.
        client_version: String,
    },
    /// Loads one exact, already acquired local model revision.
    Load {
        /// Requested protocol version.
        protocol_version: u16,
        /// Opaque native correlation identity.
        request_id: String,
        /// Exact model identity, revision, and app-owned location.
        model: ModelLocation,
    },
    /// Starts one bounded text-to-image generation.
    Generate {
        /// Requested protocol version.
        protocol_version: u16,
        /// Opaque native correlation identity.
        request_id: String,
        /// Exact loaded open-weight model identity.
        model_id: String,
        /// Native-only text prompt.
        prompt: String,
        /// Requested output width.
        width: u32,
        /// Requested output height.
        height: u32,
        /// Requested output count.
        count: u8,
        /// Deterministic seed when the negotiated runtime supports it.
        seed: Option<u64>,
    },
    /// Requests cooperative cancellation of one active operation.
    Cancel {
        /// Requested protocol version.
        protocol_version: u16,
        /// Exact request to cancel.
        request_id: String,
    },
    /// Requests clean worker termination after active work has ended.
    Shutdown {
        /// Requested protocol version.
        protocol_version: u16,
    },
}

/// Bounded worker capabilities reported before any model is loaded.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct WorkerCapabilities {
    /// Exact embedded runtime identity, such as a pinned MLX-Gen build.
    pub(crate) runtime_id: String,
    /// Whether this worker supports text-to-image generation.
    pub(crate) generation: bool,
    /// Whether this runtime accepts and reports deterministic seeds.
    pub(crate) supports_seed: bool,
    /// Maximum outputs supported for one request.
    pub(crate) max_outputs: u8,
    /// Maximum pixels supported for each output.
    pub(crate) max_pixels: u64,
}

/// Coarse progress stages shared by every local backend.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerProgressStage {
    /// Loading verified model files into the selected runtime.
    Loading,
    /// Preparing prompt embeddings and generation inputs.
    Preparing,
    /// Executing cooperative diffusion or flow-matching steps.
    Denoising,
    /// Decoding and writing the native temporary PNG output.
    Decoding,
}

/// Operation associated with a terminal worker result.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerOperation {
    /// Model load operation.
    Load,
    /// Image generation operation.
    Generate,
}

/// Stable failure categories that do not reveal backend exception text.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerFailureCode {
    /// The runtime does not support the requested operation or capability.
    Unsupported,
    /// The request violated the worker's negotiated bounds.
    InvalidRequest,
    /// Verified model files could not be loaded.
    ModelLoadFailed,
    /// Generation failed after inputs were accepted.
    GenerationFailed,
    /// The worker encountered an otherwise unclassified internal failure.
    Internal,
}

/// One native temporary output owned and validated by Rust after generation.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct WorkerOutput {
    /// Single-component PNG name relative to the Rust-created output directory.
    pub(crate) output_name: String,
    /// Worker-reported decoded width, revalidated by Rust.
    pub(crate) width: u32,
    /// Worker-reported decoded height, revalidated by Rust.
    pub(crate) height: u32,
    /// Effective generation seed when supported.
    pub(crate) seed: Option<u64>,
}

/// Terminal outcome for one load or generation request.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum WorkerResult {
    /// The operation completed and may include generated outputs.
    Completed {
        /// Empty for model load and non-empty for image generation.
        outputs: Vec<WorkerOutput>,
    },
    /// The operation cooperatively stopped without usable output.
    Cancelled {},
    /// The operation failed with a stable code and optional safe detail.
    Failed {
        /// Stable native-facing failure category.
        code: WorkerFailureCode,
        /// Optional bounded path-free and credential-free explanation.
        message: Option<String>,
    },
}

/// Events emitted by one private local image worker.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum WorkerMessage {
    /// Confirms the exact supported protocol version.
    Hello {
        /// Worker-selected protocol version.
        protocol_version: u16,
        /// Pinned worker package version.
        worker_version: String,
    },
    /// Reports runtime limits before model loading.
    Capabilities {
        /// Worker-selected protocol version.
        protocol_version: u16,
        /// Closed bounded runtime capability record.
        capabilities: WorkerCapabilities,
    },
    /// Reports monotonic coarse progress for an active operation.
    Progress {
        /// Active protocol version.
        protocol_version: u16,
        /// Opaque native correlation identity.
        request_id: String,
        /// Current backend-neutral progress stage.
        stage: WorkerProgressStage,
        /// Completed units within this stage.
        completed_steps: u32,
        /// Total units within this stage.
        total_steps: u32,
    },
    /// Reports exactly one terminal outcome for a request.
    Result {
        /// Active protocol version.
        protocol_version: u16,
        /// Opaque native correlation identity.
        request_id: String,
        /// Operation whose lifecycle ended.
        operation: WorkerOperation,
        /// Closed terminal result.
        result: WorkerResult,
    },
}

/// Bounded per-process validation state for terminal result uniqueness.
#[derive(Debug, Default)]
pub(crate) struct WorkerProtocolSession {
    terminal_request_ids: HashSet<String>,
}

impl WorkerProtocolSession {
    /// Creates an empty protocol session before the worker handshake.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Validates one message and rejects duplicate terminal results fail-closed.
    pub(crate) fn accept(&mut self, message: &WorkerMessage) -> Result<(), ProtocolError> {
        validate_worker_message(message)?;
        let WorkerMessage::Result { request_id, .. } = message else {
            return Ok(());
        };
        if self.terminal_request_ids.contains(request_id) {
            return Err(ProtocolError::DuplicateTerminalResult);
        }
        if self.terminal_request_ids.len() >= MAX_TERMINAL_RESULTS {
            return Err(ProtocolError::SessionLimitReached);
        }
        self.terminal_request_ids.insert(request_id.clone());
        Ok(())
    }
}

/// Encodes one validated host command as a bounded length-prefixed JSON frame.
pub(crate) fn encode_host_frame(message: &HostMessage) -> Result<Vec<u8>, ProtocolError> {
    validate_host_message(message)?;
    encode_frame(message)
}

/// Encodes one validated worker event as a bounded length-prefixed JSON frame.
pub(crate) fn encode_worker_frame(message: &WorkerMessage) -> Result<Vec<u8>, ProtocolError> {
    validate_worker_message(message)?;
    encode_frame(message)
}

/// Decodes and validates one host-command JSON payload after framing.
pub(crate) fn decode_host_payload(payload: &[u8]) -> Result<HostMessage, ProtocolError> {
    validate_payload_size(payload)?;
    let message = serde_json::from_slice(payload).map_err(|_| ProtocolError::MalformedMessage)?;
    validate_host_message(&message)?;
    Ok(message)
}

/// Decodes and validates one worker-event JSON payload after framing.
pub(crate) fn decode_worker_payload(payload: &[u8]) -> Result<WorkerMessage, ProtocolError> {
    validate_payload_size(payload)?;
    let message = serde_json::from_slice(payload).map_err(|_| ProtocolError::MalformedMessage)?;
    validate_worker_message(&message)?;
    Ok(message)
}

fn validate_payload_size(payload: &[u8]) -> Result<(), ProtocolError> {
    if payload.is_empty() {
        Err(ProtocolError::MalformedMessage)
    } else if payload.len() > MAX_FRAME_BYTES {
        Err(ProtocolError::FrameTooLarge)
    } else {
        Ok(())
    }
}
