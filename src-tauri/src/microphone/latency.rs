//! Monotonic, session-only timing for native microphone lifecycle events.

use std::time::{Duration, Instant};

use serde::Serialize;

/// Small path-free summary of native microphone endpoints observable by Rust.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MicrophoneLatency {
    /// Record request until the input stream successfully starts.
    pub(super) input_ready_ms: Option<u32>,
    /// Record request until the first non-empty local transcript is applied.
    pub(super) first_transcript_ms: Option<u32>,
    /// Stop processing until the final local transcript is applied.
    pub(super) final_transcript_ms: Option<u32>,
}

/// Native-only monotonic anchors plus their bounded public summary.
#[derive(Default)]
pub(super) struct MicrophoneLatencyTracker {
    capture_requested_at: Option<Instant>,
    finalization_requested_at: Option<Instant>,
    summary: MicrophoneLatency,
}

impl MicrophoneLatencyTracker {
    /// Replaces every earlier capture measurement with one new Record action.
    pub(super) fn begin_capture(&mut self, now: Instant) {
        *self = Self {
            capture_requested_at: Some(now),
            ..Self::default()
        };
    }

    /// Records successful input-stream startup without claiming a first sample.
    pub(super) fn mark_input_ready(&mut self, now: Instant) {
        if self.summary.input_ready_ms.is_none() {
            self.summary.input_ready_ms = self
                .capture_requested_at
                .map(|start| elapsed_ms(start, now));
        }
    }

    /// Records the first non-empty local transcript applied for this capture.
    pub(super) fn mark_first_transcript(&mut self, now: Instant) {
        if self.summary.first_transcript_ms.is_none() {
            self.summary.first_transcript_ms = self
                .capture_requested_at
                .map(|start| elapsed_ms(start, now));
        }
    }

    /// Starts the final-transcript interval at Bottie's native Stop handling.
    pub(super) fn begin_finalization(&mut self, now: Instant) {
        if self.finalization_requested_at.is_none() {
            self.finalization_requested_at = Some(now);
            self.summary.final_transcript_ms = None;
        }
    }

    /// Records successful final-transcript application, including an empty transcript.
    pub(super) fn mark_final_transcript(&mut self, now: Instant) {
        self.summary.final_transcript_ms = self
            .finalization_requested_at
            .map(|start| elapsed_ms(start, now));
    }

    /// Returns only bounded integer intervals and no monotonic anchors.
    pub(super) fn summary(&self) -> MicrophoneLatency {
        self.summary
    }
}

/// Saturates a monotonic duration into the small integer IPC boundary.
pub(crate) fn bounded_milliseconds(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

fn elapsed_ms(start: Instant, end: Instant) -> u32 {
    bounded_milliseconds(end.saturating_duration_since(start))
}
