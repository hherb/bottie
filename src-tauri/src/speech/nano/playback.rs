//! CPAL output ownership: bounded native PCM, device-rate conversion, and explicit drain/stop.

use super::{buffer::PcmBuffer, protocol::SOURCE_RATE};
use crate::speech::{SpeechErrorCode, lock};
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const QUEUE_FRAMES: usize = SOURCE_RATE as usize * 2;
const MAX_DEVICE_LATENCY: Duration = Duration::from_secs(2);

struct OutputState {
    queue: PcmBuffer,
    failed: bool,
    last_audio_deadline: Option<Instant>,
}

/// Lives only on the dedicated speech thread and drops the OS output stream before stop completes.
pub(super) struct NativePlayback {
    stream: Stream,
    shared: Arc<Mutex<OutputState>>,
}

impl NativePlayback {
    /// Opens only the default output device and starts silence while inference prepares PCM.
    pub(super) fn new() -> Result<Self, SpeechErrorCode> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(SpeechErrorCode::Unavailable)?;
        let supported = device
            .default_output_config()
            .map_err(|_| SpeechErrorCode::Unavailable)?;
        let config = supported.config();
        if config.channels == 0
            || config.channels > 8
            || !(8_000..=192_000).contains(&config.sample_rate)
        {
            return Err(SpeechErrorCode::Unavailable);
        }
        let shared = Arc::new(Mutex::new(OutputState {
            queue: PcmBuffer::new(QUEUE_FRAMES),
            failed: false,
            last_audio_deadline: None,
        }));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config, shared.clone()),
            SampleFormat::F64 => build::<f64>(&device, config, shared.clone()),
            SampleFormat::I16 => build::<i16>(&device, config, shared.clone()),
            SampleFormat::I32 => build::<i32>(&device, config, shared.clone()),
            SampleFormat::U16 => build::<u16>(&device, config, shared.clone()),
            _ => return Err(SpeechErrorCode::Unavailable),
        }?;
        stream.play().map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        Ok(Self { stream, shared })
    }

    /// Appends a packet only when there is room; the producer waits rather than growing the buffer.
    pub(super) fn try_push(&self, frames: &[[f32; 2]]) -> Result<bool, SpeechErrorCode> {
        let mut state = lock(&self.shared);
        if state.failed {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        if !state.queue.has_room(frames.len()) {
            return Ok(false);
        }
        state.queue.push(frames)?;
        Ok(true)
    }

    /// Waits through the last OS-predicted playback deadline instead of truncating the last device buffer.
    pub(super) fn drained(&self) -> Result<bool, SpeechErrorCode> {
        let state = lock(&self.shared);
        if state.failed {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        Ok(state.queue.is_empty()
            && state
                .last_audio_deadline
                .is_none_or(|end| Instant::now() >= end))
    }
}

impl Drop for NativePlayback {
    fn drop(&mut self) {
        let mut state = lock(&self.shared);
        state.queue.clear();
        state.last_audio_deadline = None;
        drop(state);
        let _ = self.stream.pause();
    }
}

/// Builds a typed device stream without allocating in the callback or retaining consumed PCM.
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
                let mut has_audio = false;
                for frame in data.chunks_mut(channels) {
                    has_audio |= !state.queue.is_empty();
                    let sample = state.queue.next_frame(rate);
                    for (channel, value) in frame.iter_mut().enumerate() {
                        let value_f32 = if channels == 1 {
                            (sample[0] + sample[1]) * 0.5
                        } else if channel < 2 {
                            sample[channel]
                        } else {
                            0.0
                        };
                        *value = T::from_sample(value_f32);
                    }
                }
                if has_audio {
                    let timestamp = info.timestamp();
                    let delay = timestamp.playback.duration_since(timestamp.callback);
                    if delay > MAX_DEVICE_LATENCY {
                        state.failed = true;
                        state.queue.clear();
                        return;
                    }
                    let packet =
                        Duration::from_secs_f64((data.len() / channels) as f64 / f64::from(rate));
                    state.last_audio_deadline = Some(Instant::now() + delay + packet);
                }
            },
            move |_| {
                lock(&error_shared).failed = true;
            },
            None,
        )
        .map_err(|_| SpeechErrorCode::Unavailable)
}
