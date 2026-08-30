use super::capture::error_code;
use super::vad::MAX_VOICE_SEGMENTS;
use super::*;
use cpal::ErrorKind;

const TRANSCRIPT_TEXT_LIMIT: usize = 4_000;

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
    assert_eq!(status.voice_activity, Some(VoiceActivity::Speech));
    assert_eq!(status.transcription_phase, TranscriptionPhase::Listening);
    assert!(status.transcript_segments.is_empty());
    assert_eq!(
        status.voice_segments,
        vec![VoiceSegment {
            activity: VoiceActivity::Speech,
            start_ms: 0,
            end_ms: 1_000,
        }]
    );
    let json = serde_json::to_value(status).unwrap();
    assert!(json.get("samples").is_none());
    assert!(json.get("device").is_none());
    assert!(json.get("path").is_none());
    assert!(json.get("threshold").is_none());
    let mut fields = json
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    fields.sort();
    assert_eq!(
        fields,
        [
            "channels",
            "durationMs",
            "errorCode",
            "inputLevel",
            "maxDurationMs",
            "permission",
            "phase",
            "retainedByteSize",
            "sampleRateHz",
            "transcriptSegments",
            "transcriptionErrorCode",
            "transcriptionPhase",
            "voiceActivity",
            "voiceSegments",
        ]
    );
}

#[test]
fn exposes_bounded_partial_and_final_transcript_segments_without_native_metadata() {
    let mut state = CaptureState::default();
    state.begin_recording(16_000, 1);
    state.apply_transcription(
        state.capture_id,
        7,
        false,
        Ok(vec![RawTranscriptSegment {
            text: format!("  hello {}  ", "world ".repeat(900)),
            start_ms: 120,
            end_ms: 820,
        }]),
    );

    let partial = state.status();
    assert_eq!(
        partial.transcription_phase,
        TranscriptionPhase::Transcribing
    );
    assert_eq!(partial.transcript_segments.len(), 1);
    assert!(!partial.transcript_segments[0].is_final);
    assert!(partial.transcript_segments[0].text.len() <= TRANSCRIPT_TEXT_LIMIT);

    state.apply_transcription(
        state.capture_id,
        8,
        true,
        Ok(vec![RawTranscriptSegment {
            text: " final words ".into(),
            start_ms: 100,
            end_ms: 900,
        }]),
    );
    let final_status = state.status();
    assert_eq!(final_status.transcription_phase, TranscriptionPhase::Ready);
    assert_eq!(final_status.transcript_segments[0].text, "final words");
    assert!(final_status.transcript_segments[0].is_final);

    let json = serde_json::to_value(final_status).unwrap().to_string();
    assert!(!json.contains("model_path"));
    assert!(!json.contains("model_hash"));
    assert!(!json.contains("samples"));
    assert!(!json.contains("device"));
}

#[test]
fn ignores_stale_transcription_results_and_clears_transcript_on_discard() {
    let mut state = CaptureState::default();
    state.begin_recording(16_000, 1);
    state.apply_transcription(
        state.capture_id,
        2,
        false,
        Ok(vec![RawTranscriptSegment {
            text: "new result".into(),
            start_ms: 0,
            end_ms: 400,
        }]),
    );
    state.apply_transcription(
        state.capture_id,
        1,
        false,
        Ok(vec![RawTranscriptSegment {
            text: "stale result".into(),
            start_ms: 0,
            end_ms: 200,
        }]),
    );
    assert_eq!(state.status().transcript_segments[0].text, "new result");

    state.reset();
    assert_eq!(state.status().transcription_phase, TranscriptionPhase::Idle);
    assert!(state.status().transcript_segments.is_empty());
}

#[test]
fn detects_bounded_speech_and_silence_segments_with_hysteresis() {
    let mut state = CaptureState::default();
    state.begin_recording(1_000, 1);

    state.append(&[0.0_f32; 200]);
    state.append(&[0.2_f32; 300]);
    state.append(&[0.0_f32; 400]);

    let status = state.status();
    assert_eq!(status.voice_activity, Some(VoiceActivity::Silence));
    assert_eq!(
        status.voice_segments,
        vec![
            VoiceSegment {
                activity: VoiceActivity::Silence,
                start_ms: 0,
                end_ms: 200,
            },
            VoiceSegment {
                activity: VoiceActivity::Speech,
                start_ms: 200,
                end_ms: 500,
            },
            VoiceSegment {
                activity: VoiceActivity::Silence,
                start_ms: 500,
                end_ms: 900,
            },
        ]
    );
}

#[test]
fn ignores_short_level_spikes_and_speech_pauses() {
    let mut state = CaptureState::default();
    state.begin_recording(1_000, 1);

    state.append(&[0.0_f32; 200]);
    state.append(&[0.2_f32; 40]);
    state.append(&[0.0_f32; 200]);
    assert_eq!(state.status().voice_activity, Some(VoiceActivity::Silence));

    state.append(&[0.2_f32; 300]);
    state.append(&[0.0_f32; 100]);
    assert_eq!(state.status().voice_activity, Some(VoiceActivity::Speech));
}

#[test]
fn treats_non_finite_backend_samples_as_silence() {
    let mut state = CaptureState::default();
    state.begin_recording(1_000, 1);
    state.append(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]);

    let status = state.status();
    assert_eq!(status.input_level, 0.0);
    assert_eq!(status.voice_activity, Some(VoiceActivity::Silence));
    assert_eq!(status.voice_segments[0].activity, VoiceActivity::Silence);
}

#[test]
fn bounds_activity_segments_for_the_full_capture_window() {
    let mut state = CaptureState::default();
    state.begin_recording(1_000, 1);
    for _ in 0..240 {
        state.append(&[0.2_f32; 60]);
        state.append(&[0.0_f32; 200]);
    }

    let status = state.status();
    assert_eq!(status.duration_ms, 60_000);
    assert!(status.voice_segments.len() <= MAX_VOICE_SEGMENTS);
    assert_eq!(status.voice_segments.last().unwrap().end_ms, 60_000);
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
    assert!(state.status().voice_segments.is_empty());
}
