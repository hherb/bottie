//! Process-lifetime CPAL output ownership for native PCM speech engines.
//!
//! One [`AudioOwner`] keeps the default output device open for as long as the owning engine lives, so an utterance never
//! pays device startup and never races another engine for the same output. It reopens only when the stream failed or
//! the system default device changed. Source PCM is queued in its
//! native format and converted to the device's channel layout and sample rate in the audio callback. A failed stream is
//! reopened on the next request instead of failing the engine permanently.

use cpal::{
    DeviceId, FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use crate::speech::SpeechErrorCode;

/// Bounded queue depth: two seconds of source audio bounds memory and the speech a stop has to discard.
const MAX_QUEUE_SECONDS: u32 = 2;
/// An output device reporting more buffering than this is treated as stalled rather than waited on.
const MAX_DEVICE_LATENCY: Duration = Duration::from_secs(2);
/// Rendered mono reference retained for a later echo canceller.
const MAX_REFERENCE_SECONDS: u32 = 2;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 192_000;
const MAX_DEVICE_CHANNELS: u16 = 8;

/// Source format of one PCM engine's audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AudioFormat {
    pub(super) sample_rate_hz: u32,
    pub(super) channels: u16,
}

struct OutputState {
    queue: PcmQueue,
    failed: bool,
    last_audio_deadline: Option<Instant>,
    /// Measured device buffering for the most recent audible callback, in milliseconds.
    output_latency_ms: Option<u32>,
    /// Mono mix of every device frame this owner rendered, for phase 4's echo cancellation.
    reference: ReferenceBuffer,
}

/// Owns one output stream and a bounded source-rate queue.
///
/// The type is deliberately single-threaded: CPAL streams are not `Send` on every platform, so the engine thread that
/// creates the owner must also push, drain, and drop it.
pub(super) struct AudioOwner {
    stream: Option<Stream>,
    shared: Arc<Mutex<OutputState>>,
    device_id: Option<DeviceId>,
}

impl AudioOwner {
    /// Opens the default output device and starts silence so the first pushed packet plays immediately.
    pub(super) fn new(format: AudioFormat) -> Result<Self, SpeechErrorCode> {
        if !(1..=2).contains(&format.channels)
            || !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&format.sample_rate_hz)
        {
            return Err(SpeechErrorCode::Unavailable);
        }
        let capacity_samples = usize::try_from(format.sample_rate_hz)
            .unwrap_or(usize::MAX)
            .saturating_mul(usize::try_from(MAX_QUEUE_SECONDS).unwrap_or(2))
            .saturating_mul(usize::from(format.channels));
        let shared = Arc::new(Mutex::new(OutputState {
            queue: PcmQueue::new(capacity_samples, format.channels, format.sample_rate_hz),
            failed: false,
            last_audio_deadline: None,
            output_latency_ms: None,
            reference: ReferenceBuffer::new(
                usize::try_from(format.sample_rate_hz).unwrap_or(48_000),
                format.sample_rate_hz,
            ),
        }));
        let mut owner = Self {
            stream: None,
            shared,
            device_id: None,
        };
        owner.open()?;
        Ok(owner)
    }

    /// Appends one complete packet of interleaved source samples, or reports that the queue is full.
    ///
    /// A failed stream is reopened once here; if that reopen fails, its `Unavailable` or `PlaybackFailed` is returned.
    pub(super) fn try_push(&mut self, samples: &[f32]) -> Result<bool, SpeechErrorCode> {
        self.recover_if_failed()?;
        let mut state = lock(&self.shared);
        if state.failed {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        if !state.queue.has_room(samples) {
            return Ok(false);
        }
        state.queue.push_interleaved(samples)?;
        Ok(true)
    }

    /// Reports whether every queued sample has been consumed and the last device buffer has played out.
    pub(super) fn drained(&mut self) -> Result<bool, SpeechErrorCode> {
        self.recover_if_failed()?;
        let state = lock(&self.shared);
        if state.failed {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        Ok(state.queue.is_empty()
            && state
                .last_audio_deadline
                .is_none_or(|end| Instant::now() >= end))
    }

    /// Discards everything queued for the current utterance and picks up a changed default device.
    pub(super) fn clear(&mut self) {
        self.refresh_default_device();
        let mut state = lock(&self.shared);
        state.queue.clear();
        state.last_audio_deadline = None;
    }

    /// Returns a bounded handle to the rendered mono reference for a future echo canceller.
    #[cfg_attr(not(test), allow(dead_code))] // Consumed by phase 4's AEC.
    pub(super) fn reference(&self) -> AudioReference {
        AudioReference {
            shared: self.shared.clone(),
        }
    }

    /// Reopens the output when the operating system's default device changed between utterances.
    ///
    /// Mid-utterance changes surface as stream errors and take the `recover_if_failed` path instead.
    fn refresh_default_device(&mut self) {
        let current = cpal::default_host()
            .default_output_device()
            .and_then(|device| device.id().ok());
        if current.is_none() || current == self.device_id {
            return;
        }
        self.stream = None;
        if self.open().is_err() {
            lock(&self.shared).failed = true;
        }
    }

    /// Returns the most recent measured output latency, if audio has played.
    pub(super) fn output_latency_ms(&self) -> Option<u32> {
        let state = lock(&self.shared);
        state.output_latency_ms
    }

    fn recover_if_failed(&mut self) -> Result<(), SpeechErrorCode> {
        if !lock(&self.shared).failed {
            return Ok(());
        }
        // Drop the failed stream before reopening so the device is released deterministically.
        self.stream = None;
        {
            let mut state = lock(&self.shared);
            state.queue.clear();
            state.last_audio_deadline = None;
        }
        self.open()
    }

    fn open(&mut self) -> Result<(), SpeechErrorCode> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(SpeechErrorCode::Unavailable)?;
        let supported = device
            .default_output_config()
            .map_err(|_| SpeechErrorCode::Unavailable)?;
        let config = supported.config();
        if config.channels == 0
            || config.channels > MAX_DEVICE_CHANNELS
            || !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&config.sample_rate)
        {
            return Err(SpeechErrorCode::Unavailable);
        }
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config, self.shared.clone()),
            SampleFormat::F64 => build::<f64>(&device, config, self.shared.clone()),
            SampleFormat::I16 => build::<i16>(&device, config, self.shared.clone()),
            SampleFormat::I32 => build::<i32>(&device, config, self.shared.clone()),
            SampleFormat::U16 => build::<u16>(&device, config, self.shared.clone()),
            _ => return Err(SpeechErrorCode::Unavailable),
        }?;
        stream.play().map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        {
            let mut state = lock(&self.shared);
            state.failed = false;
            state.reference = ReferenceBuffer::new(
                usize::try_from(config.sample_rate)
                    .unwrap_or(48_000)
                    .saturating_mul(usize::try_from(MAX_REFERENCE_SECONDS).unwrap_or(2)),
                config.sample_rate,
            );
        }
        self.device_id = device.id().ok();
        self.stream = Some(stream);
        Ok(())
    }
}

impl Drop for AudioOwner {
    fn drop(&mut self) {
        let mut state = lock(&self.shared);
        state.queue.clear();
        state.last_audio_deadline = None;
        drop(state);
        // Stream::drop releases the device; pause first so a stopped owner is silent even before teardown.
        if let Some(stream) = self.stream.as_ref() {
            let _ = stream.pause();
        }
        self.stream = None;
    }
}

/// Bounded mono reference of the frames this owner rendered.
struct ReferenceBuffer {
    samples: VecDeque<f32>,
    capacity: usize,
    sample_rate_hz: u32,
    next_sequence: u64,
}

impl ReferenceBuffer {
    fn new(capacity: usize, sample_rate_hz: u32) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity.min(1 << 20)),
            capacity: capacity.max(1),
            sample_rate_hz: sample_rate_hz.max(1),
            next_sequence: 0,
        }
    }

    fn push(&mut self, sample: f32) {
        while self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
        self.next_sequence = self.next_sequence.saturating_add(1);
    }

    fn snapshot(&self) -> ReferenceSnapshot {
        ReferenceSnapshot {
            sample_rate_hz: self.sample_rate_hz,
            samples: self.samples.iter().copied().collect(),
            next_sequence: self.next_sequence,
        }
    }
}

/// One bounded copy of rendered output, newest sample last.
#[cfg_attr(not(test), allow(dead_code))] // Consumed by phase 4's AEC.
pub(super) struct ReferenceSnapshot {
    pub(super) sample_rate_hz: u32,
    pub(super) samples: Vec<f32>,
    /// Sequence of the next sample this buffer will receive; lets a consumer align its own window.
    pub(super) next_sequence: u64,
}

/// Read-only handle to the output owner's rendered reference.
#[cfg_attr(not(test), allow(dead_code))] // Consumed by phase 4's AEC.
pub(super) struct AudioReference {
    shared: Arc<Mutex<OutputState>>,
}

#[cfg_attr(not(test), allow(dead_code))] // Consumed by phase 4's AEC.
impl AudioReference {
    /// Copies the retained reference window.
    pub(super) fn snapshot(&self) -> ReferenceSnapshot {
        lock(&self.shared).reference.snapshot()
    }

    /// Reports how many rendered samples are currently retained.
    pub(super) fn available_samples(&self) -> usize {
        lock(&self.shared).reference.samples.len()
    }
}

/// Bounded native stereo/mono queue with incremental linear sample-rate conversion.
pub(super) struct PcmQueue {
    samples: VecDeque<f32>,
    channels: usize,
    source_rate: u32,
    capacity_samples: usize,
    phase: f64,
}

impl PcmQueue {
    /// Creates a fixed-capacity queue measured in source samples. Sources are mono or stereo; `AudioOwner::new`
    /// validates that before any queue exists.
    pub(super) fn new(capacity_samples: usize, channels: u16, source_rate: u32) -> Self {
        debug_assert!(
            (1..=2).contains(&channels),
            "PCM sources are mono or stereo"
        );
        Self {
            samples: VecDeque::with_capacity(capacity_samples.min(1 << 20)),
            channels: usize::from(channels.max(1)),
            source_rate: source_rate.max(1),
            capacity_samples,
            phase: 0.0,
        }
    }

    /// Returns whether a complete packet fits without exceeding the memory bound.
    pub(super) fn has_room(&self, samples: &[f32]) -> bool {
        !samples.is_empty()
            && samples.len() % self.channels == 0
            && samples.len() <= self.capacity_samples.saturating_sub(self.samples.len())
    }

    /// Accepts one complete validated packet; partial writes are deliberately excluded.
    pub(super) fn push_interleaved(&mut self, samples: &[f32]) -> Result<(), SpeechErrorCode> {
        if !self.has_room(samples) {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        self.samples.extend(samples.iter().copied());
        Ok(())
    }

    /// Writes the next device frame, advancing only available source audio, and reports whether it carried signal.
    pub(super) fn next_frame(&mut self, output_rate: u32, frame: &mut [f32]) -> bool {
        match self.current_frame() {
            Some(source) => {
                write_frame(source, self.channels, frame);
                self.advance(output_rate);
                true
            }
            None => {
                write_frame([0.0; 2], self.channels, frame);
                false
            }
        }
    }

    /// Returns true only after all queued source frames have been consumed.
    pub(super) fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Discards all buffered speech immediately on cancellation or teardown.
    pub(super) fn clear(&mut self) {
        self.samples.clear();
        self.phase = 0.0;
    }

    fn current_frame(&self) -> Option<[f32; 2]> {
        if self.samples.len() < self.channels {
            return None;
        }
        let first = self.frame_at(0)?;
        let next = self.frame_at(1).unwrap_or(first);
        Some(std::array::from_fn(|channel| {
            let a = first[channel];
            let b = next[channel];
            a + (b - a) * self.phase as f32
        }))
    }

    fn frame_at(&self, index: usize) -> Option<[f32; 2]> {
        let start = index.checked_mul(self.channels)?;
        if self.samples.len() < start.saturating_add(self.channels) {
            return None;
        }
        let mut frame = [0.0; 2];
        for (channel, value) in frame.iter_mut().take(self.channels.min(2)).enumerate() {
            *value = *self.samples.get(start + channel)?;
        }
        Some(frame)
    }

    fn advance(&mut self, output_rate: u32) {
        self.phase += f64::from(self.source_rate) / f64::from(output_rate.max(1));
        while self.phase >= 1.0 {
            for _ in 0..self.channels {
                self.samples.pop_front();
            }
            self.phase -= 1.0;
            if self.samples.len() < self.channels {
                self.phase = 0.0;
                break;
            }
        }
    }
}

/// Maps one stereo-or-mono source frame onto the device's channel layout.
fn write_frame(source: [f32; 2], source_channels: usize, frame: &mut [f32]) {
    let mono = if source_channels == 1 {
        source[0]
    } else {
        (source[0] + source[1]) * 0.5
    };
    let stereo_out = frame.len() > 1;
    for (channel, value) in frame.iter_mut().enumerate() {
        *value = match channel {
            0 if !stereo_out => mono,
            0 => source[0],
            1 if source_channels == 1 => source[0],
            1 => source[1],
            // Surround devices receive the mono mix in their extra channels rather than silence.
            _ => mono,
        };
    }
}

/// Builds a typed device stream without allocating in the callback; consumed PCM survives only as the bounded mono
/// reference.
fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    shared: Arc<Mutex<OutputState>>,
) -> Result<Stream, SpeechErrorCode>
where
    T: Sample + SizedSample + FromSample<f32>,
{
    let error_shared = shared.clone();
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    device
        .build_output_stream::<T, _, _>(
            config,
            move |data, info| {
                let Ok(mut state) = shared.try_lock() else {
                    data.fill(T::from_sample(0.0));
                    return;
                };
                let mut frame = [0.0_f32; MAX_DEVICE_CHANNELS as usize];
                let frame_len = frame.len();
                let mut has_audio = false;
                for output in data.chunks_mut(channels.max(1)) {
                    let target = &mut frame[..channels.min(frame_len)];
                    has_audio |= state.queue.next_frame(rate, target);
                    for (value, source) in output.iter_mut().zip(target.iter()) {
                        *value = T::from_sample(*source);
                    }
                    for value in output.iter_mut().skip(target.len()) {
                        *value = T::from_sample(0.0);
                    }
                    // One mono reference sample per device frame, silence included. A callback that loses the lock race
                    // above renders silence without recording it, so the reference can fall behind by that buffer.
                    let mono = target.iter().copied().sum::<f32>() / target.len().max(1) as f32;
                    state.reference.push(mono);
                }
                if has_audio {
                    let timestamp = info.timestamp();
                    let delay = timestamp.playback.duration_since(timestamp.callback);
                    if delay > MAX_DEVICE_LATENCY {
                        state.failed = true;
                        state.queue.clear();
                        return;
                    }
                    let packet_frames = data.len() / channels.max(1);
                    let packet =
                        Duration::from_secs_f64(packet_frames as f64 / f64::from(rate.max(1)));
                    state.last_audio_deadline = Some(Instant::now() + delay + packet);
                    state.output_latency_ms = Some(latency_milliseconds(delay, packet));
                }
            },
            move |_| {
                lock(&error_shared).failed = true;
            },
            None,
        )
        .map_err(|_| SpeechErrorCode::Unavailable)
}

/// Reports the predicted output latency for one buffer as a bounded whole-millisecond value.
fn latency_milliseconds(delay: Duration, packet: Duration) -> u32 {
    let total = delay.saturating_add(packet);
    u32::try_from(total.as_millis()).unwrap_or(u32::MAX)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_bounds_and_rejects_partial_packets() {
        let mut queue = PcmQueue::new(8, 2, 48_000);
        assert!(queue.push_interleaved(&[0.0; 10]).is_err());
        assert!(queue.push_interleaved(&[0.0; 3]).is_err());
        assert!(queue.push_interleaved(&[0.0; 8]).is_ok());
        assert!(queue.push_interleaved(&[0.0; 2]).is_err());
    }

    #[test]
    fn stereo_source_to_mono_device_mixes_both_channels() {
        let mut queue = PcmQueue::new(8, 2, 48_000);
        queue
            .push_interleaved(&[0.0, 1.0, 1.0, 0.0, 0.0, -1.0, -1.0, 0.0])
            .unwrap();
        let mut frame = [0.0_f32; 1];
        assert!(queue.next_frame(48_000, &mut frame));
        assert_eq!(frame, [0.5]);
        assert!(queue.next_frame(48_000, &mut frame));
        assert_eq!(frame, [0.5]);
        queue.clear();
        assert!(!queue.next_frame(48_000, &mut frame));
        assert_eq!(frame, [0.0]);
    }

    #[test]
    fn mono_source_to_stereo_device_duplicates_the_channel() {
        let mut queue = PcmQueue::new(8, 1, 48_000);
        queue.push_interleaved(&[0.25, 0.5]).unwrap();
        let mut frame = [0.0_f32; 2];
        assert!(queue.next_frame(48_000, &mut frame));
        assert_eq!(frame, [0.25, 0.25]);
    }

    #[test]
    fn queue_resamples_with_linear_interpolation() {
        let mut queue = PcmQueue::new(8, 1, 48_000);
        queue.push_interleaved(&[0.0, 1.0]).unwrap();
        let mut frame = [0.0_f32; 1];
        assert!(queue.next_frame(96_000, &mut frame));
        assert_eq!(frame, [0.0]);
        assert!(queue.next_frame(96_000, &mut frame));
        assert_eq!(frame, [0.5]);
        assert!(queue.next_frame(96_000, &mut frame));
        assert_eq!(frame, [1.0]);
    }

    #[test]
    fn rendered_reference_is_bounded_and_unit_mono() {
        let mut reference = ReferenceBuffer::new(3, 48_000);
        for sample in [0.0, 0.25, 0.5, 0.75, 1.0] {
            reference.push(sample);
        }
        let snapshot = reference.snapshot();
        assert_eq!(snapshot.sample_rate_hz, 48_000);
        assert_eq!(snapshot.samples, vec![0.5, 0.75, 1.0]);
        assert_eq!(snapshot.next_sequence, 5);
    }

    #[test]
    fn latency_is_reported_as_whole_milliseconds() {
        assert_eq!(
            latency_milliseconds(Duration::from_millis(12), Duration::from_millis(3)),
            15
        );
        assert_eq!(latency_milliseconds(Duration::ZERO, Duration::ZERO), 0);
    }
}
