use super::*;

#[test]
fn downmixes_pcm_and_reports_only_bounded_metadata() {
    let mut state = CaptureState::default();
    state.begin_recording(1_000, 2);
    state.append(&vec![0.5_f32; 2_000]);

    let status = state.status();
    assert_eq!(status.phase, MicrophonePhase::Recording);
    assert_eq!(status.duration_ms, 1_000);
    assert_eq!(status.sample_rate_hz, Some(1_000));
    assert_eq!(status.channels, Some(2));
    assert_eq!(status.retained_byte_size, 4_000);
    assert_eq!(status.input_level, 0.5);
    let json = serde_json::to_value(status).unwrap();
    assert!(json.get("samples").is_none());
    assert!(json.get("device").is_none());
    assert!(json.get("path").is_none());
}

#[test]
fn stops_at_the_shorter_duration_or_memory_limit() {
    let mut duration_limited = CaptureBuffer::with_limits(10, 1, Duration::from_secs(1), 1_024);
    duration_limited.append(&[0.25_f32; 20]);
    assert!(duration_limited.limit_reached());
    assert_eq!(duration_limited.duration_ms(), 1_000);

    let mut memory_limited = CaptureBuffer::with_limits(10, 1, Duration::from_secs(60), 16);
    memory_limited.append(&[0.25_f32; 20]);
    assert!(memory_limited.limit_reached());
    assert_eq!(memory_limited.retained_byte_size(), 16);
    assert_eq!(memory_limited.max_duration_ms(), 400);
}

#[test]
fn maps_permission_and_device_errors_without_backend_text() {
    assert_eq!(
        error_code(ErrorKind::PermissionDenied),
        MicrophoneErrorCode::PermissionDenied
    );
    assert_eq!(
        error_code(ErrorKind::DeviceNotAvailable),
        MicrophoneErrorCode::DeviceUnavailable
    );
    assert_eq!(
        error_code(ErrorKind::DeviceBusy),
        MicrophoneErrorCode::DeviceBusy
    );
    assert_eq!(
        error_code(ErrorKind::BackendError),
        MicrophoneErrorCode::CaptureFailed
    );
}

#[test]
fn discard_clears_every_retained_sample_and_permission_result() {
    let mut state = CaptureState::default();
    state.begin_recording(48_000, 1);
    state.append(&[0.5_f32; 64]);
    state.reset();

    assert_eq!(state.status(), MicrophoneController::default().status());
}
