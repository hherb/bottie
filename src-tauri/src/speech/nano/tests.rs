//! Pure failure-boundary tests for Nano installation, PCM framing, and playback buffering.

use super::{buffer::PcmBuffer, protocol::Decoder, runtime::NanoRuntime};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::fs;

#[test]
fn nano_configuration_is_native_absolute_and_never_implicitly_created() {
    let root = std::env::temp_dir().join(format!("bottie-nano-config-{}", uuid::Uuid::new_v4()));
    assert!(NanoRuntime::discover(&root).is_none());
    assert!(!root.exists());
    fs::create_dir_all(root.join("nano-speech")).unwrap();
    fs::write(
        root.join("nano-speech/runtime.json"),
        br#"{"pythonPath":"relative/python","assetDirectory":"/tmp"}"#,
    )
    .unwrap();
    assert!(NanoRuntime::discover(&root).is_none());
    fs::remove_dir_all(root).unwrap();
}

fn audio_event(sequence: usize, values: &[f32]) -> serde_json::Value {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    serde_json::json!({"event":"audio","sequence":sequence,"sampleRate":48000,"channels":2,
        "samplesPerChannel":values.len()/2,"pcm":STANDARD.encode(bytes)})
}

#[test]
fn nano_rejects_reordered_nonfinite_and_mismatched_pcm() {
    let mut decoder = Decoder::default();
    assert!(decoder.audio(audio_event(1, &[0.0, 0.0])).is_err());
    assert!(decoder.audio(audio_event(0, &[f32::NAN, 0.0])).is_err());
    let mut mismatched = audio_event(0, &[0.0, 0.0]);
    mismatched["samplesPerChannel"] = 2.into();
    assert!(decoder.audio(mismatched).is_err());
    assert_eq!(
        decoder.audio(audio_event(0, &[0.25, -0.25])).unwrap(),
        vec![[0.25, -0.25]]
    );
    assert!(
        decoder
            .complete(&serde_json::json!({"audioChunks":1,"audioSeconds":1.0}))
            .is_err()
    );
    assert!(
        decoder
            .complete(&serde_json::json!({"audioChunks":1,"audioSeconds":1.0/48000.0}))
            .is_ok()
    );
}

#[test]
fn nano_buffer_bounds_resamples_and_clears_without_retaining_audio() {
    let mut queue = PcmBuffer::new(4);
    assert!(queue.push(&[[0.0, 0.0]; 5]).is_err());
    queue
        .push(&[[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]])
        .unwrap();
    assert_eq!(queue.next_frame(48000), [0.0, 1.0]);
    assert_eq!(queue.next_frame(48000), [1.0, 0.0]);
    queue.clear();
    assert!(queue.is_empty());
    assert_eq!(queue.next_frame(44100), [0.0, 0.0]);
    queue
        .push(&[[0.0, 0.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0]])
        .unwrap();
    assert_eq!(queue.next_frame(96000), [0.0, 0.0]);
    assert_eq!(queue.next_frame(96000), [0.5, 0.5]);
}

#[test]
fn nano_request_chunks_preserve_unicode_and_worker_text_bounds() {
    let text = "Grüße aus Köln. 你好！ ".repeat(500);
    let chunks = super::protocol::request_chunks(&text);
    assert!(chunks.len() > 1);
    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.len() <= 2000 && !chunk.is_empty())
    );
    let strip = |value: &str| {
        value
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    };
    assert_eq!(strip(&chunks.join("")), strip(&text));
}

#[test]
fn nano_ready_must_bind_exact_assets_and_cpu_only_inference() {
    let lock: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../scripts/moss-nano-spike/assets.lock.json"
    ))
    .unwrap();
    let ready = serde_json::json!({"event":"ready","protocolVersion":1,"torchImported":false,
        "telemetryDisabled":true,"executionProviders":["CPUExecutionProvider"], "voices":["Ava","Adam"],
        "sourceRevision":lock["sourceRevision"],"modelRevision":lock["modelRevision"],"codecRevision":lock["codecRevision"]});
    super::protocol::validate_ready(&ready).unwrap();
    for key in [
        "modelRevision",
        "codecRevision",
        "sourceRevision",
        "executionProviders",
        "telemetryDisabled",
    ] {
        let mut changed = ready.clone();
        changed[key] = serde_json::Value::Null;
        assert!(super::protocol::validate_ready(&changed).is_err());
    }
}

#[tokio::test]
async fn nano_partial_events_remain_bounded_and_cancel_safe() {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    let (mut input, mut output) = tokio::io::duplex(512);
    output.write_all(b"{").await.unwrap();
    let mut buffer = Vec::new();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(5),
            super::process::read_json_line(&mut input, &mut buffer)
        )
        .await
        .is_err()
    );
    assert_eq!(buffer, b"{");
    output.write_all(b"\"event\":\"ready\"}\n").await.unwrap();
    assert_eq!(
        super::process::read_json_line(&mut input, &mut buffer)
            .await
            .unwrap()["event"],
        "ready"
    );
    buffer = vec![b'x'; super::protocol::MAX_EVENT_BYTES + 1];
    assert!(
        super::process::read_json_line(&mut input, &mut buffer)
            .await
            .is_err()
    );
    buffer = vec![b'{'];
    drop(output);
    assert!(
        super::process::read_json_line(&mut input, &mut buffer)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn nano_stalled_output_fails_within_its_deadline() {
    use crate::speech::SpeechErrorCode;
    use std::time::Duration;
    assert!(
        super::service::wait_for_output(|| Ok(false), Duration::from_millis(1))
            .await
            .is_err()
    );
    assert!(
        super::service::wait_for_output(
            || Err(SpeechErrorCode::PlaybackFailed),
            Duration::from_secs(1)
        )
        .await
        .is_err()
    );
    super::service::wait_for_output(|| Ok(true), Duration::from_secs(1))
        .await
        .unwrap();
}

#[cfg(unix)]
#[test]
fn failed_nano_start_is_visible_and_stop_confirms_microphone_admission() {
    use crate::speech::{SpeechController, SpeechPhase};
    let root = std::env::temp_dir().join(format!("bottie-nano-failure-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("nano-speech")).unwrap();
    fs::write(
        root.join("nano-speech/runtime.json"),
        serde_json::to_vec(&serde_json::json!({
            "pythonPath":"/usr/bin/false","assetDirectory":root,
        }))
        .unwrap(),
    )
    .unwrap();
    let backend = super::NanoBackend::discover(&root).unwrap();
    let controller = SpeechController::with_backend(Box::new(backend));
    controller.list_voices().unwrap();
    controller
        .speak("A failed worker must not be hidden.")
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while controller.status().phase != SpeechPhase::Error {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(controller.blocks_microphone_capture());
    controller.stop_before_microphone_capture().unwrap();
    assert!(!controller.blocks_microphone_capture());
    assert_eq!(controller.status().phase, SpeechPhase::Idle);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires explicitly installed Nano runtime and plays synthetic speech through the host output device"]
fn host_nano_native_playback_stop_recovery_and_preference() {
    use crate::speech::{
        SpeechBackend, SpeechController, SpeechPhase, voices::stable_voice_preference,
    };
    use std::time::{Duration, Instant};
    let installed = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/com.bottie.app/nano-speech/runtime.json");
    let root =
        std::env::temp_dir().join(format!("bottie-nano-native-proof-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("nano-speech")).unwrap();
    fs::copy(installed, root.join("nano-speech/runtime.json")).unwrap();
    let mut backend =
        super::NanoBackend::discover(&root).expect("run the explicit Nano setup command first");
    backend.set_voice("bottie-moss-nano-v1:Ava").unwrap();
    backend
        .speak("Hello, this is Bottie. This speech is generated locally on your device.")
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while backend.is_speaking().unwrap() {
        assert!(
            Instant::now() < deadline,
            "native playback did not complete"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let preferred = stable_voice_preference("bottie-moss-nano-v1:Ava");
    let controller =
        SpeechController::with_backend_and_preference(Box::new(backend), Some(preferred.clone()));
    let voices = controller.list_voices().unwrap();
    assert_eq!(controller.selected_voice_preference(), Some(preferred));
    assert!(voices.iter().any(|voice| voice.id
        == controller.status().selected_voice_id.clone().unwrap()
        && voice.name == "MOSS Nano · Ava"));
    controller
        .speak("Guten Tag, hier ist Bottie. Diese Stimme wird direkt auf deinem Gerät erzeugt.")
        .unwrap();
    std::thread::sleep(Duration::from_millis(250));
    assert!(controller.blocks_microphone_capture());
    let stopped = Instant::now();
    controller.stop_before_microphone_capture().unwrap();
    assert!(!controller.blocks_microphone_capture());
    assert!(stopped.elapsed() < Duration::from_secs(5));
    controller
        .speak("Die Wiedergabe funktioniert auch nach einer Unterbrechung.")
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while controller.status().phase == SpeechPhase::Speaking {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(controller.status().phase, SpeechPhase::Idle);
    println!(
        "Native Nano playback, warm reuse, saved preference, stop-before-capture, and restart recovery passed."
    );
    controller.stop_before_microphone_capture().unwrap();
    drop(controller);
    let preference_path = root.join("audio-preferences.json");
    let mut preferences =
        crate::local_audio_preferences::LocalAudioPreferenceStore::load(preference_path.clone());
    preferences
        .remember_speech_voice(Some(stable_voice_preference("bottie-moss-nano-v1:Ava")))
        .unwrap();
    let remembered =
        crate::local_audio_preferences::LocalAudioPreferenceStore::load(preference_path)
            .speech_voice_id();
    let combined = SpeechController::with_local_runtime(remembered, root.clone());
    let voices = combined.list_voices().unwrap();
    assert!(voices.iter().any(|voice| voice.id
        == combined.status().selected_voice_id.clone().unwrap()
        && voice.name == "MOSS Nano · Ava"));
    if let Some(system_voice) = voices
        .iter()
        .find(|voice| !voice.name.starts_with("MOSS Nano"))
    {
        combined.select_voice(&system_voice.id).unwrap();
    }
    let neural = voices
        .iter()
        .find(|voice| voice.name == "MOSS Nano · Ava")
        .unwrap();
    combined.select_voice(&neural.id).unwrap();
    drop(combined);
    fs::remove_dir_all(root).unwrap();
}
