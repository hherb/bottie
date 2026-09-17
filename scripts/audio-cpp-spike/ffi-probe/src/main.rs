//! Spike probe for embedding `audio.cpp` in Bottie through its C ABI.
//!
//! Answers three questions and writes them as JSON, so the evaluation in
//! `docs/audio-cpp-evaluation.md` rests on measurements rather than on the
//! upstream README:
//!
//! 1. Does the C ABI bind from Rust, and is the linked model set only what was
//!    asked for?
//! 2. Can `libaudiocpp`'s statically linked ggml coexist in one process with
//!    whisper-rs's own statically linked ggml, which is what embedding it in the
//!    Tauri binary would require?
//! 3. With weights present, what is the text-to-speech realtime factor on CPU?
//!
//! Question 3 needs a Supertonic 3 package, which is not redistributable through
//! this repository. Point `AUDIOCPP_SPIKE_MODEL` at a local one to include it;
//! without it the probe reports the first two answers and marks the third
//! `null`.

mod ffi;

use std::{
    ffi::{CStr, CString},
    path::PathBuf,
    ptr,
    time::Instant,
};

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Probe {
    abi_version: String,
    build_version: String,
    linked_families: Vec<String>,
    /// True when whisper-rs reported its own context in the same process after
    /// `libaudiocpp` was loaded and used.
    coexists_with_whisper_rs: bool,
    whisper_rs_system_info: String,
    text_to_speech: Option<TextToSpeech>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextToSpeech {
    family: String,
    voice_id: String,
    backend: String,
    threads: i32,
    characters: usize,
    load_seconds: f64,
    synthesis_seconds: f64,
    audio_seconds: f64,
    /// Audio produced per second of wall time. Above 1.0 is faster than realtime.
    realtime_factor: f64,
    sample_rate_hz: i32,
    channels: i32,
    output_wav: String,
    /// What the family asks a streaming caller to push per call. Bottie's
    /// current capture loop recognizes on a fixed 1,500 ms interval, so this is
    /// the number that says whether the two cadences agree.
    streaming: Option<StreamingPolicy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamingPolicy {
    input_kind: i32,
    output_kind: i32,
    preferred_chunk_samples: i64,
    preferred_chunk_seconds: f64,
}

/// Borrowed C string from the ABI. Never NULL by contract; a field a model did
/// not populate reads as `""`.
unsafe fn borrowed(pointer: *const std::os::raw::c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

unsafe fn last_error() -> String {
    unsafe { borrowed(ffi::audiocpp_last_error()) }
}

unsafe fn check(status: ffi::Status, context: &str) -> Result<(), String> {
    if status == ffi::AUDIOCPP_OK {
        return Ok(());
    }
    let (name, detail) = unsafe { (borrowed(ffi::audiocpp_status_string(status)), last_error()) };
    Err(format!("{context}: {name} ({detail})"))
}

fn main() -> Result<(), String> {
    let abi = unsafe { ffi::audiocpp_abi_version() };
    let (major, minor, patch) = (abi >> 16, (abi >> 8) & 0xff, abi & 0xff);
    if major != 0 {
        return Err(format!(
            "probe was written against ABI major 0, library reports {major}"
        ));
    }

    let build_version = unsafe { borrowed(ffi::audiocpp_build_version()) };

    let mut registry: *mut ffi::Registry = ptr::null_mut();
    unsafe {
        check(
            ffi::audiocpp_registry_create(ptr::null(), &mut registry),
            "registry_create",
        )?
    };

    let mut linked_families = Vec::new();
    let count = unsafe { ffi::audiocpp_registry_family_count(registry) };
    for index in 0..count {
        let mut family: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(
                ffi::audiocpp_registry_family(registry, index, &mut family),
                "registry_family",
            )?;
            linked_families.push(borrowed(family));
        }
    }
    linked_families.sort();

    let text_to_speech = match std::env::var_os("AUDIOCPP_SPIKE_MODEL") {
        Some(path) => Some(synthesize(registry, PathBuf::from(path))?),
        None => {
            eprintln!(
                "AUDIOCPP_SPIKE_MODEL is unset, so no weights were loaded and no \
                 realtime factor was measured."
            );
            None
        }
    };

    unsafe { ffi::audiocpp_registry_free(registry) };

    // Same process, after libaudiocpp has been loaded and (when weights were
    // supplied) has already run inference through its own ggml.
    let whisper_rs_system_info = whisper_rs::print_system_info().to_string();

    let probe = Probe {
        abi_version: format!("{major}.{minor}.{patch}"),
        build_version,
        linked_families,
        coexists_with_whisper_rs: !whisper_rs_system_info.is_empty(),
        whisper_rs_system_info,
        text_to_speech,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&probe).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn synthesize(registry: *mut ffi::Registry, model_path: PathBuf) -> Result<TextToSpeech, String> {
    let family = std::env::var("AUDIOCPP_SPIKE_FAMILY").unwrap_or_else(|_| "supertonic".into());
    let voice_id = std::env::var("AUDIOCPP_SPIKE_VOICE").unwrap_or_else(|_| "M1".into());
    let backend = std::env::var("AUDIOCPP_SPIKE_BACKEND").unwrap_or_else(|_| "cpu".into());
    let threads: i32 = std::env::var("AUDIOCPP_SPIKE_THREADS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);
    // Long enough that the realtime factor is not dominated by fixed setup cost.
    let text = std::env::var("AUDIOCPP_SPIKE_TEXT").unwrap_or_else(|_| {
        "Bottie keeps local inference and storage as first-class, while cloud \
         providers remain an explicit choice. This sentence exists so that the \
         measured realtime factor reflects sustained synthesis rather than \
         one-off setup cost."
            .into()
    });
    let output_wav = std::env::var("AUDIOCPP_SPIKE_OUT").unwrap_or_else(|_| "spike-tts.wav".into());

    let model_path_c = cstring(model_path.to_string_lossy().as_ref())?;
    let family_c = cstring(&family)?;
    let config = ffi::ModelConfig {
        family: family_c.as_ptr(),
        config: ptr::null(),
        weight: ptr::null(),
        model_spec_override: ptr::null(),
    };

    let started = Instant::now();
    let mut model: *mut ffi::Model = ptr::null_mut();
    unsafe {
        check(
            ffi::audiocpp_model_load(
                registry,
                model_path_c.as_ptr(),
                &config,
                ptr::null(),
                &mut model,
            ),
            "model_load",
        )?
    };

    let backend_c = cstring(&backend)?;
    let backend_config = ffi::BackendConfig {
        backend: backend_c.as_ptr(),
        device: 0,
        threads,
    };
    let task_c = cstring("tts")?;
    let mode_c = cstring("offline")?;
    let mut session: *mut ffi::Session = ptr::null_mut();
    unsafe {
        check(
            ffi::audiocpp_session_create(
                model,
                task_c.as_ptr(),
                mode_c.as_ptr(),
                &backend_config,
                ptr::null(),
                &mut session,
            ),
            "session_create",
        )?
    };
    let load_seconds = started.elapsed().as_secs_f64();

    let request = unsafe { ffi::audiocpp_request_create() };
    if request.is_null() {
        return Err("request_create returned NULL".into());
    }
    let text_c = cstring(&text)?;
    let language_c = cstring("en")?;
    let voice_key_c = cstring("voice-id")?;
    let voice_c = cstring(&voice_id)?;
    unsafe {
        check(
            ffi::audiocpp_request_set_text(request, text_c.as_ptr(), language_c.as_ptr()),
            "request_set_text",
        )?;
        check(
            ffi::audiocpp_request_set_option(request, voice_key_c.as_ptr(), voice_c.as_ptr()),
            "request_set_option(voice-id)",
        )?;
    }

    let started = Instant::now();
    let mut result: *mut ffi::Result_ = ptr::null_mut();
    unsafe {
        check(
            ffi::audiocpp_session_run(session, request, &mut result),
            "session_run",
        )?
    };
    let synthesis_seconds = started.elapsed().as_secs_f64();

    let mut samples: *const f32 = ptr::null();
    let (mut frames, mut rate, mut channels) = (0usize, 0i32, 0i32);
    unsafe {
        check(
            ffi::audiocpp_result_audio(result, &mut samples, &mut frames, &mut rate, &mut channels),
            "result_audio",
        )?
    };
    if samples.is_null() || frames == 0 || rate <= 0 || channels <= 0 {
        return Err("result_audio returned no usable audio".into());
    }

    // Borrowed until the result is freed, so copy before releasing anything.
    let interleaved =
        unsafe { std::slice::from_raw_parts(samples, frames * channels as usize) }.to_vec();
    let audio_seconds = frames as f64 / rate as f64;

    write_wav(&output_wav, &interleaved, rate as u32, channels as u16)?;

    let streaming = streaming_policy(model, &backend_config);

    unsafe {
        ffi::audiocpp_result_free(result);
        ffi::audiocpp_request_free(request);
        ffi::audiocpp_session_free(session);
        ffi::audiocpp_model_free(model);
    }

    Ok(TextToSpeech {
        family,
        voice_id,
        backend,
        threads,
        characters: text.chars().count(),
        load_seconds,
        synthesis_seconds,
        audio_seconds,
        realtime_factor: audio_seconds / synthesis_seconds,
        sample_rate_hz: rate,
        channels,
        output_wav,
        streaming,
    })
}

/// Asks the family what a streaming caller should push per call. Returns None
/// when the family has no streaming mode, which is not an error.
fn streaming_policy(
    model: *const ffi::Model,
    backend_config: &ffi::BackendConfig,
) -> Option<StreamingPolicy> {
    let task = cstring("tts").ok()?;
    let mode = cstring("streaming").ok()?;
    let mut session: *mut ffi::Session = ptr::null_mut();
    let created = unsafe {
        ffi::audiocpp_session_create(
            model,
            task.as_ptr(),
            mode.as_ptr(),
            backend_config,
            ptr::null(),
            &mut session,
        )
    };
    if created != ffi::AUDIOCPP_OK {
        eprintln!("streaming session unavailable: {}", unsafe { last_error() });
        return None;
    }

    let (mut input_kind, mut output_kind) = (0, 0);
    let (mut chunk_samples, mut chunk_seconds) = (0i64, 0f64);
    let queried = unsafe {
        ffi::audiocpp_stream_policy(
            session,
            &mut input_kind,
            &mut output_kind,
            &mut chunk_samples,
            &mut chunk_seconds,
        )
    };
    unsafe { ffi::audiocpp_session_free(session) };
    if queried != ffi::AUDIOCPP_OK {
        eprintln!("stream_policy failed: {}", unsafe { last_error() });
        return None;
    }

    Some(StreamingPolicy {
        input_kind,
        output_kind,
        preferred_chunk_samples: chunk_samples,
        preferred_chunk_seconds: chunk_seconds,
    })
}

fn cstring(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|error| format!("interior NUL in {value:?}: {error}"))
}

/// 16-bit PCM WAV, so the output can be listened to without another dependency.
fn write_wav(path: &str, samples: &[f32], rate: u32, channels: u16) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    let byte_rate = rate * channels as u32 * 2;

    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let clamped = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&clamped.to_le_bytes());
    }

    std::fs::write(path, bytes).map_err(|error| format!("writing {path}: {error}"))
}
