//! A bounded native stereo queue with incremental linear sample-rate conversion.

use super::protocol::SOURCE_RATE;
use crate::speech::SpeechErrorCode;
use std::collections::VecDeque;

/// Keeps source frames native and produces silence on underrun without retaining consumed speech.
pub(super) struct PcmBuffer {
    frames: VecDeque<[f32; 2]>,
    capacity: usize,
    phase: f64,
}

impl PcmBuffer {
    /// Creates a fixed-capacity queue measured in 48 kHz stereo frames.
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity),
            capacity,
            phase: 0.0,
        }
    }
    /// Returns whether a whole decoded packet can be appended without exceeding the memory bound.
    pub(super) fn has_room(&self, count: usize) -> bool {
        count <= self.capacity.saturating_sub(self.frames.len())
    }
    /// Accepts one complete validated packet; partial writes are deliberately excluded.
    pub(super) fn push(&mut self, frames: &[[f32; 2]]) -> Result<(), SpeechErrorCode> {
        if !self.has_room(frames.len()) {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        self.frames.extend(frames);
        Ok(())
    }
    /// Returns the next output stereo frame at the device's rate, advancing only available source audio.
    pub(super) fn next_frame(&mut self, output_rate: u32) -> [f32; 2] {
        let Some(first) = self.frames.front().copied() else {
            self.phase = 0.0;
            return [0.0; 2];
        };
        let next = self.frames.get(1).copied().unwrap_or(first);
        let mixed = std::array::from_fn(|channel| {
            first[channel] + (next[channel] - first[channel]) * self.phase as f32
        });
        self.phase += f64::from(SOURCE_RATE) / f64::from(output_rate.max(1));
        while self.phase >= 1.0 {
            self.frames.pop_front();
            self.phase -= 1.0;
            if self.frames.is_empty() {
                self.phase = 0.0;
                break;
            }
        }
        mixed
    }
    /// Returns true only after all queued source frames have been consumed.
    pub(super) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    /// Discards all buffered speech immediately on cancellation or teardown.
    pub(super) fn clear(&mut self) {
        self.frames.clear();
        self.phase = 0.0;
    }
}
