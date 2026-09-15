//! Pure lifecycle policy for one long-lived, single-operation local image worker.

use std::{collections::HashSet, time::Duration};

use super::protocol::{
    CURRENT_PROTOCOL_VERSION, HostMessage, ModelLocation, ProtocolError, WorkerCapabilities,
    WorkerMessage, WorkerOperation, WorkerOutput, WorkerProtocolSession, WorkerResult,
    encode_host_frame,
};

/// Cooperative cancellation grace before the owning transport must kill the worker.
pub(super) const CANCELLATION_GRACE: Duration = Duration::from_secs(3);

/// Stable manager failures without worker payload or native path detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ManagerError {
    /// The worker already owns an active operation.
    Busy,
    /// The requested command is not valid in the current lifecycle phase.
    InvalidState,
    /// A protocol message failed closed-schema or field validation.
    Protocol,
    /// The worker replied for a request other than the active request.
    CorrelationMismatch,
    /// Completed outputs differ from the exact accepted generation request.
    ResultMismatch,
    /// The requested generation exceeds negotiated worker capabilities.
    CapabilityMismatch,
    /// A valid message arrived out of handshake or operation order.
    UnexpectedMessage,
}

/// Path-free readiness exposed by future native command adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkerReadiness {
    /// No worker transport is owned.
    Stopped,
    /// Version and capabilities are still being negotiated.
    Starting,
    /// A worker is ready but has no loaded model.
    Ready,
    /// A verified model is loaded and reusable.
    Loaded,
    /// One load, generation, cancellation, or shutdown is active.
    Busy,
}

/// Accepted worker event after manager correlation and lifecycle validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ManagerEvent {
    /// The worker confirmed the current protocol version.
    Hello,
    /// The worker completed capability negotiation.
    Ready,
    /// The exact requested model was loaded.
    ModelLoaded,
    /// A bounded progress update matched the active request.
    Progress,
    /// Generation completed with private relative output names.
    Generated(Vec<WorkerOutput>),
    /// The active operation cooperatively cancelled.
    Cancelled,
    /// The active operation failed with already validated safe detail.
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Stopped,
    AwaitingHello,
    AwaitingCapabilities,
    Idle,
    Busy,
    Stopping,
}

#[derive(Clone, Debug)]
struct ActiveOperation {
    request_id: String,
    operation: WorkerOperation,
    generation: Option<GenerationExpectation>,
    cancellation_started: Option<Duration>,
}

#[derive(Clone, Debug)]
struct GenerationExpectation {
    width: u32,
    height: u32,
    count: u8,
    seed: Option<u64>,
    supports_seed: bool,
    max_outputs: u8,
    max_pixels: u64,
}

/// Rust-owned lifecycle state for one attached private worker process.
#[derive(Debug)]
pub(crate) struct WorkerManager {
    phase: Phase,
    capabilities: Option<WorkerCapabilities>,
    loaded_model: Option<ModelLocation>,
    pending_model: Option<ModelLocation>,
    active: Option<ActiveOperation>,
    protocol_session: WorkerProtocolSession,
    teardown_required: bool,
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self {
            phase: Phase::Stopped,
            capabilities: None,
            loaded_model: None,
            pending_model: None,
            active: None,
            protocol_session: WorkerProtocolSession::new(),
            teardown_required: false,
        }
    }
}

impl WorkerManager {
    /// Creates a stopped manager with no retained runtime or model state.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Returns coarse path-free readiness without exposing process or model locations.
    pub(crate) fn readiness(&self) -> WorkerReadiness {
        match self.phase {
            Phase::Stopped => WorkerReadiness::Stopped,
            Phase::AwaitingHello | Phase::AwaitingCapabilities => WorkerReadiness::Starting,
            Phase::Idle if self.loaded_model.is_some() => WorkerReadiness::Loaded,
            Phase::Idle => WorkerReadiness::Ready,
            Phase::Busy | Phase::Stopping => WorkerReadiness::Busy,
        }
    }

    /// Returns the negotiated runtime identity without exposing capabilities across IPC.
    pub(crate) fn runtime_id(&self) -> Option<&str> {
        self.capabilities
            .as_ref()
            .map(|capabilities| capabilities.runtime_id.as_str())
    }

    /// Starts exact-version negotiation for one newly spawned private worker.
    pub(crate) fn begin_handshake(
        &mut self,
        client_version: impl Into<String>,
    ) -> Result<HostMessage, ManagerError> {
        if self.phase != Phase::Stopped || self.teardown_required {
            return Err(ManagerError::InvalidState);
        }
        let message = HostMessage::Hello {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            client_version: client_version.into(),
        };
        validate_outgoing(&message)?;
        self.phase = Phase::AwaitingHello;
        Ok(message)
    }

    /// Starts loading one exact, already acquired and verified model revision.
    pub(crate) fn begin_load(
        &mut self,
        request_id: impl Into<String>,
        model: ModelLocation,
    ) -> Result<HostMessage, ManagerError> {
        self.require_idle()?;
        let request_id = request_id.into();
        let message = HostMessage::Load {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: request_id.clone(),
            model: model.clone(),
        };
        validate_outgoing(&message)?;
        self.loaded_model = None;
        self.pending_model = Some(model);
        self.active = Some(ActiveOperation {
            request_id,
            operation: WorkerOperation::Load,
            generation: None,
            cancellation_started: None,
        });
        self.phase = Phase::Busy;
        Ok(message)
    }

    /// Starts one generation against the retained loaded model.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_generation(
        &mut self,
        request_id: impl Into<String>,
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
        seed: Option<u64>,
    ) -> Result<HostMessage, ManagerError> {
        self.require_idle()?;
        let model = self
            .loaded_model
            .as_ref()
            .ok_or(ManagerError::InvalidState)?;
        let capabilities = self
            .capabilities
            .as_ref()
            .ok_or(ManagerError::InvalidState)?;
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        if count == 0
            || count > capabilities.max_outputs
            || pixels > capabilities.max_pixels
            || (seed.is_some() && !capabilities.supports_seed)
        {
            return Err(ManagerError::CapabilityMismatch);
        }
        let request_id = request_id.into();
        let message = HostMessage::Generate {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: request_id.clone(),
            model_id: model.model_id.clone(),
            prompt: prompt.into(),
            width,
            height,
            count,
            seed,
        };
        validate_outgoing(&message)?;
        self.active = Some(ActiveOperation {
            request_id,
            operation: WorkerOperation::Generate,
            generation: Some(GenerationExpectation {
                width,
                height,
                count,
                seed,
                supports_seed: capabilities.supports_seed,
                max_outputs: capabilities.max_outputs,
                max_pixels: capabilities.max_pixels,
            }),
            cancellation_started: None,
        });
        self.phase = Phase::Busy;
        Ok(message)
    }

    /// Requests cooperative cancellation and starts the fixed teardown grace period.
    pub(crate) fn cancel_active(&mut self, now: Duration) -> Result<HostMessage, ManagerError> {
        let active = self.active.as_mut().ok_or(ManagerError::InvalidState)?;
        if active.cancellation_started.is_some() {
            return Err(ManagerError::Busy);
        }
        let message = HostMessage::Cancel {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: active.request_id.clone(),
        };
        validate_outgoing(&message)?;
        active.cancellation_started = Some(now);
        Ok(message)
    }

    /// Returns true once cancellation requires forced transport teardown and resets manager state.
    pub(crate) fn requires_forced_teardown(&mut self, now: Duration) -> bool {
        let expired = self
            .active
            .as_ref()
            .and_then(|active| active.cancellation_started)
            .is_some_and(|started| now.saturating_sub(started) >= CANCELLATION_GRACE);
        if expired {
            self.fail_closed();
        }
        expired
    }

    /// Requests clean shutdown only when no operation is active.
    pub(crate) fn begin_shutdown(&mut self) -> Result<HostMessage, ManagerError> {
        self.require_idle()?;
        let message = HostMessage::Shutdown {
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        validate_outgoing(&message)?;
        self.phase = Phase::Stopping;
        Ok(message)
    }

    /// Records process exit and discards all process-specific capability and model state.
    pub(crate) fn mark_stopped(&mut self) {
        self.reset(false);
    }

    /// Accepts one validated worker message in strict lifecycle and request order.
    pub(crate) fn accept(&mut self, message: WorkerMessage) -> Result<ManagerEvent, ManagerError> {
        if self.protocol_session.accept(&message).is_err() {
            self.fail_closed();
            return Err(ManagerError::Protocol);
        }
        let result = match (self.phase, message) {
            (Phase::AwaitingHello, WorkerMessage::Hello { .. }) => {
                self.phase = Phase::AwaitingCapabilities;
                Ok(ManagerEvent::Hello)
            }
            (Phase::AwaitingCapabilities, WorkerMessage::Capabilities { capabilities, .. }) => {
                self.capabilities = Some(capabilities);
                self.phase = Phase::Idle;
                Ok(ManagerEvent::Ready)
            }
            (Phase::Busy, WorkerMessage::Progress { request_id, .. }) => self
                .require_active_request(&request_id, None)
                .map(|()| ManagerEvent::Progress),
            (
                Phase::Busy,
                WorkerMessage::Result {
                    request_id,
                    operation,
                    result,
                    ..
                },
            ) => self.accept_result(request_id, operation, result),
            _ => Err(ManagerError::UnexpectedMessage),
        };
        if result.is_err() {
            self.fail_closed();
        }
        result
    }

    /// Reports whether the owning transport must be killed and reaped before restart.
    pub(crate) fn teardown_required(&self) -> bool {
        self.teardown_required
    }

    fn accept_result(
        &mut self,
        request_id: String,
        operation: WorkerOperation,
        result: WorkerResult,
    ) -> Result<ManagerEvent, ManagerError> {
        self.require_active_request(&request_id, Some(operation))?;
        self.validate_result_relationship(operation, &result)?;
        let event = match result {
            WorkerResult::Completed { outputs: _ } if operation == WorkerOperation::Load => {
                self.loaded_model = self.pending_model.take();
                ManagerEvent::ModelLoaded
            }
            WorkerResult::Completed { outputs } => ManagerEvent::Generated(outputs),
            WorkerResult::Cancelled {} => ManagerEvent::Cancelled,
            WorkerResult::Failed { .. } => ManagerEvent::Failed,
        };
        if operation == WorkerOperation::Load && !matches!(event, ManagerEvent::ModelLoaded) {
            self.pending_model = None;
        }
        self.active = None;
        self.phase = Phase::Idle;
        Ok(event)
    }

    fn validate_result_relationship(
        &self,
        operation: WorkerOperation,
        result: &WorkerResult,
    ) -> Result<(), ManagerError> {
        let WorkerResult::Completed { outputs } = result else {
            return Ok(());
        };
        if operation == WorkerOperation::Load {
            return Ok(());
        }
        let expected = self
            .active
            .as_ref()
            .and_then(|active| active.generation.as_ref())
            .ok_or(ManagerError::ResultMismatch)?;
        let output_names = outputs
            .iter()
            .map(|output| output.output_name.as_str())
            .collect::<HashSet<_>>();
        let outputs_match = outputs.len() == usize::from(expected.count)
            && outputs.len() <= usize::from(expected.max_outputs)
            && output_names.len() == outputs.len()
            && outputs.iter().all(|output| {
                let pixels = u64::from(output.width).saturating_mul(u64::from(output.height));
                let seed_matches = match expected.seed {
                    Some(seed) => output.seed == Some(seed),
                    None if !expected.supports_seed => output.seed.is_none(),
                    None => true,
                };
                output.width == expected.width
                    && output.height == expected.height
                    && pixels <= expected.max_pixels
                    && seed_matches
            });
        if outputs_match {
            Ok(())
        } else {
            Err(ManagerError::ResultMismatch)
        }
    }

    fn require_idle(&self) -> Result<(), ManagerError> {
        match self.phase {
            Phase::Idle => Ok(()),
            Phase::Busy | Phase::Stopping => Err(ManagerError::Busy),
            _ => Err(ManagerError::InvalidState),
        }
    }

    fn require_active_request(
        &self,
        request_id: &str,
        operation: Option<WorkerOperation>,
    ) -> Result<(), ManagerError> {
        let active = self
            .active
            .as_ref()
            .ok_or(ManagerError::UnexpectedMessage)?;
        if active.request_id != request_id
            || operation.is_some_and(|value| value != active.operation)
        {
            Err(ManagerError::CorrelationMismatch)
        } else {
            Ok(())
        }
    }

    fn fail_closed(&mut self) {
        self.reset(true);
    }

    fn reset(&mut self, teardown_required: bool) {
        self.phase = Phase::Stopped;
        self.capabilities = None;
        self.loaded_model = None;
        self.pending_model = None;
        self.active = None;
        self.protocol_session = WorkerProtocolSession::new();
        self.teardown_required = teardown_required;
    }
}

fn validate_outgoing(message: &HostMessage) -> Result<(), ManagerError> {
    encode_host_frame(message)
        .map(|_| ())
        .map_err(|_error: ProtocolError| ManagerError::Protocol)
}
