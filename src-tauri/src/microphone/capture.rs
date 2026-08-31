//! Native default-input stream ownership for bounded microphone capture.

use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, RecvTimeoutError, SyncSender},
};

use cpal::{
    Device, Error, ErrorKind, FromSample, I24, Sample, SampleFormat, SizedSample, Stream,
    StreamConfig, U24,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use super::{CAPTURE_WORKER_POLL, CaptureCommand, CaptureState, MicrophoneErrorCode, lock};

/// Owns one operating-system input stream and schedules bounded native transcription snapshots.
pub(super) fn capture_worker(
    shared: Arc<Mutex<CaptureState>>,
    receiver: Receiver<CaptureCommand>,
    transcription_wake: Option<SyncSender<()>>,
) {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        lock(&shared).fail(MicrophoneErrorCode::DeviceUnavailable);
        return;
    };
    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            lock(&shared).fail(error_code(error.kind()));
            return;
        }
    };
    let sample_rate_hz = config.sample_rate();
    let channels = config.channels();
    let sample_format = config.sample_format();
    let stream = match build_input_stream(&device, config.config(), sample_format, shared.clone()) {
        Ok(stream) => stream,
        Err(error) => {
            lock(&shared).fail(error);
            return;
        }
    };
    lock(&shared).begin_recording(sample_rate_hz, channels);
    if let Err(error) = stream.play() {
        lock(&shared).fail(error_code(error.kind()));
        return;
    }
    lock(&shared).mark_input_ready();

    let discard = loop {
        if lock(&shared).phase == super::MicrophonePhase::Error || lock(&shared).limit_reached() {
            break false;
        }
        schedule_transcription(&shared, transcription_wake.as_ref(), false);
        match receiver.recv_timeout(CAPTURE_WORKER_POLL) {
            Ok(CaptureCommand::Stop) => break false,
            Ok(CaptureCommand::Discard) | Err(RecvTimeoutError::Disconnected) => break true,
            Err(RecvTimeoutError::Timeout) => {}
        }
    };
    drop(stream);
    let mut state = lock(&shared);
    if discard {
        state.reset();
        return;
    }
    state.finish();
    let scheduled = state.schedule_transcription(true);
    drop(state);
    if scheduled {
        wake(transcription_wake.as_ref());
    }
}

fn schedule_transcription(
    shared: &Arc<Mutex<CaptureState>>,
    transcription_wake: Option<&SyncSender<()>>,
    is_final: bool,
) {
    if lock(shared).schedule_transcription(is_final) {
        wake(transcription_wake);
    }
}

fn wake(sender: Option<&SyncSender<()>>) {
    if let Some(sender) = sender {
        let _ = sender.try_send(());
    }
}

fn build_input_stream(
    device: &Device,
    config: StreamConfig,
    format: SampleFormat,
    shared: Arc<Mutex<CaptureState>>,
) -> Result<Stream, MicrophoneErrorCode> {
    match format {
        SampleFormat::I8 => build_typed_stream::<i8>(device, config, shared),
        SampleFormat::I16 => build_typed_stream::<i16>(device, config, shared),
        SampleFormat::I24 => build_typed_stream::<I24>(device, config, shared),
        SampleFormat::I32 => build_typed_stream::<i32>(device, config, shared),
        SampleFormat::I64 => build_typed_stream::<i64>(device, config, shared),
        SampleFormat::U8 => build_typed_stream::<u8>(device, config, shared),
        SampleFormat::U16 => build_typed_stream::<u16>(device, config, shared),
        SampleFormat::U24 => build_typed_stream::<U24>(device, config, shared),
        SampleFormat::U32 => build_typed_stream::<u32>(device, config, shared),
        SampleFormat::U64 => build_typed_stream::<u64>(device, config, shared),
        SampleFormat::F32 => build_typed_stream::<f32>(device, config, shared),
        SampleFormat::F64 => build_typed_stream::<f64>(device, config, shared),
        _ => Err(MicrophoneErrorCode::UnsupportedFormat),
    }
}

fn build_typed_stream<T>(
    device: &Device,
    config: StreamConfig,
    shared: Arc<Mutex<CaptureState>>,
) -> Result<Stream, MicrophoneErrorCode>
where
    T: Copy + Sample + SizedSample,
    f32: FromSample<T>,
{
    let error_state = shared.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |input, _| lock(&shared).append(input),
            move |error: Error| lock(&error_state).fail(error_code(error.kind())),
            None,
        )
        .map_err(|error| error_code(error.kind()))
}

/// Maps backend-specific audio failures into Bottie's fixed path-free categories.
pub(super) fn error_code(kind: ErrorKind) -> MicrophoneErrorCode {
    match kind {
        ErrorKind::PermissionDenied => MicrophoneErrorCode::PermissionDenied,
        ErrorKind::DeviceNotAvailable | ErrorKind::HostUnavailable => {
            MicrophoneErrorCode::DeviceUnavailable
        }
        ErrorKind::DeviceBusy => MicrophoneErrorCode::DeviceBusy,
        ErrorKind::UnsupportedConfig
        | ErrorKind::UnsupportedOperation
        | ErrorKind::InvalidInput => MicrophoneErrorCode::UnsupportedFormat,
        _ => MicrophoneErrorCode::CaptureFailed,
    }
}
