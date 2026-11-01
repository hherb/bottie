# Voice mode: local STT and TTS options

Last reviewed: 2026-11-01

This document evaluates local speech-to-text and text-to-speech options for a better Bottie voice mode. It is an
evaluation of options, not a decision and not a commitment to integrate. Nothing here changes what Bottie ships;
Milestone 7 remains the current voice implementation.

The question came from the product owner: Bottie needs a better voice mode, the investigation should cover local
STT and TTS, keep the inference model free to be any (non-voice) model, and pay particular attention to

1. automatic wakeword recognition — hands-free activation rather than pressing **Record**;
2. automatic detection of when voice input ends — endpointing, so the user does not press **Stop**;
3. streaming spoken responses — speech begins while the reply is still being generated.

The investigation ran as four parallel research slices plus repository grounding. Their working notes are in
[`voice-mode-parts/`](voice-mode-parts/): [conventions](voice-mode-parts/CONVENTIONS.md),
[wakeword](voice-mode-parts/wakeword.md), [STT and endpointing](voice-mode-parts/stt-endpointing.md),
[streaming TTS](voice-mode-parts/tts-streaming.md), and [pipeline architecture](voice-mode-parts/pipeline-architecture.md).
Those parts carry the full candidate tables, per-candidate citations, per-module integration sketches, and source
lists; this document is the merged position. The research pass downloaded and measured nothing; a follow-up
measurement of three TTS engines on this host is recorded in
[`voice-mode-parts/tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) with audition samples in
[`voice-mode-parts/samples/`](voice-mode-parts/samples/). The implementation plan that turns this document into work
against the existing modules is [`voice-mode-implementation-plan.md`](voice-mode-implementation-plan.md). Every number
below is labelled, and the unlabelled latency figures in the plan are estimates composed from labelled pieces.

## Summary

**The three requested capabilities are independently adoptable, and their costs are wildly different.** The cheapest
and highest-impact change is *orchestration*: keep the current recogniser, decide the end of a turn natively, and let
speech start on the first finished sentence instead of the whole reply. That needs no new dependency, no new model,
and no new download. The most expensive and least certain change is the wakeword, and it was blocked on a licensing
question rather than on engineering — a question the owner has since ruled on (see below and
[the measured comparison](voice-mode-parts/tts-head-to-head.md)).

**Owner ruling, 2026-11-01:** the Supertonic weights' Open RAIL conditions are acceptable, and GPL components are not
a hindrance. That removes those licences as the *first* selection criterion and turns them into a recorded set of
obligations per engine. **It does not waive dataset licences, which are a separate gate**: the Piper voice the owner liked
best, `en_US-lessac-medium`, is trained on Blizzard 2013 Lessac data under a research-only, non-transferable licence
that excludes developing voice-synthesis products, so it cannot ship (see
[`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) §4). The repository itself remains MIT (`LICENSE`,
`package.json`, `src-tauri/Cargo.toml`), and no relicense is implied.

**Owner audition, 2026-11-01:** Supertonic 3 and Piper `en_US-lessac-medium` sounded superior; Kokoro fp32 was
acceptable; Kokoro int8 and Piper `en_US-ljspeech-medium` were rejected. A second audition of the three redistributable
Piper voices found none good (cori-high the least bad), and Lessac cannot be redistributed, so Piper is out.
**Supertonic 3 is the default**, Kokoro fp32 the licence-clean fallback.

| Capability | Recommendation | New dependencies | New assets | Risk |
| --- | --- | --- | --- | --- |
| Automatic end of speech | Phase 0: native end-of-turn controller over the existing 20 ms VAD — a 480–700 ms turn-level silence hangover, transcript-stability gates, 1,200 ms hard cap. Phase 2: Silero VAD through the **already-linked** `whisper-rs` VAD API | none | phase 0 none; phase 2 `ggml-silero-v6.2.0.bin`, 885,098 B, MIT | low |
| Accuracy step for recognition | Keep `whisper-rs` and add `ggml-base-q5_1.bin` beside tiny, same pinned repository revision, as an explicit user-visible option | none | 59,707,625 B, MIT | low |
| Streaming spoken responses | Phase 0: tap the existing native `StreamSink` token stream, reduce to speakable text, chunk clause-first then by sentence, and synthesize per chunk through the existing speech backend. Phase 3: **Supertonic 3** default through the validated `sherpa-onnx` C++ path (399 MB, RTF 0.108 → 9.3× realtime, ≈0.76–1.01 s to first audible with a 1–4-word opening clause; 0.93–1.16 s on the archived SDK; own clause chunker); **Kokoro-82M fp32** fallback (354 MB, 4.3× realtime, Apache weights); **Piper** rejected (best voice unshippable, clean voices failed the audition) | phase 0 none; phase 3 `ort` (already in the graph) or `sherpa-onnx` | phase 0 none; phase 3 one pinned engine pack, 63–399 MB | medium |
| Automatic wakeword | In-process ONNX detector on the frozen-embedding architecture; **"hey bottie" classifier is a Bottie build artifact**, because every off-the-shelf option fails on licence or needs an activation key. Provenance of the two shared front-end files must be cleared first | `tract-onnx` (pure Rust) or the existing `ort` | ≈ 2.4 MB front-end + ≈ 1 MB classifier | high |
| Hands-free / barge-in | Half-duplex hands-free conversation first; full-duplex voice barge-in only after Bottie owns its rendered PCM **and** a per-platform echo-cancellation decision exists | `rubato`, `ringbuf` for the audio owner | none | high |

If only one thing is done, do the streaming playback path plus turn-level endpointing: together they turn today's
five-action exchange (Record → Stop → Use transcript → Send → Play) into "start a conversation and talk", with no new
download, and they are the prerequisite for everything else.

## What Bottie has today

Voice works as push-to-talk, not as a conversation. The current contract is described in
[`README.md`](../README.md#local-voice-capture) and was delivered by Milestone 7 in [`ROADMAP.md`](../ROADMAP.md):

- **Capture and VAD.** The explicit **Record** action opens the selected input through `cpal`, downmixes into one
  bounded 60-second, 32 MiB in-memory session capture, and runs a native energy detector over 20 ms frames with
  separate onset (3 frames) and release (10 frames) confirmation (`src-tauri/src/microphone/vad.rs`). The WebView
  sees only phase, level, duration, and speech/silence timing.
- **Recognition.** A Rust worker transcribes speech-containing snapshots with `whisper-rs` `=0.16.0` against
  whisper.cpp multilingual `ggml-tiny-q5_1.bin` (32,152,673 bytes, pinned in
  [`runtime-assets.json`](../runtime-assets.json)) on a fixed 1,500 ms interval, and the stopped result is final. The
  transcript is a list of bounded ranges the user may correct; at most 4,000 UTF-8 bytes are exposed.
- **Text handling.** **Use transcript as text** copies corrected turns into the composer draft and never submits them.
  **Send recording** requires a model that advertises audio input; **Retain locally** stores a native-only WAV.
  Neither is on by default.
- **Playback.** **Play response aloud** speaks at most 32 KiB of visible Markdown-derived text through the `tts`
  crate `=0.26.3` — Speech Dispatcher with eSpeak NG voices on Linux, installed system voices on macOS and Windows.
  An optional local MOSS-TTS-Nano worker adds two higher-quality voices with streaming PCM playback.
- **Interruption.** While Bottie is speaking or generating, **Record** becomes **Interrupt & record**: cancel the
  run, stop playback, then capture. Capture and playback are mutually exclusive, and that interlock is the current
  anti-self-hearing guarantee.
- **Streaming text already exists.** Providers stream normalized deltas through the `StreamSink` trait
  (`src-tauri/src/inference/provider.rs`) into `ChannelSink`, which checkpoints each delta into the conversation
  store and forwards it to the WebView (`src-tauri/src/stream_channel.rs`). Any streaming speech feature has a native
  token stream to consume without involving the WebView.

What does not exist: a wakeword, automatic end-of-speech, streaming speech for any voice, automatic turn-taking, and
full-duplex audio. The roadmap leaves acoustic echo cancellation open as "if later evidence justifies native DSP".

## Constraints that decide the answer

These come from Bottie's existing design documents rather than from this investigation.

| Constraint | Source | What it rules out |
| --- | --- | --- |
| Fully local speech I/O; no cloud speech API, no telemetry, no runtime activation key | README, `DEPENDENCY-LICENCES.md` posture | Hosted STT/TTS, cloud turn detectors, Picovoice Porcupine (its AccessKey is validated at engine init, so the SDK does not run without it) |
| No GPL, AGPL, LGPL, or SSPL; MIT-compatible code **and weights** | [`DEPENDENCY-LICENCES.md`](../DEPENDENCY-LICENCES.md) | eSpeak-linked engines in-process, non-commercial and research-only weights, field-of-use-restricted weights |
| Runtime assets pinned by revision plus SHA-256 and size, licence recorded | [`runtime-assets.json`](../runtime-assets.json), `src-tauri/src/semantic_model.rs` | "resolve whatever the model hub serves" |
| Four release targets: macOS arm64/x64, Windows x64, Linux x64 | `.github/workflows/`, packaging scripts | Single-platform speech APIs as *the* path |
| Rust owns audio, PCM, devices, cancellation; IPC is path-free and bounded | README, `src-tauri/src/lib.rs` | WebView audio, PCM over IPC, engine paths in UI state |
| Heavy native code in supervised workers, not the credential-holding process | `src-tauri/src/local_image_worker.rs`, `src-tauri/src/speech/nano.rs` | New in-process C/C++ surfaces without an explicit decision |
| Bounded memory, download, and idle CPU | README, [`moss-nano-integration.md`](moss-nano-integration.md) | Always-loaded gigabyte models and always-open microphones without a budget case |
| The turn stays a text transcript plus a text reply | product requirement | Speech-to-speech models as the primary path |

Two facts about the existing stack shape every candidate comparison:

- **ONNX Runtime is already present.** `fastembed` `6` pulls `ort` `2.0.0-rc.13` and ONNX Runtime 1.28.0 for the
  built-in EmbeddingGemma model, and `src-tauri/src/semantic_model.rs` already implements the pinned-download,
  SHA-256-verified, app-owned-cache pattern for an ONNX model. An ONNX wakeword, VAD, turn detector, or TTS voice
  therefore reuses an existing runtime and an existing acquisition contract.
- **That runtime has an unrecorded fourth target.** `runtime-assets.json` pins ONNX Runtime archives for
  `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, and `x86_64-unknown-linux-gnu` only — there is **no
  `x86_64-apple-darwin` archive**. Any new `ort`-hosted model inherits an unresolved asset contract for one of the
  four release targets. This is cheap to fix and must be fixed before, not after, a model depends on it.

## Decision criteria

Candidates are compared on, in this order:

1. **Licence and distribution.** MIT-compatible code *and weights*, and terms that survive shipping a desktop
   application to end users. This criterion eliminated more candidates than every technical criterion combined.
2. **Locality.** Fully local inference with no account, key, or network dependency at runtime.
3. **Platform coverage.** All four release targets from one integration, with per-platform fallbacks documented.
4. **Fit with the existing architecture.** Reuse of `cpal`, `ort`, the pinned-asset contract, the `StreamSink` token
   stream, and the existing run-cancellation and capture-admission rules.
5. **Latency and resource cost.** Time-to-first-audio and end-of-speech-to-final for the feature; resident memory
   and idle CPU for anything always-on.
6. **Trust surface.** Bytes and processes inside the credential-holding Rust process.
7. **Maturity.** Maintenance activity, release history, and how fast the upstream moves under a pinned integration.

Raw throughput is a weak discriminator on modern desktop hardware — most models surveyed run several times faster
than realtime — so the ordering above deliberately puts distribution, fit, and resource cost ahead of speed.

## Five findings that decide the answer

### 1. Licence is the gate, not engineering

Almost every attractive 2025–2026 speech model fails Bottie's weight-licence rule, and the failures are invisible
from the Hugging Face tags or a crate's `license` field:

- **openWakeWord's pre-trained models are CC-BY-NC-SA-4.0** even though its code is Apache-2.0
  ([project README](https://github.com/dscripka/openWakeWord)), and its README applies that sentence to everything in
  its model directory, including the Silero VAD copy it bundles. That blocks every shipped openWakeWord classifier —
  the architecture is still the right one. Silero VAD itself is MIT, which is the copy the recommended path uses.
- **Piper is GPL-3.0 in its maintained form** (`piper1-gpl`, which "embeds espeak-ng for phonemization"), and the
  `piper-rs` crate statically links a vendored eSpeak NG in its `build.rs` despite MIT crate metadata. eSpeak NG is a
  system dependency on Bottie's Linux path today; bundling it would put GPL code inside the product.
- **Supertonic 3's weights are OpenRAIL-M** (use-restricted) and its upstream repository was archived in July 2026.
  This closes a gap the earlier [`docs/audio-cpp-evaluation.md`](audio-cpp-evaluation.md) explicitly left open.
- **The LiveKit turn detector's licence forbids standalone use and any framework other than LiveKit Agents.** It is
  distributed as a perfectly usable ONNX file, and using it would breach its terms.
- **Parakeet weights are CC-BY-4.0, Nemotron uses NVIDIA's open-model licence, and SenseVoice-Small uses a custom
  FunASR model licence.** None is on Bottie's accepted list without an explicit review; none can be called
  MIT-compatible as-is.
- **TEN VAD is "Apache-2.0 with additional conditions"** carrying a field-of-use restriction, and **XTTS, F5-TTS,
  KittenTTS 2, and the Llama-derived TTS models** are non-commercial or community-licensed.

What survives cleanly: whisper.cpp and Whisper weights (MIT), Silero VAD (MIT), smart-turn v3 (BSD-2-Clause),
Moonshine (MIT), Vosk and sherpa-onnx (Apache-2.0), Kokoro-82M weights (Apache-2.0), and MOSS-TTS-Nano's pinned
assets (Apache-2.0). The practical consequence is that Bottie should build on the permissive core and treat every
"best model" recommendation from elsewhere as unverified until its weights licence is read.

**The owner has since ruled on the two remaining exceptions** (2026-11-01): Open RAIL conditions are acceptable and
GPL is not a hindrance. The licence filter therefore stops being the first selection criterion for TTS and becomes a
per-engine obligations ledger — pass-through and use-restriction duties for Open RAIL, copyleft and source-offer duties
for GPL — recorded in the measured comparison at [`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md). The
filter still applies to everything no one has waived, and the licence review of any *new* dependency remains a
prerequisite, not a formality.

### 2. "Streaming TTS" is an orchestration property, not a model property

No shortlisted engine can emit a partial waveform mid-sentence: they synthesise a clause or sentence and then yield
audio. KittenTTS's own documentation says streaming "is chunk-level, not token-level". The user-visible win therefore
comes from two things Bottie controls, not from a model's feature list:

1. **Consume the token stream natively.** `ChannelSink::text_delta` already runs inside Rust. A second `StreamSink`
   implementation — a bounded, non-blocking speech tap — can reduce deltas to speakable text and chunk them without
   any WebView involvement, preserving the existing IPC contract.
2. **Cut a short first chunk.** Clause-first chunking reaches speech roughly 400 ms earlier than waiting for a
   sentence, at the cost of a slightly rougher join; sentence-first is smoother. The design should start with a short
   first clause, then sentences, and expose the join behaviour as a tuning constant.

With that in place the engine decision turns on licence, size, and realtime factor — Kokoro for a licence-clean
destination engine, the already-shipped MOSS-TTS-Nano for the cheapest usable streaming path this month.

### 3. Echo cancellation and streaming TTS are one dependency, not two

Echo cancellation needs the exact PCM being rendered as its reference. The OS `tts` backend hands text to the
platform speech engine and never exposes samples, so **true voice barge-in is unreachable until Bottie owns its
rendered audio**. That makes a PCM-owning speech backend a prerequisite for full duplex rather than an independent
nice-to-have, and it means the streaming playback work pays for itself twice.

Two supporting facts from the repository and the pinned crates:

- **`cpal` 0.18.2 has no duplex API.** `supports_duplex`/`build_duplex_stream` exist only on unreleased upstream
  master; in the pinned version, duplex means two streams on two clocks with a ring buffer.
- **The current capture/playback interlock is the anti-self-hearing guarantee.** `speak_text` fails with
  `MicrophoneActive` and capture stops playback first. Hands-free conversation must replace device *exclusion* with a
  *processing* gate — keep the stream open and drop frames while speaking — and that gate must live in the audio
  callback, not on a timer. A leaked frame means Bottie transcribes itself and loops.

Per-platform, echo cancellation is a qualification item rather than shared code: macOS voice-processing I/O requires
taking over both audio nodes and abandoning `cpal`; Windows exposes no app-callable AEC (the platform AEC is an
APO-author API); Linux depends on PipeWire/PulseAudio echo-cancel configuration. "Barge-in works everywhere" is not
an honest promise.

### 4. Automatic end-of-speech is cheap, and the key capability is already linked

`whisper-rs` `0.16.0` is the newest published version, so "upgrade the binding" is not an available move — but that
same crate already exposes `WhisperVadContext`, whisper.cpp's standalone Silero VAD pipeline. The highest-value
capability in this whole investigation is therefore reachable with **no dependency change at all**: one 885,098-byte
MIT ggml asset, one new module, and a state machine.

The endpointing design that falls out is unremarkable in the good way: VAD decides silence, the transcript decides
completeness, timers decide when to stop waiting. A 480 ms silence hangover, extended to 750/1,000 ms while the
partial transcript is still changing and capped at 1,200 ms, with an immediate final pass on release, gives an
estimated 0.6–0.7 s from end of speech to final transcript. An optional semantic stage (Pipecat smart-turn v3,
BSD-2-Clause, 8.68 MB int8) can shorten the hesitant-speaker case later; it is a refinement of silence endpointing,
not a replacement.

Two corrections worth recording: `webrtc-vad` is not GPL-3.0 (the WebRTC algorithm is BSD-3-Clause, the Rust crate
MIT — the LGPL hazard is a different packaging of the audio-processing fork), and Whisper is snapshot-based, not
streaming: an ASR upgrade alone does not produce low-latency partials.

### 5. The wakeword is the most expensive ask and the only one that is licence-blocked

The product request lists the wakeword first; the evidence puts it last. The architecture is settled — a frozen
speech-embedding front end with a small classifier head, 80 ms frames, a few MB, in-process — and the remaining work
is ownership:

- **The classifier must be a Bottie build artifact.** Train "hey bottie" with LiveKit's Apache-2.0 pipeline (synthetic
  positives, augmented negatives, ONNX export, DET/FPPH evaluation) and pin the resulting ~1 MB file. The phrase is a
  shipped, evaluated asset, not a user-renamable setting: a renamable wakeword needs either a training service —
  Snowboy's exact mistake, its custom words lived behind a hosted endpoint and died with the service — or a per-user
  model Bottie cannot evaluate.
- **The two shared front-end files need a provenance decision.** openWakeWord's blanket non-commercial sentence may
  be over-broad for a mel-spectrogram front end and a re-implementation of Google's Apache-2.0 speech embedding, but
  "may be over-broad" is not a licence review, and LiveKit's crate compiles those same bytes into the binary while
  declaring only Apache-2.0. Resolve it before shipping, or rebuild the two files from clean provenance.
- **Always-on is the wrong default.** The microphone opens when the user arms **Voice mode**, the armed state is
  visible and time-limited, and detection is suspended while Bottie speaks (no AEC yet). The honest framing is
  "opt-in hands-free activation", not "always listening".
- **The runner-up removes training entirely.** sherpa-onnx keyword spotting is Apache-2.0 with an official Rust
  crate and a `KeywordSpotter` API; a custom phrase is a keywords-file line, not a training run. It loses on model
  size (~19 MB), 160–320 ms chunking, an in-process C++ core, and no published English false-accept rate.

An OS-level shortcut does not exist: none of macOS, Windows, or Linux offers a public always-on keyword-spotting API.

## Options by capability

Verdicts are against the licence-and-fit criteria above, not against raw accuracy. The parts carry the full tables.

### Speech recognition and end of speech

| Candidate | Licence | Size / RAM | Streaming / endpointing | Verdict |
| --- | --- | --- | --- | --- |
| **whisper.cpp tiny Q5** (shipping) | MIT | 32,152,673 B | snapshot; re-recognise growing window | keep |
| **whisper.cpp base Q5** | MIT | 59,707,625 B, same pinned revision | as above | **recommended accuracy step**, user-visible option |
| whisper.cpp small Q5 / large-v3-turbo | MIT | 190 MB / 574 MB | as above | later, if RAM budget grows |
| **Silero VAD via `whisper-rs`** | MIT | 885,098 B | frame classifier, batch-shaped API | **recommended release-stage VAD** |
| **Pipecat smart-turn v3.2 int8** | BSD-2-Clause | 8,679,182 B, 23 languages | semantic end-of-turn after VAD silence | optional phase, needs the fourth ORT archive |
| sherpa-onnx streaming zipformer | Apache-2.0 | ≈ 72.7 MB English int8 | true streaming partials | runner-up if snapshot cadence binds |
| Moonshine tiny/base | MIT | ≈ 28 / 63 MB int8 | incremental decode | rejected: English-first |
| Vosk | Apache-2.0 | 40 MB model, ~300 MB RSS | true streaming | rejected: Kaldi-era accuracy, stale crate |
| NVIDIA Parakeet / Nemotron | CC-BY-4.0 / NVIDIA OML | 355–1,256 MB / 2,244 MB RSS | cache-aware streaming | blocked: weights licence, no Rust binding, RAM |
| SenseVoice-Small | custom FunASR model licence | 254 MB / 556 MB RSS | non-streaming | blocked: weights licence (supersedes the earlier "candidate to beat" call) |
| macOS/Windows OS speech | OS terms | 0 | partials, own endpointing | rejected: single-platform, per-locale on-device guarantee |
| Speech-to-speech LLMs | mixed | large | n/a | excluded by the pluggable-model requirement |

### Text-to-speech, streaming

| Candidate | Licence | Size / RAM | Realtime factor / first audio | Verdict |
| --- | --- | --- | --- | --- |
| **Supertonic 3** | code MIT, **weights Open RAIL-M** (accepted by the owner), upstream archived | **398,653,124 B** / 608 MiB process RSS [measured] | **RTF 0.209 → 4.8× realtime**; **0.93–1.16 s first audible** with a 1–4-word opening clause at a 0.5–0.7 s endpoint [measured, CPU 4t]; no streaming API | **default candidate**: the owner ranked it superior by ear, licence accepted; measured in [`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) |
| **Kokoro-82M fp32** | **code MIT, weights Apache-2.0** | **353,719,767 B** / ~1.1 GB process high-water [measured] | **RTF 0.234 → 4.3× realtime**, ~0.83 s first clause [measured, CPU 4t] | **licence-clean fallback** (owner: acceptable); int8 variant **rejected on audition** |
| **Piper 1.8** | **GPL-3.0-or-later**, statically embedded eSpeak NG (accepted); voice must be licensed | medium voice **63,206,179 B** / 375 MB RSS [measured] | **RTF 0.026 → 39× realtime**, ~0.095 s first audio [measured, CPU 4t]; high-tier clean voice only 4.8× | **rejected**: `en_US-lessac-medium` sounds best but is **not redistributable** (research-only data); the three clean voices were auditioned and none is good enough |
| **MOSS-TTS-Nano** (shipping, opt-in) | Apache-2.0 assets | 763 MB + Python 3.12, 1.6–1.7 GB RSS [measured] | first PCM 0.115–0.159 s, RTF 0.249–0.372 [measured] | keep as the existing opt-in engine; no longer the only streaming path |
| **OS voices** (shipping) | platform | 0 | whole-utterance; cannot stream | keep as fallback; sentence-level playback through the new path |
| KittenTTS v0.8 | Apache-2.0 | 25–80 MB | small; TTFB unmeasured | plausible small option, needs a measurement |
| Chatterbox-Nano / VibeVoice / Orpheus / CSM / Higgs | MIT / MIT / mixed / restricted | 0.1–6 GB | no CPU realtime case | rejected: too large for a CPU laptop, or licence |
| XTTS-v2 / F5-TTS / KittenTTS 2 | CPML / CC-BY-NC-4.0 / Stellon | — | — | rejected: non-commercial weights |

**What accepting Supertonic 3 means in practice** (the owner accepted it on 2026-11-01, then ranked its voice superior
to the alternatives, making it the default candidate). Its weights use the
BigScience Open RAIL-M licence (checked in the model repository's `LICENSE` on 2026-11-01), not a GPL or
non-commercial licence: commercial use and redistribution are permitted with copyright and patent grants. The
conditions remain in force and are the reason this stays a scoped exception rather than a licence reclassification:

- **Pass-through.** Distribution must give recipients the licence and must include the Attachment A use restrictions as
  an enforceable provision binding subsequent users, with notice that the model is subject to them. Bottie would ship
  the licence text and bind its users in its terms, not just a notices line.
- **Two user-visible obligations.** Attachment A(e) forbids disseminating generated content without expressly
  disclaiming that it is machine-generated — a spoken reply is generated content — and (l) prohibits medical advice and
  medical interpretation, while (h) prohibits fully automated decisions affecting legal rights. Those are product
  statements, not dependency bookkeeping.
- **A remote-restriction clause.** §7 reserves the licensor's right to restrict or update the model "remotely or
  otherwise" and asks you to use the latest version, which sits badly with Bottie's pin-and-verify, local-only,
  no-telemetry posture — and is practically moot, because the upstream reference implementation was archived in July
  2026, so a pinned integration becomes permanently Bottie-maintained.
- **It stays outside Bottie's default permissive graph.** Acceptance does not make it MIT-compatible; the obligation to
  record it in `DEPENDENCY-LICENCES.md` and the notices is what makes the exception auditable.

**What accepting Piper means in practice — and why its best voice still cannot ship.** `piper-tts` 1.8.0 is
GPL-3.0-or-later and its macOS wheel statically embeds eSpeak NG 1.52.0 plus 19.3 MB of voice data, so bundling ships a
GPL binary and requires a source offer; the owner has accepted that. The blocker is the *voice*: the owner ranked
`en_US-lessac-medium` superior, and that voice is trained on Blizzard 2013 Lessac data under a research-only,
non-transferable licence that excludes developing voice-synthesis products and use by third parties. Acceptance of GPL
does not waive a dataset licence. Worse, the catalogue concentrates the problem: 13 of 26 English/German Piper voices
are fine-tuned from the Lessac voice and five from the CC BY-NC-SA Ryan voice, so a clean dataset label does not make
the weights clean — only five voices are both from scratch and on redistributable data, and `en_US-ljspeech-medium` was
rejected on quality. The three remaining clean voices (`en_US-kristin-medium`, `en_US-norman-medium`,
`en_GB-cori-high`) are the second-audition candidates in [`samples/`](voice-mode-parts/samples/); medium-tier clean
voices keep Piper's 39× realtime advantage, while the high-tier one is only ~5× and therefore no faster than
Supertonic. The full audit is in [`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) §4.

### Wakeword

| Candidate | Licence | Size | Custom phrase | Verdict |
| --- | --- | --- | --- | --- |
| **Own classifier on the OWW-embedding front end** | Apache-2.0 front end (provenance to clear); classifier is Bottie's | ≈ 2.4 MB + ≈ 1 MB | retrain | **recommended**, licence gate first |
| **sherpa-onnx KWS** | Apache-2.0 | ≈ 19 MB fp32 / ≈ 8 MB int8 + 3.2 MB phone table | keywords-file edit, no training | **runner-up** |
| openWakeWord pre-trained models | **CC-BY-NC-SA-4.0** | ≈ 2.4 MB front end + head | retrain | blocked on weights |
| Picovoice Porcupine | proprietary, AccessKey | ≈ 1 MB | console | rejected: runtime activation gate |
| microWakeWord | Apache-2.0 **including weights** | 115,400 B | very hard to train | cleanest licence, wrong device class |
| Vosk / rustpotter / phonix | Apache / Apache / **AGPL-3.0** | 40 MB / small / — | — | rejected: footprint, unmaintained, licence |
| OS keyword APIs | — | — | — | do not exist |

## Recommendation: a phased plan

The phases are ordered so that each one is independently useful and each later phase's prerequisite is built, not
hoped for. Estimates are labelled as such. The measured engine comparison in
[`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) fills in phase 3's numbers, and
[`voice-mode-implementation-plan.md`](voice-mode-implementation-plan.md) maps each phase to concrete module changes,
tests, and exit criteria.

| Phase | Content | New dependencies | Exit criteria |
| --- | --- | --- | --- |
| **0 — Hands-free half-duplex** | A Rust turn controller: explicit **Start conversation** arms a bounded session; existing VAD + a 480–700 ms turn-level silence hangover with transcript-stability gates ends the turn; the final transcript enters the existing generation path unchanged; a native token tap + clause/sentence chunker starts speech on the first chunk through the *existing* speech backend; **Interrupt** stays explicit; per-stage latency instrumentation | none | ≥ 5 consecutive turns without a keypress or a new device open; interrupt ≤ 250 ms; zero self-transcription with the mic gate in the audio callback (tested) |
| **1 — Audio owner hardening** | One long-lived output owner with a bounded queue; resampling; duplex ring buffer (two `cpal` streams); device-change recovery; measured output latency from callback timestamps | `rubato`, `ringbuf` | survives a route change mid-session; no per-sentence device open; output latency measured and reported |
| **2 — Endpointing upgrade** | Silero VAD (`WhisperVadContext`) as the release stage; ASR interval 1,500 → 1,000 ms with onset-anchored deadlines and a release-triggered final pass; optional `ggml-base-q5_1`; measure end-of-speech → final transcript and onset → first partial | none (ORT target fix if the semantic stage is added) | false-cut rate measurably below silence-only; end-of-speech → final ≤ 1 s |
| **3 — PCM-owning streaming TTS** | **Supertonic 3** as the default voice (measured 399 MB, 4.8× realtime, **0.93–1.16 s first audible** with a 1–4-word opening clause at a 0.5–0.7 s endpoint; no streaming API, so the clause chunker is mandatory; Open RAIL obligations discharged in notices/terms), integrated through the **validated** `sherpa-onnx` 2.0.1 path with a **first-party conversion of the official fp32 weights** (399 MB, RTF 0.108 → **1.94× the Python SDK on identical weights**, 0.26–0.32 s cold load, 643 MiB RSS, no eSpeak; own clause chunker because the engine callback is per-text-chunk, not intra-chunk; sampler seed must be pinned) or an `ort` driver — **not** the archived Python SDK; int8 pack an optional compact variant (145 MB vs 399 MB; owner audition: fp32 preferred, int8 acceptable) streaming `begin/push/end/interrupt` backend trait; **Kokoro fp32** the licence-clean fallback (354 MB, 4.3×, Apache weights, Bottie-owned phonemiser); **Piper** rejected: its best voice cannot be redistributed and its three clean voices failed the audition (cori-high, the best of them, only ~5× realtime); sentence-level fallback for OS voices and Nano | `ort` (already present) or `sherpa-onnx` | first audible sample after first token measured end to end; interrupt silences within one output callback; Open RAIL obligations recorded; no unbundled research-licensed voice anywhere in the tree |
| **4 — Echo cancellation and voice barge-in** | Render reference from the owned PCM; per-platform AEC decision (in-process `aec3`, macOS voice-processing I/O, or documented PipeWire requirement); voice-triggered barge-in reusing the existing cancellation boundary | per-platform | measured echo return loss enhancement per platform; barge-in ≤ 250 ms; Windows documented as keypress-only if it cannot qualify |
| **5 — Wakeword** | Licence-clean front-end provenance; "hey bottie" classifier trained and evaluated as a release artifact; arming TTL and visible armed state; scoring suspended during playback | `tract-onnx` or `ort` | false-accept rate acceptable on a realistic noise set; idle CPU and privacy budget written down; per-platform qualification |

Why this order rather than the order in the request: phase 0 delivers two of the three requested capabilities (an
end-of-speech that needs no keypress, and speech that starts before the reply finishes) with no new dependency, no
download, and no new trust surface. The wakeword is last because it is the only capability whose blocker is legal
rather than technical, and because a wakeword on top of push-to-talk still forces the user to wait for a full reply.

Two things this plan deliberately does **not** do: bundle a GPL phonemiser by default, and promise platform-neutral
barge-in before each platform qualifies. Both would be easy to write and expensive to retract. The measured
comparison and the owner's audition changed one thing in this plan: **phase 3 now has a chosen default** (Supertonic 3),
and a licence-clean fallback (Kokoro fp32). Piper was rejected after the second audition, so the engine decision is
made; what remains is integration and end-to-end latency measurement.

## What this changes in earlier documents

- **`docs/audio-cpp-evaluation.md`**: its ASR conclusion ("if recognition is ever revisited, SenseVoice-Small is the
  candidate to beat") is superseded — SenseVoice-Small's weights use a custom FunASR model licence that the earlier
  document had no data for. Its TTS conclusion (Supertonic 3 is worth integrating) now has a licence answer:
  OpenRAIL-M, with an archived upstream; the owner has accepted the conditions and ranked the voice superior, so it is
  the default candidate. Its measurements stand and were independently reproduced: the official Supertonic SDK measured
  RTF 0.209 against audio.cpp's 0.222 on the same host and text, a 5.8% difference inside host noise.
- **`whisper-rs` licence**: the crate declares **Unlicense** in `dependency-inventory.json` (classified compatible),
  not MIT; whisper.cpp itself is MIT. The recommendation is unaffected, but the notice text should say Unlicense.
- **WebRTC VAD**: not GPL-3.0. The WebRTC algorithm is BSD-3-Clause and the `webrtc-vad` crate is MIT (last
  published 2019); the LGPL hazard is a different packaging of the audio-processing fork. The reason to skip it is
  age and accuracy, not licence.
- **LiveKit turn detector**: usable-looking and unusable — its licence forbids standalone use and non-LiveKit
  frameworks, which is invisible from its `license: other` tag.
- **Kokoro size and quantisation**: the earlier "≈ 88 MB int8" was wrong. The smallest released English ONNX pack is
  **142,333,725 B** (114.1 MB int8 model + 28.2 MB voice table), and the owner's audition rejected the int8 variant as
  audibly worse — so the viable Kokoro pack is the **353,719,767 B fp32** one. The measured Kokoro path also dlopens
  GPL-3.0 `libespeak-ng` in-process, which is why shipping it cleanly needs a Bottie-owned phonemiser.
- **Piper voice catalogues**: a clean dataset label does not imply clean weights. Most English/German Piper voices are
  fine-tuned from Lessac (research-only data) or Ryan (CC BY-NC-SA), and `en_US-lessac-medium` — the best-sounding one —
  cannot be redistributed at all; the three redistributable voices were auditioned and none is good enough. Piper is
  out; see [`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) §4.
- **The production path is validated, and it beats the reference implementation.** `sherpa-onnx` 2.0.1 driving the
  official fp32 Supertonic weights measured **RTF 0.1077 (1.94× the Python SDK on the same weights)** — the speed-up is
  the C++ implementation, not quantisation — with a 0.26–0.32 s cold load and 643 MiB RSS. The third-party int8 pack is
  only 3–8% faster, is not bit-transparent, and its licence metadata is internally contradictory (MIT file, OpenRAIL
  upstream, and RAIL binds derivatives), so Bottie should convert the official weights first-party and treat them as
  Open RAIL-M throughout. The sampler's default random seed must be pinned for A/B, audio caching, and tests.

## Open measurements

None of these blocks phase 0, and each one is cheap. They are ordered by what they would change.

| # | Measurement | Settles |
| --- | --- | --- |
| 1 | whisper tiny Q5 decode time for 5 s and 30 s windows, warm, per platform | whether the final-transcript flush fits the 1 s end-of-speech target |
| 2 | End-of-speech → final transcript and onset → first partial, instrumented in-app | the real endpointing latency, and whether the ASR interval should go to 800 ms |
| 3 | Endpoint false-cut rate at 480/700/900 ms on recorded conversations | the default hangover, and whether a semantic stage is worth its 8.7 MB |
| 4 | Device output latency distribution per platform and device class | the floor under first-audible-word latency |
| 5 | Interrupt latency: user action → last sample out | whether ≤ 250 ms is already met |
| 6 | CPU and RSS of one wakeword detector, armed, per platform | the always-armed budget case |
| 7 | "hey bottie" false-accepts per hour on ≥ 5 h of real room audio and false-rejects over ≥ 20 speakers | the default threshold, and whether the per-user verifier is mandatory |
| 8 | Supertonic and Kokoro fp32 realtime factor on the **weakest supported host** (four-core x64 laptop) — the M3 Max numbers are in [`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) | whether the default engine needs a hardware floor (a 4-core laptop is roughly 2–3× slower than this host) |
| 9 | OOV/numbers/proper-noun quality of a Bottie-owned English phonemiser (CMUdict coverage is known: 31/32 protocol words) — only needed if the Kokoro fallback is activated | the cost of licence-clean phonemisation for the fallback path |
| 10 | Echo return loss enhancement of `aec3` on recorded render/capture pairs | whether phase 4 is feasible per platform |
| 11 | Whether `WhisperVadContext` holds a stable 5 Hz decision rate with a 2 s trailing window | whether the Silero design needs a different windowing strategy |
| 12 | Whisper tiny vs base resident set in Bottie's process shape | the memory cost of the accuracy step |
| 13 | First audible word end-to-end through the real pipeline, with a real generation and the clause chunker, for Supertonic | whether the 1.5 s median target survives integration and provider latency (the component numbers say 0.93–1.16 s before provider time) |
| 15 | ~~A/B int8 vs fp32 via sherpa~~ — done 2026-11-01: fp32 preferred, int8 acceptable | closed: fp32 is the default (1.94× the SDK, approved voice); int8 stays an optional compact pack |
| 16 | Build `sherpa-onnx-sys` in the Tauri crate and codesign the bundled macOS dylib | whether the validated path survives packaging on all four release targets (build-time native download is a supply-chain pin, not yet done) |
| 14 | ~~Second Piper audition~~ — done 2026-11-01: none of the three clean voices is good enough (cori-high least bad) | Piper is out of the plan; the medium-tier fast path is unavailable |

## Confidence and limits

This is a survey with an integration argument plus two purpose-built measurement sessions, not a benchmark programme.
The candidate landscape is dated by inspection to 2026-11-01 and speech tooling moves in months; the licence
positions are the most durable part, and the realtime factors are the least. The three TTS engines in
[`tts-head-to-head.md`](voice-mode-parts/tts-head-to-head.md) were actually run on this host, but on a shared machine
that was never idle, so their numbers carry a ±10–18% band and were taken under different load stamps; the ASR,
wakeword, and pipeline numbers remain estimates or vendor claims. The voice ranking is the owner's, from a
non-blind audition of one sample per engine that confounds engine, voice, and sample rate; the second Piper audition
found no acceptable clean voice, but it was not blind either. The Lessac licence reading is an interpretation of the published agreement, not legal advice,
and whether trained weights are a Derivative of the training data is the unsettled point. Where a number would decide
a phase, the phase starts with the measurement instead of the claim.
