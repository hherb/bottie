//! Bounded session-only correction for final local transcript turns.

use serde::Serialize;

use super::{
    CaptureState, MicrophoneController, MicrophonePhase, MicrophoneStatus, lock, transcription,
};

pub(super) const MAX_TRANSCRIPT_TURN_BYTES: usize = 512;

/// Stable rejection categories for one bounded session-only transcript correction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TranscriptCorrectionError {
    /// The final stopped transcript is not available for correction.
    TranscriptNotReady,
    /// The requested turn does not exist in the current transcript.
    TurnUnavailable,
    /// The replacement is blank or exceeds a transcript text boundary.
    InvalidText,
}

impl MicrophoneController {
    /// Replaces one final transcript turn in session memory without touching retained PCM.
    pub(crate) fn correct_transcript(
        &self,
        turn_index: usize,
        text: &str,
    ) -> Result<MicrophoneStatus, TranscriptCorrectionError> {
        let mut state = lock(&self.shared);
        state.correct_transcript(turn_index, text)?;
        Ok(state.status())
    }
}

impl CaptureState {
    /// Replaces one final turn while preserving its native timing and the aggregate transcript ceiling.
    pub(super) fn correct_transcript(
        &mut self,
        turn_index: usize,
        text: &str,
    ) -> Result<(), TranscriptCorrectionError> {
        if self.phase != MicrophonePhase::Captured
            || self.transcription_phase != transcription::TranscriptionPhase::Ready
        {
            return Err(TranscriptCorrectionError::TranscriptNotReady);
        }
        let replacement = text.trim();
        if replacement.is_empty() || replacement.len() > MAX_TRANSCRIPT_TURN_BYTES {
            return Err(TranscriptCorrectionError::InvalidText);
        }
        let Some(current) = self.transcript_segments.get(turn_index) else {
            return Err(TranscriptCorrectionError::TurnUnavailable);
        };
        let other_text_bytes = self
            .transcript_segments
            .iter()
            .map(|segment| segment.text.len())
            .sum::<usize>()
            .saturating_sub(current.text.len());
        if other_text_bytes.saturating_add(replacement.len())
            > transcription::MAX_TRANSCRIPT_TEXT_BYTES
        {
            return Err(TranscriptCorrectionError::InvalidText);
        }
        let current = &mut self.transcript_segments[turn_index];
        if !current.is_final {
            return Err(TranscriptCorrectionError::TranscriptNotReady);
        }
        current.text = replacement.to_owned();
        current.is_corrected = true;
        Ok(())
    }
}
