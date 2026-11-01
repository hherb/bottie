# Voice mode implementation plan

Written 2026-11-01 against commit `118dea43` (2026-10-31). This turns
[`voice-mode-evaluation.md`](voice-mode-evaluation.md) and the measured engine decision in
[`voice-mode-parts/tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) into work against the modules Bottie has today.

Status: phase 0 in progress. 0.1–0.4 are implemented on branch `feat/voice-mode-phase0`; 0.5 (frontend store, controls, auto-submit) remains. The engine and endpointing decisions are made; the open questions are engineering
and are listed per phase.

## 0. Phase map

| Phase | Ships | Engine/deps | Blocked on |
| --- | --- | --- | --- |
| **0** | Hands-free half-duplex conversation: explicit **Start conversation**, automatic end-of-turn, transcript auto-submitted, reply spoken sentence-by-sentence while it streams, explicit **Interrupt** | none — existing OS/Nano speech backends | nothing; the pure primitives are already on `feat/voice-mode-phase0`, wiring is next |
| 1 | Audio owner hardening: one long-lived output owner, resampling, duplex ring buffer, device-change recovery, measured output latency | `rubato`, `ringbuf` | phase 0 |
| 2 | Endpointing upgrade: Silero VAD via `whisper-rs`'s `WhisperVadContext`, 1,000 ms ASR interval, optional `ggml-base-q5_1` | none (new pinned assets) | phase 0; asset contract |
| 3 | Supertonic 3 engine: fp32 weights (owner-approved voice) through `sherpa-onnx`, clause chunker, first-audible target | `sherpa-onnx` + pinned weights | phase 1 for the PCM-owning path; packaging review |
| 4 | Echo cancellation + voice barge-in | per-platform AEC | phase 3; per-platform measurement |
| 5 | Wakeword ("hey bottie", opt-in, armed session) | `tract-onnx`/`ort` + a Bottie-trained classifier | licence-clean front-end provenance |

Phase 0 is the whole deliverable for "better voice mode" in the common case: it removes both clicks and starts speech
before the reply finishes, with no new dependency and no new download. Phases 2–3 make it faster and better; phase 4
makes it interruptible by voice; phase 5 makes it hands-free from idle.

## 1. What exists, and the two facts that shape the work

Grounded in the current tree:

- **Capture** — `MicrophoneController` (`src-tauri/src/microphone.rs`) owns one session slot: `MicrophonePhase`
  (`idle/starting/recording/captured/error`), a 60 s / 32 MiB bound (`MAX_CAPTURE_DURATION`,
  `MAX_RETAINED_BYTES`), and a 40 ms poll (`CAPTURE_WORKER_POLL`). The worker (`microphone/capture.rs`) opens the
  device, downmixes, pushes every sample through the energy VAD (`microphone/vad.rs`, 20 ms frames, 3-frame onset,
  10-frame release), and calls `schedule_transcription(...)` each poll. `Stop` finishes; `Discard` resets.
- **Recognition** — `TranscriptionWorker` (`microphone/transcription.rs`) runs `whisper-rs` on a 1,500 ms interval
  (`TRANSCRIPTION_INTERVAL_MS`), emits bounded partial segments, and the stopped result is final. A worker applies
  transcript state back into `CaptureState`.
- **Speech** — `SpeechController` (`src-tauri/src/speech.rs`) holds one `Mutex<SpeechState>` containing
  `Box<dyn SpeechBackend>`; `SpeechBackend` is `{voices, set_voice, speak, stop, is_speaking}`. `CombinedSpeechBackend`
  routes OS voices and Nano presets. Status is `SpeechPhase { idle, speaking, error }` plus a playback-accepted
  latency.
- **Text stream** — providers call `StreamSink::text_delta/reasoning_delta/usage_updated`
  (`inference/provider.rs`). `ChannelSink` (`stream_channel.rs`) checkpoints each delta into the store and forwards it
  on the Tauri channel. The sink is constructed in `generation.rs` (~line 340) and passed to either
  `stream_native_tools` or `provider.stream_chat`.
- **Admission and cancellation** — `lib.rs` `start_microphone_capture` takes `voice_interaction`, cancels all chats
  and image runs, calls `speech.stop_before_microphone_capture()`, then starts capture. `generation.rs` re-checks
  `microphone.is_capturing()` under the same lock and refuses to start a run during capture. `ActiveRuns` /
  `append` / abort registration own cancellation.
- **Frontend** — `src/routes/page-state.svelte.ts` `sendMessage()` (line ~497) is the single send path, calling
  `startChat` (`src/lib/inference.ts:325`). Capture UI is `src/lib/MicrophoneControl.svelte` with state from
  `src/lib/microphone.ts`; speech UI is `src/lib/speech.ts` + `speech-state.svelte.ts`; `LocalSpeechSettings.svelte`
  is the voice picker.

Two facts force prerequisites:

1. **Chunked playback is blocked by the backend's interrupt semantics, not by the controller lock** (corrected
   2026-11-01 after reading the pinned crates). `tts` 0.26.3's `speak(text, interrupt: bool)` is non-blocking and the
   bool means *interrupt*, not *block*; the controller lock is held only for the dispatch, so `stop` already reaches
   `engine.stop()` during OS playback. What breaks streaming is that `SystemSpeechBackend` hardcodes
   `speak(text, true)` — every chunk would cut off the previous one — and `NanoBackend::speak` returns
   `PlaybackFailed` while its progress is not `Idle`, so Nano cannot queue a second chunk at all. The phase-0 fix is
   therefore a **streaming session that waits for the engine to become idle between chunks** (implemented in
   `speech/streaming.rs`), not an actor refactor. Reactor-shaped engine ownership remains a later option if
   per-chunk idle polling proves too slow; the measured cost is one 20 ms poll gap between chunks at worst.
2. **The capture/playback interlock is the anti-self-hearing guarantee.** `speak_text` fails with
   `MicrophoneActive`; capture stops playback first. A hands-free session must relax device *exclusion* into a
   *processing* gate — the input stream stays open, frames are dropped while Bottie speaks, and the gate lives in the
   audio callback. A leaked frame means Bottie transcribes itself and loops, so this needs a test that asserts zero
   frame delivery while `Speaking`.

## 2. Phase 0 — hands-free half-duplex conversation

### 2.1 State machine

```
Idle
 └─ start_voice_conversation ──► Listening   (capture open, VAD armed)
       Listening ──endpoint──► Finalizing   (capture stops, final transcript)
       Finalizing ──final transcript──► Thinking   (capture closed, generation starts)
       Thinking ──first sentence──► Speaking       (speech session active)
       Speaking ──speech drained──► Listening      (capture re-opens)
       any ──interrupt──► Interrupted ──► Listening
       any ──end_voice_conversation──► Idle
```

Half-duplex by design: capture is closed while Bottie thinks or speaks, so there is **no automatic voice barge-in in
phase 0**. The existing explicit interrupt (button) remains the way to take the turn back, and it must complete within
250 ms. Voice barge-in waits for AEC (phase 4).

### 2.2 Backend changes

**New `src-tauri/src/microphone/endpointing.rs`** — a pure, deterministic `EndPointer`:

```rust
pub(super) enum EndpointDecision { Continue, End { reason: EndReason } }
pub(super) struct EndpointConfig { min_speech_ms: u64, silence_hangover_ms: u64, hard_cap_ms: u64,
                                   stability_confirmations: u8 }
impl EndPointer {
    fn armed(&mut self, now: Instant);
    fn on_activity(&mut self, now: Instant, activity: VoiceActivity) -> EndpointDecision;
    fn on_transcript(&mut self, now: Instant, text: &str) -> EndpointDecision;
}
```

Phase 0 policy (all relative to the **last confirmed speech frame**, never to a transcript result): 480 ms silence
hangover; if the last two partials are identical, finish at 480 ms; if the newest partial ends in `. ? !`, finish at
480 ms; if it is still changing, extend to 750 → 1,000 ms; hard cap 1,200 ms; no transcript and no speech for 700 ms
ends the turn empty (returns to `Listening`). Unit-testable with a fake clock, mirroring `vad.rs` tests.

**`microphone/capture.rs`** — the loop gains one branch: after `schedule_transcription(...)`, consult
`state.endpoint_decision(now)`; on `End`, break with a new `CaptureEnd::AutoStop` that runs the same
`finish()` + final-schedule path as `CaptureCommand::Stop`. No new thread, no new device handle.

**`microphone.rs`** — add `MicrophoneMode { PushToTalk, Conversation }`, `start_conversation()` /
`stop_conversation()`, an `endpoint: Option<EndpointStatus>` field on `MicrophoneStatus` (reason + silence ms +
partial-stability bucket, path-free), and call `conversation.note_transcript(...)` when a partial is applied so the
stability gates can see it. Push-to-talk behaviour is untouched when the mode is `PushToTalk`.

**`speech/streaming.rs` — the streaming driver (implemented 2026-11-01, inert until wired).** It plays chunks strictly
in order through a narrow `SpeechSink` trait (`is_idle`/`speak`/`stop`), waiting for the engine to become idle before
each chunk so neither the OS interrupt semantics nor Nano's busy rejection can cut a chunk off. The session owns one
worker thread, a bounded command queue, and a `SessionProgress` state; interrupts are processed on the worker loop so
they are never blocked behind a waiting chunk. The future `impl SpeechSink for Arc<SpeechController>` is the only
controller-side addition this needs; engine ownership stays where it is.

`SpeechController::speak` itself is unchanged, and its existing tests stand. If per-chunk idle polling ever costs too
much first-audio time, the fallback is to add a queueing `speak_followup(text)` (OS backend calls
`tts.speak(text, false)`) before considering an actor refactor.

`SpeakableReducer` and `Chunker` in the same file are pure and tested:

- `SpeakableReducer` — a streaming-safe Markdown reduction that **never emits a link destination**, holds back
  unterminated constructs (`` ` ``, `[`…`](`, `**`, fences), and clamps its buffer (8 KiB) by flushing at the last
  safe boundary.
- `Chunker` — first chunk ≤ 4 words (hard rule: never split inside a word), then sentence/clause chunks of 12–20
  words; joins are plain concatenation with a single space; measured Supertonic cost model makes this the latency
  lever, not a nicety.
- `StreamingSpeechSession` — bounded mpsc (4 chunks / 8 KiB), one worker thread that calls
  `SpeechController::speak_chunk(text)` sequentially, exposes
  `begin/push_speakable/end/interrupt/progress` and a `spoken_through` index. `interrupt()` flips an atomic, clears
  the queue, and calls the controller's stop; `progress()` is what the UI shows.
- `SpeechTap` — decorates the sink: `text_delta` first delegates to the existing `ChannelSink` (so checkpointing and
  the WebView stream are unchanged), then feeds reducer → chunker → session. Speech failures are logged and disable
  the session for that reply; they must never fail the provider stream.

**`stream_channel.rs` / `generation.rs`** — the minimal wiring that avoids trait plumbing: add
`speech: Option<Arc<StreamingSpeechSession>>` to `ChannelSink`; in `text_delta`, after the existing checkpoint and
channel send, push the delta into the session. In `generation.rs`, construct the sink with
`voice.active_speech_session(&task_run_id)` (Some only for a conversation turn) and end/drop the session when the run
finishes or is aborted. No change to `StreamSink`, providers, or tool streaming.

**`src-tauri/src/voice/mod.rs`** — new `VoiceConversationController` owning the state machine, the arm/disarm
lifecycle, the speech session, admission rules, and the status payload. Proposed commands:
`start_voice_conversation`, `end_voice_conversation`, `interrupt_voice_turn`, `get_voice_conversation_status`.
`lib.rs` registers the controller in `AppState` and the four commands.

Ordering rules, written down because they are the bug surface:

1. `start_voice_conversation` takes `voice_interaction`, cancels active chats/image runs (reusing
   `cancel_all_chats`), awaits `speech` idle (bounded), then starts capture in `Conversation` mode.
2. Automatic endpoint → capture `finish()` → final transcript → state `Finalizing`; the frontend submits only after
   `phase == captured`, so `generation.rs`'s `is_capturing()` check passes.
3. A generation that finishes must `end()` its speech session **and** drain before the controller re-arms capture.
4. `interrupt_voice_turn` = `stop_speech` (bounded) → cancel the run → re-arm capture; never the reverse.
5. Capture admission and playback remain mutually exclusive in `PushToTalk` mode; only `Conversation` uses the
   processing gate.

### 2.3 Frontend changes

- **New `src/lib/voice-conversation.ts`** — status types, command wrappers, and the turn orchestration helpers.
  `INITIAL_VOICE_CONVERSATION_STATUS` follows the existing `INITIAL_*` convention; all payloads are bounded enums and
  counters, no paths/levels/hashes.
- **New `src/routes/voice-conversation-state.svelte.ts`** — the Svelte 5 state store, parallel to
  `microphone-state.svelte.ts` and `speech-state.svelte.ts`. Responsibilities: arm/disarm, watch
  `microphone.status` for `captured + ready`, call the **existing** `sendMessage()` path (it already exists at
  `page-state.svelte.ts:497`; no second send implementation), observe generation completion, re-arm the next turn.
- **`src/lib/MicrophoneControl.svelte`** — a **Start conversation** / **End conversation** action beside Record, and
  the phase vocabulary `Listening`, `Thinking`, `Speaking`, `Interrupted`; Record stays available and unchanged
  outside conversation mode.
- **`src/lib/speech.ts` / `LocalSpeechSettings.svelte`** — the existing voice picker keeps owning which voice
  conversation mode speaks with; the streaming path reads the same selection, so there is one voice preference, not
  two.
- **`src/routes/page.svelte` / `ConversationView.svelte`** — show the conversation state where the composer currently
  shows Record/Stop state, and keep **Interrupt** one visible action (the existing interrupt path: `stop_speech` +
  `cancel_chat`).
- IPC: four commands and one new status object; no PCM, paths, device identity, or engine internals. All additions
  follow the existing bounded/path-free rules.

### 2.4 Tests

Rust: `EndPointer` table tests with fake clocks (done: twelve tests including "no transcript yet", "still changing",
"punctuation", "empty turn", hard-cap-versus-late-transcript); `SpeakableReducer` Markdown cases (done: link
destinations never emitted, fences, unterminated link); `Chunker` tests (done: short first chunk, order-preserving,
no word split); `StreamingSpeechSession` tests (done: in-order drain, interrupt, failure); `ChannelSink` test that a
`text_delta` reaches both the checkpoint and the speech session; capture-loop test with the fake clock for auto-stop;
a self-hearing test asserting zero decoded frames while state is `Speaking`.

Frontend: store tests for the turn cycle (listen → captured → submit → thinking → speaking → listen), interrupt
ordering, and "Send is not called while capture is active"; existing `MicrophoneControl` tests extended for the new
mode.

### 2.5 Acceptance criteria

- Five consecutive turns without a keypress or a new device open, with the OS-voice backend (no Nano, no model
  download).
- Final transcript produced ≤ 1.0 s after end of speech with the phase-0 policy (instrumented; today's interval would
  be ~1.5 s, so this may need the phase-2 interval change pulled forward — measure before promising).
- Speech starts on the first finished sentence while the reply is still generating; no second `speak_text` call.
- Interrupt: user action → silence ≤ 250 ms, measured through the streaming session's interrupt path.
- Self-hearing: zero frames decoded while speaking, and the UI says the mic is muted.
- All existing microphone/speech tests plus the new ones green; no source path or PCM crosses IPC.

## 3. Phase 1 — audio owner hardening

New `src-tauri/src/speech/audio_owner.rs`: one long-lived output stream, a bounded queue (≤2 s), engine-rate PCM with
device-rate conversion at the edge, `rubato` resampling, a duplex ring buffer implemented as **two cpal streams** (cpal
0.18.2 has no duplex API — verified in the pinned crate source), device-change recovery, and output latency measured
from callback timestamps into the existing `SpeechLatency`. Exit: a route change mid-session does not break playback;
no per-sentence device open; measured output latency reported in status.

## 4. Phase 2 — endpointing upgrade

- `microphone/vad_silero.rs` (new): owns `whisper_rs::WhisperVadContext`, resamples to 16 kHz once at the stream edge,
  evaluates a trailing 2,000 ms window every 100 ms on its own thread; exposes one decision per 20 ms frame to the
  endpointer. Energy stays the onset gate; Silero becomes the release gate (the failure mode energy thresholds get
  wrong in rooms).
- `transcription.rs`: `TRANSCRIPTION_INTERVAL_MS` becomes a mode-dependent constant (1,500 ms push-to-talk, 1,000 ms
  conversation), deadline anchored at speech onset, release-triggered final pass; the asset contract becomes an enum
  so `whisperTinyQ5` and `whisperBaseQ5` can both validate.
- `runtime-assets.json`: `sileroVadGgml` (885,098 B, MIT, `ggml-org/whisper-vad` pinned revision) and optionally
  `whisperBaseQ5` (59,707,625 B, same pinned whisper.cpp revision). Both join the existing "download after explicit
  action, verify SHA-256, then load" gate; a missing asset degrades endpointing to the phase-0 policy, never disables
  voice input.
- Exit: false-cut rate measurably below silence-only on recorded turns; end-of-speech → final ≤1 s on the M3 Max and
  the slowest release target.

## 5. Phase 3 — Supertonic 3 through `sherpa-onnx`

Decision: **fp32 weights are the default** (owner-preferred voice; measured RTF 0.1077 through the C++ path, 1.94× the
Python SDK); **int8 is an acceptable compact variant** (145,317,917 B vs 398,652,950 B; +3–8% RTF; owner audited and
accepted). Neither is shipped via the third-party int8 pack: Bottie does a **first-party conversion of the official
fp32 weights** so the provenance chain and the Open RAIL-M notices are unambiguously ours.

- New `src-tauri/src/speech/supertonic/mod.rs`: model contract, conversion helpers (JSON sidecars → the binary forms
  sherpa-onnx expects: int32 LE indexer, 48-byte-header `voice.bin`), generation config (`sid 5` = M1, `seed` pinned,
  threads = 4), and the streaming backend implementation behind the existing streaming trait.
- `runtime-assets.json`: one entry for the official weight set (repository + revision + every file's SHA-256/size,
  Open RAIL-M terms recorded) and one for the converted pack; the conversion is reproducible and tested (the int32
  indexer must come out byte-identical to the reference).
- `Cargo.toml`: `sherpa-onnx` 2.0.1 (or the current release at integration time) with vendored/pinned native
  libraries — the crate's build script otherwise downloads from GitHub, which needs a digest pin for release builds.
- Clause chunker is reused unchanged; the engine's own callback is **not** used for latency (it fires per
  `ChunkText(max_len=300)` chunk), though it can feed the playback queue once a chunk is complete.
- Tests: conversion golden files; a load-and-synthesise smoke behind an ignored test; a seed-pinned regression test on
  a fixed short text (audio digest or LTAS-bound) that catches packaging drift; a Tauri packaging check that the dylib
  is bundled and codesigned.
- Exit: first audible word ≈0.76–1.01 s at a 0.5–0.7 s endpoint in the real pipeline (measured, not derived);
  interrupt within one output callback; Open RAIL notices and the machine-generated-content disclaimer present.

## 6. Phases 4–5 (outline)

- **4 — AEC/voice barge-in.** Render reference from the owned PCM (phase 3 prerequisite), per-platform decision:
  in-process `aec3`, macOS voice-processing I/O (takes over both nodes; abandons cpal), or a documented PipeWire
  requirement on Linux. Windows has no app-callable AEC — keypress-only there. Exit: measured echo-return-loss
  enhancement per platform, barge-in ≤250 ms, false barge-ins below an agreed rate.
- **5 — Wakeword.** Licence-clean front-end provenance, a Bottie-trained "hey bottie" classifier (frozen-embedding
  architecture, 80 ms frames), armed-session TTL and visible state, scoring suspended during playback until AEC
  exists. Exit: measured false-accept rate on ≥5 h of room audio, idle CPU/RSS budget, per-platform qualification.

## 7. Work breakdown and sequencing

| # | Task | Depends on | Size |
| --- | --- | --- | --- |
| 0.1 | ~~`SpeakableReducer`, `Chunker`, `StreamingSpeechSession` + fake-sink tests~~ — done 2026-11-01 (`6cc386b`, `4783c11`) | — | M |
| 0.2 | ~~`EndPointer` + unit tests + capture auto-stop + status fields~~ — done 2026-11-01 (`6cc386b`, `2e1b769`) | 0.1 | M |
| 0.3 | ~~`impl SpeechSink for SpeechController`, `ChannelSink` speech tap, `voice::VoiceSpeech`, generation wiring, `Arc` controller in `AppState`~~ — done 2026-11-01 (`4aeb0eb`) | 0.1 | M |
| 0.4 | ~~`VoiceConversationController` + commands + admission/ordering rules~~ — done 2026-11-01 (`dde1ef4`); frontend is 0.5 | 0.2, 0.3 | M |
| 0.5 | Frontend store + controls + auto-submit + interrupt | 0.4 | M |
| 0.6 | Acceptance run: latency instrumentation, self-hearing test, five-turn script | 0.5 | S |
| 1.1 | Audio owner (queue, resample, duplex ring buffer, device recovery) | 0.6 | L |
| 2.1 | Silero VAD module + assets + interval/anchoring changes | 0.6 | M |
| 2.2 | Optional base-q5_1 checkpoint option | 2.1 | S |
| 3.1 | Supertonic weight contract + first-party conversion + golden tests | 1.1 | M |
| 3.2 | `sherpa-onnx` integration, packaging pin, Tauri dylib bundling/codesign | 3.1 | L |
| 3.3 | End-to-end first-audible measurement + tuning | 3.2 | S |
| 4.x | AEC per platform | 3.3 | L (per platform) |
| 5.x | Wakeword provenance + training + integration | 3.3 (scoring gate) | L |

Sizes are relative (S ≤ a day, M ≈ 2–4 days, L ≈ 1–2 weeks) for one engineer; 0.1–0.6 is the phase-0 slice.

## 8. Risks

| Risk | Where it bites | Mitigation |
| --- | --- | --- |
| Per-chunk idle polling inserts a gap between chunks | 0.3 playback | one 20 ms poll at worst in phase 0; phase 1's audio owner removes the gap, and a queueing `speak_followup` is the fallback |
| Chunked playback cuts off the previous chunk | 0.3 | session waits for idle before each chunk; OS backend's interrupt semantics never see two chunks in flight |
| Mic-gate leak transcribes Bottie's own speech | 0.4/0.6 | gate inside the audio callback, zero-delivery test, UI states the mic is muted |
| Provider token rate makes "first sentence" slow | 0.3/0.6 | measure end-to-end; first chunk is a short clause, so a slow provider only delays that clause |
| `sherpa-onnx-sys` build-time native download is a supply-chain hole | 3.2 | pin the archive digest or vendor it; codesign the bundled dylib; verify on all four targets |
| Open RAIL obligations missed | 3.1 | own the conversion, ship the RAIL text + Attachment A, add the machine-generated disclaimer |
| Seed default is random | 3.1 tests/regressions | pin `extra["seed"]` in every test, cache key, and A/B |
| Endpointing cuts hesitant speakers | 2.1 | tune on recorded turns; hard cap; semantic stage as a later refinement |
| Windows has no AEC | 4.x | per-platform capability matrix; keypress interrupt on Windows |
| 60 s/32 MiB capture bound interacts with long turns | 0.2 | endpoint before bounds, keep the existing limit and surface it, do not raise it silently |

## 9. Non-goals for phase 0

No wakeword, no voice barge-in, no AEC, no change to audio delivery/retention, no new model download, no change to the
provider interface, and no second conversation/send implementation in the frontend. Push-to-talk keeps its current
behaviour exactly; conversation mode is an additional explicit mode.
