//! Focused contracts for bounded session-only local speech playback.

use super::*;

#[test]
fn normalizes_spoken_text_without_retaining_markup_whitespace() {
    assert_eq!(
        normalize_speech_text("  Hello\n\n  local\tvoice.  "),
        "Hello local voice."
    );
}

#[test]
fn rejects_blank_and_oversized_speech_text() {
    assert_eq!(
        validated_speech_text(" \n "),
        Err(SpeechCommandError::InvalidText)
    );
    assert_eq!(
        validated_speech_text(&"é".repeat(MAX_SPEECH_TEXT_BYTES)),
        Err(SpeechCommandError::TextTooLong),
    );
}

#[test]
fn exposes_only_bounded_path_free_voice_metadata() {
    let voices = bounded_voices(vec![
        NativeSpeechVoice {
            id: "voice.zulu".into(),
            name: "Zulu".into(),
            language: "en-ZA".into(),
        },
        NativeSpeechVoice {
            id: "voice.zulu".into(),
            name: "Duplicate native identity".into(),
            language: "aa-AA".into(),
        },
        NativeSpeechVoice {
            id: "voice.alpha".into(),
            name: "Alpha".into(),
            language: "en-AU".into(),
        },
        NativeSpeechVoice {
            id: "voice.control".into(),
            name: "Unsafe\0Name".into(),
            language: "zz\nZZ".into(),
        },
        NativeSpeechVoice {
            id: "x".repeat(MAX_NATIVE_VOICE_ID_BYTES + 1),
            name: "Oversized identity".into(),
            language: "zz-ZZ".into(),
        },
    ]);

    assert_eq!(voices.len(), 3);
    assert_eq!(voices[0].0.id, "local-voice-001");
    assert_eq!(voices[0].1, "voice.alpha");
    assert_eq!(voices[1].0.id, "local-voice-002");
    assert_eq!(voices[1].1, "voice.zulu");
    assert_eq!(voices[2].0.name, "Unsafe Name");
    assert_eq!(voices[2].0.language, "zz ZZ");
    assert_eq!(voices[2].1, "voice.control");
    let public_voices = voices
        .into_iter()
        .map(|(voice, _)| voice)
        .collect::<Vec<_>>();
    let serialized = serde_json::to_value(&public_voices).unwrap();
    let object = serialized[0].as_object().unwrap();
    assert_eq!(
        object.keys().cloned().collect::<Vec<_>>(),
        ["id", "language", "name"]
    );
    assert!(!serialized.to_string().to_ascii_lowercase().contains("path"));
}

#[test]
fn controller_selects_voices_plays_and_stops_without_echoing_text() {
    let backend = RecordingBackend::default();
    let record = backend.record.clone();
    let controller = SpeechController::with_backend(Box::new(backend));

    let voices = controller.list_voices().unwrap();
    assert_eq!(voices.len(), 2);
    assert_eq!(
        controller.status().selected_voice_id.as_deref(),
        Some("local-voice-001"),
    );

    let selected = controller.select_voice("local-voice-002").unwrap();
    assert_eq!(
        selected.selected_voice_id.as_deref(),
        Some("local-voice-002")
    );
    assert_eq!(
        record.lock().unwrap().selected_voice.as_deref(),
        Some("voice.two")
    );

    let speaking = controller.speak(" Read **this** locally. ").unwrap();
    assert_eq!(speaking.phase, SpeechPhase::Speaking);
    assert_eq!(
        record.lock().unwrap().spoken,
        vec!["Read **this** locally."]
    );
    let serialized = serde_json::to_value(&speaking).unwrap().to_string();
    assert!(!serialized.contains("Read"));

    assert_eq!(controller.stop().phase, SpeechPhase::Idle);
    assert_eq!(record.lock().unwrap().stop_count, 1);
}

#[test]
fn rejected_voice_and_text_leave_existing_selection_unchanged() {
    let controller = SpeechController::with_backend(Box::new(RecordingBackend::default()));
    controller.list_voices().unwrap();

    assert_eq!(
        controller.select_voice("missing"),
        Err(SpeechCommandError::VoiceNotFound),
    );
    assert_eq!(
        controller.status().selected_voice_id.as_deref(),
        Some("local-voice-001"),
    );
    assert_eq!(controller.speak("  "), Err(SpeechCommandError::InvalidText));
    assert_eq!(controller.status().phase, SpeechPhase::Idle);
}

#[test]
#[ignore = "requires the host operating system's local speech engine"]
fn host_local_speech_engine_lists_bounded_voices_without_playing_audio() {
    let controller = SpeechController::default();
    let voices = controller
        .list_voices()
        .expect("the host local speech engine should enumerate voices");

    assert!(!voices.is_empty());
    assert!(voices.len() <= MAX_SPEECH_VOICES);
    assert!(
        voices
            .iter()
            .all(|voice| voice.id.starts_with("local-voice-"))
    );
    assert_eq!(controller.status().phase, SpeechPhase::Idle);
}

#[derive(Default)]
struct RecordingBackend {
    record: std::sync::Arc<std::sync::Mutex<BackendRecord>>,
}

#[derive(Default)]
struct BackendRecord {
    selected_voice: Option<String>,
    spoken: Vec<String>,
    stop_count: usize,
    speaking: bool,
}

impl SpeechBackend for RecordingBackend {
    fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode> {
        Ok(vec![
            NativeSpeechVoice {
                id: "voice.two".into(),
                name: "Second".into(),
                language: "en-US".into(),
            },
            NativeSpeechVoice {
                id: "voice.one".into(),
                name: "First".into(),
                language: "en-AU".into(),
            },
        ])
    }

    fn set_voice(&mut self, voice_id: &str) -> Result<(), SpeechErrorCode> {
        self.record.lock().unwrap().selected_voice = Some(voice_id.into());
        Ok(())
    }

    fn speak(&mut self, text: &str) -> Result<(), SpeechErrorCode> {
        let mut record = self.record.lock().unwrap();
        record.spoken.push(text.into());
        record.speaking = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), SpeechErrorCode> {
        let mut record = self.record.lock().unwrap();
        record.stop_count += 1;
        record.speaking = false;
        Ok(())
    }

    fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode> {
        Ok(self.record.lock().unwrap().speaking)
    }
}
