# Optional MOSS-TTS-Nano speech

Implemented: 2026-10-18. The owner approved the short English and German Ava audition samples with “These voices sound
great!” This is qualitative approval of those two samples, not a pronunciation assessment of every language, numeric
fixture, preset, or long-form chunk boundary.

## Use Nano in Bottie

Run the explicit setup once with Python 3.12, then restart an updated Bottie build:

```sh
python3.12 scripts/moss-nano-spike/install.py
```

Choose **Settings → Local speech voice → MOSS Nano · Ava** or **MOSS Nano · Adam**. Use the existing **Play response
aloud** action. System voices stay available, and the choice is saved through Bottie's existing opaque native
preference. The first Nano reply loads the model. Later replies reuse the worker until an active Stop, a voice switch,
interruption, an error, or app shutdown tears it down. An idle worker may remain resident during microphone capture. Generated speech is not saved as a recording.

Setup explicitly downloads the approximately 763 MB pinned asset inventory plus six pinned Python packages. It creates
a dedicated environment in app data and reuses compatible registered environments on subsequent runs. Nothing is
downloaded or installed automatically by the app. A missing installation leaves system voices available; a selected
Nano runtime failure returns the existing fixed speech error state instead of silently switching engines.

To reuse the verified trial cache without another model download:

```sh
python3.12 scripts/moss-nano-spike/install.py --source-cache /absolute/path/to/trial-assets
```

`--reuse-python /absolute/path/to/venv/bin/python` and `--reuse-assets /absolute/path/to/assets` register existing
verified installations in place. These remain external native dependencies; keep them at their registered locations.
`--app-data` supports an explicit Bottie app-data location, including a sandbox-container installation. The default is
`~/Library/Application Support/com.bottie.app` on macOS, `%APPDATA%/com.bottie.app` on Windows, and
`$XDG_DATA_HOME/com.bottie.app` (normally `~/.local/share/com.bottie.app`) on Linux. Restart is required after setup.

## Native ownership and bounds

`SpeechController` routes the closed Nano presets and existing OS voices through the same bounded metadata and
preference contract. Model paths, Python paths, process identities, PCM, and utterance state remain native. There is
no WebView installation/process command, provider endpoint, audio API permission, or generated-audio retention path.
The app's exact embedded worker scripts are materialized under a content-derived directory before execution; the
worker verifies every code/model/codec asset against the immutable trial lock before loading it. Python runs in
isolated mode, only ONNX CPU sessions are accepted, telemetry is disabled, and the Python socket audit guard stays
enabled. That guard is not an operating-system network sandbox or a complete native network-containment proof.

One dedicated Rust actor owns the persistent Python child and CPAL output. It validates newline framing, model/code
revisions, CPU capability, ordered float32 PCM, finite samples, alignment, sample counts, and completion. Unexpected
EOF, malformed output, an empty result, frame limits, or mismatched counts fail the utterance. Stdout events are capped
at 600 KiB, decoded packets at one second, and the native queue at two seconds of 48 kHz stereo (768,000 bytes). Output
uses the default native device, converts to its sample rate/channel count, and waits through the last predicted device
playback deadline before reporting idle. No acoustic timing or echo-cancellation claim is made.

Controller text remains capped at 32 KiB. Rust splits it into at most 2,000 UTF-8 bytes per worker request; the worker
then splits by a 75-token limit and retains its 375-frame ceiling per text chunk. Oversized generation is an error,
not a claimed completion. Startup has a 60-second deadline, complete events 45 seconds, private writes five seconds,
stalled output five seconds, and a whole utterance fifteen minutes. Stop has a separate cancellation signal, so neither
backpressure nor a pending worker read can delay it behind queued commands. The output stream and queue are dropped
before the child is killed/reaped. A stop that cannot confirm teardown within five seconds keeps microphone admission
fail-closed. Capture and speech-start registration share the existing voice-interaction mutex.

The system backend can be unavailable independently of Nano. Voice enumeration reserves space for the two Nano
presets while retaining deterministic bounded system ordering. Switching engines attempts both stops and propagates
a failed stop. App shutdown also tears down an idle resident model. Nano is separate from the sandboxed user-code
`run_python` runtime and from the image-worker runtime.

## Verification and remaining qualification

The macOS arm64 host check plays fixed synthetic English and German through real CPAL output, verifies reuse of the
ready worker, restores the opaque preference, stops before capture admission, and completes playback after a fresh
worker restart. It is opt-in because it plays audio:

```sh
cargo test --manifest-path src-tauri/Cargo.toml \
  host_nano_native_playback_stop_recovery_and_preference --lib -- --ignored --nocapture
python3 -m unittest discover -s scripts/moss-nano-spike
```

The initial sandboxed playback check failed; the identical host-access retry passed. Browser preview inspection uses
the explicit development-only `?voice=nano-voices` fixture and checks the real Settings component at desktop and
620 × 800 layouts. It proves labels and
layout, not native Tauri clicks or audio. Native controller/host tests provide the playback evidence; native UI click
automation is not claimed. The original [trial measurements](moss-nano-evaluation.md) remain separate.

Final checks pass: 428 frontend/script tests (three existing skips), 18 Python boundary/setup tests, clean Svelte
diagnostics, Prettier, the production frontend build, Cargo formatting, and locked Cargo checking. The serial host
Rust suite passes 700 library tests (40 ignored), one updater fixture, six adapter fixtures, ten transport fixtures,
and doc tests. The sandbox-only keychain failure passes in the identical host retry. Saved voice discovery completes
before the UI reads restored status, with regression coverage for that startup ordering.

Linux/Windows runtime availability, packaging, device behavior, and native playback remain unqualified. This integration
uses an explicitly provisioned external Python runtime and model cache; neither is bundled into the application
installer. The normal contained `run_python` runtime remains included in application builds. A complete Nano runtime
distribution/voice review and signed release qualification are separate work. The Apache-2.0 declarations and source
licence are retained with the locked assets; this document is not a new distribution approval.
