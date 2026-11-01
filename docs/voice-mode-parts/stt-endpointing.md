# Local streaming speech-to-text and automatic end-of-speech detection

Investigated 2026-11-01. Working notes for the voice-mode investigation; the Lead merges these parts into
`docs/voice-mode-evaluation.md`. This file is research input, not product documentation, and no Bottie source,
dependency, build, packaging, or test path reads it.

Scope: the ASR half of voice mode (what turns microphone audio into a transcript locally) and the capability Bottie
actually lacks — deciding *when the user has finished speaking* without a Stop press. Wakeword and TTS belong to other
parts of this investigation and are not re-litigated here.

Repository state this was written against: `src-tauri/Cargo.toml` pins `whisper-rs = "=0.16.0"`, `cpal = "0.18.2"`,
`tts = "=0.26.3"`, `fastembed = "6"` (which selects `ort`/`ort-sys 2.0.0-rc.13`), and `runtime-assets.json` pins exactly
one speech asset (`ggml-tiny-q5_1.bin`, 32,152,673 bytes) plus three ONNX Runtime archives. Recording owns capture,
PCM, and cancellation in Rust; the WebView never sees PCM.

## 1. Answer first

**Keep Whisper in-process and spend the effort on endpointing, not on a new recogniser.** Keep `whisper-rs` and the
Whisper family, add a second pinned Whisper checkpoint (`ggml-base-q5_1.bin`, 59,707,625 bytes, same immutable
repository revision already pinned) as the accuracy step up, and build automatic end-of-turn out of things Bottie can
already link: Silero VAD through the `WhisperVadContext` that `whisper-rs` 0.16.0 already exposes (885,098-byte MIT
ggml asset), a 480–700 ms silence hangover, an ASR-stability confirmation, and a hard cap. Runner-up: sherpa-onnx
streaming zipformer (Apache-2.0, ~72.7 MB int8 for English) with Pipecat smart-turn v3.2 int8 (BSD-2-Clause, 8,679,182
bytes) as the semantic end-of-turn stage — adopted only if real-microphone measurements show Whisper's snapshot cadence
is the binding constraint rather than the endpointing policy.

## 2. Constraints that decide it

These are the ones that actually separate candidates; the rest of the conventions list does not discriminate here.

1. **Licence.** Bottie's `DEPENDENCY-LICENCES.md` records no GPL/AGPL/LGPL/SSPL and treats Apache-2.0, BSD, MIT, ISC,
   MPL-2.0 as acceptable. Applied strictly, this eliminates or blocks several attractive models, and the blocks are the
   single largest finding of this slice:
   - LiveKit's turn detector is under a **custom LiveKit model licence that forbids standalone use and use with any
     framework other than LiveKit Agents** ("you may use these LiveKit models freely but can only use them together with
     the LiveKit Agents framework. You cannot use the LiveKit models on a standalone basis or with any other
     frameworks."). That is a hard rejection for Bottie, and it is not visible from the model's `license: other` tag
     alone.
   - Parakeet-TDT-0.6B-v3 **weights** are `cc-by-4.0` upstream; NVIDIA's Nemotron streaming model is under the
     `nvidia-open-model-license`; SenseVoice-Small is under a `model-license` ("other") in FunASR, not MIT/Apache. None
     of these are on Bottie's acceptable list, so all three need explicit review before they can be recommended, and
     none can be called MIT-compatible as-is.
   - TEN VAD is **"Apache 2.0 with additional conditions"**: the extra terms forbid deploying it in a way that competes
     with Agora's offerings and restrict it to "your Application(s)". That is a field-of-use restriction, so it is not
     plain Apache-2.0 either.
   - Clean licences confirmed for the recommended stack: whisper.cpp + Whisper weights (MIT), Silero VAD via
     `ggml-org/whisper-vad` (MIT), smart-turn v3 (BSD-2-Clause), sherpa-onnx models (Apache-2.0), Moonshine (MIT),
     Vosk (Apache-2.0), `whisper-rs`/`sherpa-rs`/`vosk` crates (MIT).
2. **Trust boundary.** The established precedent for heavy native code is the supervised sidecar, not new in-process
   linkage (`CONVENTIONS.md`). `whisper-rs`/`whisper.cpp` is already inside the process and already reviewed, so
   extending it costs no new trust surface. Every added runtime (ONNX Runtime *for a new purpose*, a Python sidecar,
   sherpa-onnx's C++ core) is new surface that is not justified by "the transcript is slightly better".
3. **Cross-platform completeness.** Release targets are macOS arm64, macOS x64, Windows x64, Linux x64. ONNX Runtime is
   already in the Rust graph through `fastembed`, but `runtime-assets.json` pins archives for **only three** targets:
   `aarch64-apple-darwin` (coreml), `x86_64-pc-windows-msvc` (directml), `x86_64-unknown-linux-gnu` (none). There is no
   recorded `x86_64-apple-darwin` archive. Any new ORT-hosted model (Silero via `ort`, smart-turn via `ort`) therefore
   inherits an unresolved fourth-target asset contract, whereas the whisper-rs Silero path inherits nothing.
4. **Bounded resources.** The pinned speech model is 32 MB and `MAX_RETAINED_BYTES` is 32 MiB; the app documents a
   deliberate memory budget. A 32 MB → 60 MB model is inside that budget. A 254 MB–1.26 GB model with a measured
   556 MB–2.24 GB resident set is a product decision, not a dependency bump.
5. **The transcript is the interface.** Whatever ASR wins must produce plain text with timing so any text model remains
   usable. This is what rules out speech-to-speech audio LLMs as the primary path (§4.9).

## 3. Candidate tables

### 3.1 ASR candidates

Sizes are exact bytes from the immutable repository listings I checked (Hugging Face API, 2026-11-01); "RSS" is
resident memory and is `unknown` unless someone measured it — audio.cpp's numbers are from
[`audio-cpp-evaluation.md`](../audio-cpp-evaluation.md) on an Apple M3 Max and are quoted, not re-measured.

| Candidate | What it is | Licence | Weights / disk | RAM (RSS) | Platforms | Streaming / endpointing support | Rust integration path | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **whisper.cpp / `whisper-rs` `ggml-tiny-q5_1`** (shipping) | 39M-param encoder-decoder, multilingual, ggml | MIT (code + weights) | **32,152,673 B** | unknown in-process; whisper.cpp quotes ~273 MB for f16 `tiny` | all 4 | Snapshot (not partial): re-transcribe a growing window; VAD API available | already in-process (`whisper-rs =0.16.0`) | [measured] size/contract in repo; [vendor-claimed] memory table |
| **`ggml-base-q5_1`** | Whisper base, 74M params, multilingual | MIT | **59,707,625 B** | unknown; ~388 MB class for f16 `base` | all 4 | as above | same crate, drop-in file swap (same pinned revision) | [measured] size; [vendor-claimed] memory |
| **`ggml-small-q5_1`** | Whisper small, 244M params, multilingual | MIT | **190,085,487 B** | unknown; ~852 MB class for f16 `small` | all 4 | as above | same crate | [measured] size; [vendor-claimed] memory |
| **`ggml-large-v3-turbo-q5_0`** | Whisper large-v3-turbo, multilingual | MIT | **574,041,195 B** | unknown; large-v3 class is ~3.9 GB unquantised | all 4 | as above | same crate | [measured] size; rejected on disk/RAM |
| **distil-whisper `distil-large-v3`** | Distilled large-v3, **English only** | MIT | 756,405,760 params (F16); ONNX + safetensors | unknown | all 4 (needs conversion) | as above | needs a ggml conversion; none in the pinned whisper.cpp model repo | [measured] params; rejected: English-only, size, conversion outside the pinned revision |
| **faster-whisper / CTranslate2** | Whisper re-implemented on CTranslate2 (int8/fp16) | MIT / MIT | per-model CT2 packs | unknown | all 4 via Python | pseudo-streaming; own VAD; no partial API guarantee | Python sidecar (MOSS-Nano precedent) | [measured] licences; rejected for STT: adds a Python runtime per capture |
| **NVIDIA Parakeet-TDT-0.6B-v3 via ggml** | FastConformer-TDT transducer, 25 European languages + English | weights `cc-by-4.0` upstream; ggml conversion repo tagged MIT | q4_0 **355,615,679 B**, q8_0 668,757,119 B, f16 1,255,897,319 B | unknown | ggml builds on all 4 | offline chunked (not cache-aware streaming) | whisper.cpp has `parakeet-cli`, but `whisper-rs` 0.16.0 exposes **no** Parakeet API | [measured] sizes; blocked: licence + no Rust binding |
| **NVIDIA Nemotron streaming 0.6B** | Cache-aware streaming FastConformer-RNNT, **English only** | `nvidia-open-model-license` ("other") | q8_0 GGUF ~628 MB (`gguf.total` 628,356,737) | **2,244 MB** [measured, audio.cpp] | ggml/ONNX builds | true cache-aware streaming (1.12 s frame size) | no Rust binding; ONNX path = ORT | [measured] RAM/throughput 21.3× CPU / 72.1× Metal; blocked: licence, English-only, RAM |
| **SenseVoice-Small** | Non-autoregressive encoder, 22 languages + auto-detect, very fast | **custom `model-license`** (FunASR) — not MIT/Apache | 254.2 MB [measured]; `model.pt` (PyTorch) needs ONNX export | **556 MB** [measured] | ONNX via ORT | not streaming; needs Silero asset + strict 16 kHz | ORT (4th target archive problem) | [measured] size/RAM/39.7× CPU; **blocked: weights licence** |
| **Moonshine tiny / base** | Conformer-style encoder + decoder, **English** (+ tiny ja/ko/vi/uk/zh/ar), purpose-built for streaming | MIT | base int8 encoder 20,513,063 B + decoder 42,498,870 B ≈ **63 MB**; tiny int8 ≈ 28 MB; float base ≈ 247 MB | unknown | ORT | split preprocess/encode/cached-decode ONNX = incremental decode | ORT (in-process today, but see §2.3) | [measured] file sizes; rejected as primary: English-only |
| **Vosk (small models)** | Kaldi nnet3/ivector, per-language models, truly streaming | Apache-2.0 (code + models) | small en-us **40 MB**; ~31–58 MB per small language | ~300 MB [vendor-claimed] | all 4 via `libvosk` | best-in-class streaming partials + built-in endpointing | `vosk` crate 0.3.1, **last published 2024-10** (stale) | [vendor-claimed] sizes/RAM |
| **sherpa-onnx streaming zipformer** | Transducer (encoder/decoder/joiner ONNX), Apache-2.0 zoo, chunk-16 configs | Apache-2.0 (models tagged `apache-2.0`) | en 2023-06-26 int8: encoder 71,083,163 B + decoder 1,307,236 B + joiner 259,335 B ≈ **72.7 MB**; fp32 encoder 262 MB | unknown | all 4 (C++ core, prebuilt libs) | real streaming (`chunk-16` ≈ 320 ms, `left-64/128` context) + separate VAD/TEN-VAD | `sherpa-rs` 0.6.8 (MIT wrapper, **last published 2025-10**) | [measured] file sizes; licence [vendor-claimed] |
| **NVIDIA audio.cpp `nemotron_asr`/`sense_asr`** | C ABI wrapper over the same families | Apache-2.0 framework | as above | as above | measured: Linux x64 + macOS arm64 only | pull-based streaming | in-process C ABI or sidecar | [measured] in `audio-cpp-evaluation.md`; not a new option, already evaluated |
| **macOS Speech (`SFSpeechRecognizer`)** | OS ASR | OS, no asset | 0 B | OS-owned | macOS only | partial results, own endpointing | ObjC/Swift from Rust (`objc2`) | [vendor-claimed] docs; **rejected: "network-based service", 1-minute cap, per-locale on-device support** |
| **macOS 26 `SpeechAnalyzer`/`SpeechTranscriber`** | New on-device analyser + module pipeline | OS, no asset | 0 B (downloaded OS assets) | OS-owned | macOS 26+ only | volatile (partial) ranges first class | ObjC/Swift from Rust | [vendor-claimed] docs; rejected: single-platform, macOS 26+ floor |
| **Windows `SpeechRecognizer` (WinRT)** | OS ASR | OS, no asset | 0 B (language pack) | OS-owned | Windows only | hypothesis events + built-in silence `Timeouts` | `windows` crate (C++/WinRT ABI) | [vendor-claimed] docs; rejected: single-platform |

Whisper memory reference (whisper.cpp README, unquantised f16): tiny 75 MiB disk / ~273 MB mem, base 142 MiB / ~388 MB,
small 466 MiB / ~852 MB, medium 1.5 GiB / ~2.1 GB, large 2.9 GiB / ~3.9 GB. The repository does **not** have a measured
resident set for Bottie's own q5_1 tiny process; treat the row above as `unknown` and measure it (§7).

### 3.2 Endpointing / end-of-turn candidates

| Candidate | What it is | Licence | Size on disk | Runtime cost | Languages | Rust path | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Energy VAD (shipping)** | RMS per 20 ms frame, onset ≥0.02 × 3 frames, release ≤0.012 × 10 frames | Bottie's own | 0 B | negligible | none | already in `microphone/vad.rs` | [measured] from source (deterministic) |
| **Silero VAD v5.1.2 / v6.2.0 via whisper.cpp** | Small CRNN frame classifier, ggml conversion | MIT | **885,098 B** each | RTF ≈ 0.0127 on one Xeon core [third-party] | none (language-agnostic) | **already exposed**: `whisper_rs::WhisperVadContext` in the pinned 0.16.0 | [measured] size/sha; [third-party] RTF |
| **Silero VAD v5/v6 via ONNX** | upstream ONNX model | MIT | ~2.16 MB (JIT) / 2.22 MB (ONNX) [third-party] | RTF ≈ 0.0127 | none | `ort` (already in-process; **macOS x64 archive missing**) | [third-party] sizes |
| **TEN VAD** | Frame-level streaming VAD, 10/16 ms hops | **Apache-2.0 + Agora field-of-use conditions** | 306 KB (Linux lib), 731 KB (macOS) | RTF 0.0086–0.057 [vendor-claimed] | none | C ABI / C++ core; ONNX model also shipped | [vendor-claimed]; licence needs review |
| **WebRTC VAD** | Classic GMM VAD, 10/20/30 ms frames | **BSD-3-Clause** upstream WebRTC; `webrtc-vad` crate MIT; `py-webrtcvad` repo reports NOASSERTION | crate 944 KB (vendored C) | very low | none | `webrtc-vad` crate 0.4.0, **last published 2019** | [third-party]; see correction in §4.10 |
| **Cobra (Picovoice)** | Commercial on-device VAD, licensed SDK | commercial / access-key | SDK | low | none | C SDK only | rejected: access-key + licence terms unfriendly to MIT redistribution |
| **Pipecat smart-turn v3.2** | Whisper-tiny encoder + linear head, semantic end-of-turn on raw PCM | **BSD-2-Clause** | **8,679,182 B** int8 (`v3.2-cpu`), 32,411,198 B fp32 | ~10 ms "on some CPUs" to ~65 ms on a 1× cloud instance [vendor-claimed] | 23 | `ort` — but see §2.3 archive gap | [measured] size/sha; [vendor-claimed] accuracy 92.63 % over 31,527 samples, latency |
| **LiveKit turn detector** | Qwen2.5-0.5B distilled text classifier, ONNX | **LiveKit model licence: no standalone use, no non-LiveKit frameworks** | `model_quantized.onnx` in repo | LLM-class inference | 14 | `ort` technically | **rejected on licence** |
| **ASR-stability endpointing** | Decide "finished" from partial transcript stability/punctuation | none (own code) | 0 B | negligible | inherits ASR | own Rust | [unverified] — no published benchmark; design in §5.4 |
| **LLM-based / Whisper-based end-of-turn** | Prompt a text model with the partial transcript | inherits model licence | inherits | one extra generation | inherits | existing inference stack | rejected: adds latency and a second model call per silence |

## 4. Candidate notes

### 4.1 The Whisper family: no new crate is needed, but the model in the crate is not the ceiling

`whisper-rs` 0.16.0 is the **newest published version** (2026-03-12 on crates.io, `max_version` = `newest_version` =
`0.16.0`), so "upgrade whisper-rs" is not an available move: Bottie is already on it. What the crate exposes that Bottie
does not use is `WhisperVadContext` — a handle to whisper.cpp's built-in Silero VAD pipeline
(`new(model_path, params)`, `detect_speech`, `probabilities`, `segments_from_samples`). That single fact reshapes this
slice: the highest-value capability (real VAD for endpointing) is reachable without a dependency change.

Upstream whisper.cpp itself has moved past whatever `whisper-rs-sys 0.15` vendors: the current release is **v1.9.5**
(2026-10-06), and its changelog includes Parakeet support (`whisper : add check for ttype in whisper and parakeet
(#4091)`), Silero VAD model-load validation (`vad : reject n_encoder_layers other than 4 in model load (#4064)`), an
optional ANEForge encoder backend for the Apple Neural Engine, and dozens of `ggml` backends `whisper-rs` does not enable.
[Bottie consumes none of that today: `whisper-rs` builds with no features at all, i.e. CPU only.] The version skew is
worth naming as a risk (§7) but it is not a reason to leave the family.

Model-side, the answer to "can a better Whisper be a drop-in at acceptable RAM" is **yes, one step**: `ggml-base-q5_1`
(59.7 MB, 1.86× tiny) and `ggml-small-q5_1` (190 MB, 5.9× tiny) are both in the **same immutable repository revision
Bottie already pins** (`5359861c…`, 2024-10-29), so adding base is a new line in `runtime-assets.json` and a constant in
`transcription.rs`; no new upstream, no new licence, no new code path. `large-v3-turbo-q5_0` (574 MB) is the multilingual
quality ceiling at a size the current memory story does not support. distil-whisper is **English-only** and has no
first-party ggml conversion in that pinned repository, so it cannot replace a multilingual model and would require a
conversion step whose bytes Bottie would then have to pin itself.

Two negative results worth recording. First, **Whisper is snapshot-based, not streaming**: there is no partial-result
API; "streaming" here means re-running the recogniser on a growing window, which is exactly what Bottie does on a fixed
interval. Any claim that a Whisper upgrade alone gives low-latency partials is wrong; the latency comes from the cadence
and from how the window is trimmed, not from the model. Second, **`whisper-stream`'s "VAD mode" is not Silero**: setting
`--step 0` enables a sliding-window mode with "a very basic VAD detector" whose `-vth` threshold triggers transcription on
silence. It is an energy VAD shaped like Bottie's, not the `whisper-vad` model, and reading the stream example as "whisper.cpp
has Silero in its streaming example" would be a mistake.

### 4.2 NVIDIA Parakeet and Nemotron: excellent, and out of reach for three independent reasons

Parakeet-TDT-0.6B-v3 is the strongest accuracy-per-byte candidate on paper: 25 languages, and ggml conversions exist at
three quantisations (q4_0 355.6 MB, q8_0 668.8 MB, f16 1.26 GB). Nemotron streaming is the only true cache-aware
streaming model in this comparison and measured **21.3× realtime on CPU, 72.1× on Metal** on the M3 Max — numbers no
CPU-bound Whisper competes with.

Three things block them together:

1. **Licence.** Parakeet weights are `cc-by-4.0`; Nemotron is under the NVIDIA Open Model License. Neither is on Bottie's
   acceptable list; both require explicit review with attribution/notice obligations that the current `MODEL-NOTICE.txt`
   pattern could satisfy but which nobody has reviewed. The ggml conversion repository is tagged MIT, but a conversion
   cannot relicense the weights it was converted from — that tag must not be read as a licence grant.
2. **No Rust binding.** `whisper-rs` 0.16.0's published API has no Parakeet surface at all (only Whisper contexts, VAD
   contexts, and the usual types), even though whisper.cpp now ships `parakeet-cli`. Getting it in-process means raw FFI
   against `whisper-rs-sys` (or waiting for bindings), i.e. exactly the kind of new native surface the trust-boundary
   rule asks to avoid.
3. **Memory.** Nemotron measured **2,244 MB peak RSS** for 930.6 MB of weights. Parakeet q4_0 is smaller but there is no
   measured resident set, and its q8_0 file (668.8 MB) is close enough to Nemotron's 930.6 MB that the same class of RSS
   is the honest expectation.

English-only is an additional disqualifier for Nemotron streaming specifically. Verdict: revisit Parakeet when a binding
exists and the licence is reviewed; do not put it on the critical path.

### 4.3 Moonshine: the right architecture at the wrong language coverage

Moonshine is purpose-built for streaming: the ONNX graph is split into `preprocess` / `encode` / `cached_decode` /
`uncached_decode`, so decode is incremental with a key-value cache rather than a full re-encode. The int8 merged packages
are small (base ≈ 63 MB, tiny ≈ 28 MB), it is MIT, and ORT is already in the process. What it is not is multilingual: base
and tiny are English, and the per-language tiny variants are ja/ko/vi/uk/zh/ar — Bottie's shipping model is multilingual
Whisper, and replacing it with six single-language packs is not a drop-in.

If Bottie ever ships a per-language fast path, Moonshine is the best streaming-shaped option for English. It also
inherits the ORT fourth-target problem (§2.3), because ORT is currently pinned for three of four release targets.

### 4.4 SenseVoice: audio.cpp's "candidate to beat" is licence-blocked, and that changes the earlier conclusion

`audio-cpp-evaluation.md` concluded that "if recognition is ever revisited, SenseVoice-Small is the candidate to beat, not
Nemotron". That conclusion was drawn from measurements on synthetic studio speech, and it was drawn without licence data
— the same document notes that audio.cpp's `model_specs/*.json` "carries no licence field", so the reviewer could not see
this. The model card settles it: `FunAudioLLM/SenseVoiceSmall` is `license: other` pointing at
`FunASR/blob/main/MODEL_LICENSE`, not MIT or Apache-2.0. With 254 MB of weights, 556 MB measured RSS, an ONNX export step
(`model.pt` is PyTorch), a hard Silero asset dependency, strict 16 kHz input, and almost no punctuation, it is **not**
recommendable without a licence review that currently does not exist. This slice therefore supersedes that sentence:
the ASR upgrade path that survives every filter is a larger Whisper checkpoint.

### 4.5 Vosk and sherpa-onnx: the only true-streaming Rust-reachable options

Both are genuinely streaming (partial hypotheses emitted per chunk, not re-decoded windows), both are Apache-2.0 for code
and models, and both run on all four targets.

Vosk's weakness is ecosystem age rather than quality: models are per-language (small en-us 40 MB and ~300 MB RAM is the
recommended desktop size; small models for other languages are 31–58 MB), it is Kaldi-era accuracy, quality varies
per-language (small pt reports 32.6–68.9 % WER depending on set, small en-us 9.85 % on librispeech test-clean), and the
Rust crate `vosk` 0.3.1
was last published 2024-10-27. A "40 MB model per language, downloaded on demand" story also breaks Bottie's current
one-model-for-everyone download contract.

sherpa-onnx is the stronger of the two: a broad model zoo including streaming zipformer transducers in `chunk-16`
configurations (≈320 ms chunks with 64–128 frames of left context), int8 English packs around 72.7 MB, offline
punctuation/casing models, and built-in Silero and TEN-VAD integration. The Rust wrapper `sherpa-rs` (MIT) wraps the
Apache-2.0 C++ core and downloads prebuilt binaries; its last release was **2025-10-05**, i.e. a year stale against a fast-
moving upstream, and it puts a large C++ runtime in-process. That is the trade: real streaming partials and better
endpointing ergonomics, at the cost of a second ASR runtime and a stale wrapper.

### 4.6 faster-whisper / CTranslate2: technically fine, structurally wrong

Both are MIT (`faster-whisper`, CTranslate2), both are fast, and CTranslate2 is exactly what a Python sidecar would use.
But STT already runs in-process, and a Python sidecar for recognition would add a second runtime, a model pack per
language selection, a process boundary inside the audio path, and the same per-invocation or long-lived-worker cost the
MOSS-TTS-Nano worker already carries — in exchange for none of the capabilities this slice is about. Bottie's own
`audio-cpp-evaluation.md` reached the same conclusion for `audio.cpp`'s in-process C ABI: recognition was "not worth it,
yet". faster-whisper does not change that arithmetic.

### 4.7 iOS-style platform speech: the OS APIs are the cheapest option and the wrong shape for Bottie

**macOS Speech framework.** `SFSpeechRecognizer` is available on macOS 10.15+, and on-device recognition exists —
`supportsOnDeviceRecognition` reports it and `requiresOnDeviceRecognition` on a request forces it. But Apple's own
documentation is explicit that it is a network service by default: "For some languages, the recognizer might require an
Internet connection", and the same page warns that "speech recognition is a network-based service", that per-device and
per-app limits are enforced, and that the framework **stops recognition tasks longer than one minute**. So on-device mode
is real but locale-dependent and unguaranteed, and a local-only product cannot ship a path whose on-device guarantee is
per-locale and unspecified in the docs I can read. It is also a per-platform Swift/ObjC integration in a Rust codebase
with no shared implementation on Windows or Linux.

**macOS 26 `SpeechAnalyzer`/`SpeechTranscriber`.** This is the modern, on-device, module-based API (macOS/iOS 26.0+):
`AssetInventory` installs the language assets, results stream as an `AsyncSequence`, and `volatileRange` /
`volatileRangeChangedHandler` make partial (still-changing) transcript ranges first-class — conceptually exactly what
voice mode wants, including better endpointing hygiene than Whisper snapshots. Constraints: macOS 26+ only, Swift-only
surface, system-resource limits with an `insufficientResources` error, and the doc's available-language metadata is a
9-locale list (de-DE, en-US, es-419, fr-FR, it-IT, ja-JP, ko-KR, pt-BR, zh-CN) — good coverage, not Bottie's Whisper
multilingual set. As a macOS-only accelerator it is interesting; as Bottie's STT it fails the multi-platform rule.

**Windows.** `Windows.Media.SpeechRecognition.SpeechRecognizer` is WinRT, callable from Rust via the `windows` crate, and
it has genuinely useful machinery: `ContinuousRecognitionSession`, `HypothesisGenerated` (partial fragments),
`Timeouts` (how long it ignores silence before giving up — i.e. OS-provided endpointing), and `RecognitionQualityDegrading`
events. It works offline once the Speech language pack is installed for the locale (Settings > Time & Language >
Speech), and topic/web-service grammars are the parts that need the cloud. Two disqualifiers remain: it is Windows-only,
and the language-pack requirement turns "works offline" into "works if the user already installed the pack", which is a
worse onboarding story than a 60 MB verified download that Bottie controls.

**Verdict.** Both OS routes are worth a one-line negative entry, not an investigation: platform-locked, and neither
guarantees local operation on all supported languages. If Bottie ever wants a zero-download fast path, the honest shape is
an optional macOS-26-only accelerator behind a capability probe — not the primary path.

### 4.8 Licence traps worth naming explicitly

- **LiveKit turn detector** — custom licence, standalone use prohibited. Even though it is distributed as ONNX and would
  otherwise be a perfectly usable `ort` model, using it in Bottie would breach its terms. This is the single clearest
  licence rejection in this slice, and it is invisible from the HF tags.
- **TEN VAD** — "Apache 2.0 with additional conditions" containing an explicit no-competition clause. Treat as
  `review required`, not as Apache-2.0.
- **Cobra** — commercial SDK with access-key activation; incompatible with a MIT app that must build in CI on four hosts
  and redistribute freely.
- **Parakeet / Nemotron / SenseVoice weights** — as in §4.2 and §4.4.
- **`py-webrtcvad`** — GitHub reports its combined licence as NOASSERTION because it vendors WebRTC C sources under a
  different licence than its own Python wrapper.

### 4.9 Explicitly rejected alternative: speech-to-speech audio LLMs

Moshi, Qwen-Omni, MiniCPM-o and similar audio-in/audio-out models would solve endpointing and barge-in almost for free
(they model turn boundaries natively) and would remove the transcript round trip. They are rejected as the primary path
because they violate the product's structural requirement: the turn must remain **a text transcript plus a text reply**
so that any chat model — hosted OpenAI-compatible, oMLX, local GGUF — stays usable. An audio LLM makes the voice path
depend on one specific model, and it also breaks the bounded-resource and trust-boundary rules at once. They remain a
plausible *optional* "full-duplex" mode later, exactly as the conventions say: mention, reject, and move on.

### 4.10 Correction to the brief: WebRTC VAD is not GPL-3.0

The investigation brief asked for WebRTC VAD to be flagged as GPL-3.0. I could not substantiate that and it appears to be
wrong for the code in question:

- The VAD algorithm ships inside the WebRTC project, whose source is **BSD-3-Clause**;
- the Rust wrapper crate `webrtc-vad` declares **MIT** and vendors that C code (1,134 lines of C);
- `py-webrtcvad` is a Python wrapper over the same sources and reports `NOASSERTION` on GitHub only because its combined
  tree mixes its own wrapper licence with the vendored WebRTC code.

What *is* a licence hazard in this family is a different packaging — `webrtc-audio-processing` (the PulseAudio fork of
WebRTC audio processing) is commonly distributed under LGPL-2.1 [unverified: I did not open its `COPYING`]. So the honest
flag is "the algorithm is BSD-3; a specific *fork* is LGPL — check which one you link", not "WebRTC VAD is GPL-3.0".
Practically, the reason to skip WebRTC VAD is not licence at all: `webrtc-vad` was last published in **2019**, and Silero
is better on every published precision/recall curve.

### 4.11 Semantic turn detectors: the good one is BSD-2, the popular one is unusable

**Pipecat smart-turn v3.2** is the standout: BSD-2-Clause for code *and* weights, 8.68 MB int8 ONNX, 23 languages, and a
modest 8M-parameter Whisper-tiny encoder with a linear head that runs on raw PCM (so it uses prosody, not just words).
The published benchmark reports 92.63 % accuracy over 31,527 samples, with per-language accuracy from 96.96 % (Korean)
down to 79.38 % (Vietnamese) and the notable weak spots being Chinese (85.79 %), Bengali (83.80 %), Marathi (82.43 %) and
Vietnamese. Input contract: 16 kHz mono, up to 8 s of the current turn, zero-padded at the *beginning*; re-run on the whole
turn if more speech arrives; do not run it on very short segments. Latency: "as little as 10 ms on some CPUs, under 100 ms
on most cloud instances", ~65 ms on a 1× cloud instance for `LocalSmartTurnAnalyzerV3`.

The deployment shape is the important part: it is designed to run **only after a VAD has detected silence**, on the turn
recording. That makes it a *refinement* of silence endpointing, not a replacement — which is exactly the hybrid in §5.4.
Its runtime requirements are ORT plus one 8.68 MB file, both already available in principle; the gap is the missing
`x86_64-apple-darwin` ORT archive.

**LiveKit's turn detector** is the one most people reach for and the one Bottie cannot use: it is a Qwen2.5-0.5B-Instruct
distilled classifier under the LiveKit model licence quoted in §2.1. Its ONNX packaging and 14-language coverage are
irrelevant once the licence forbids non-LiveKit frameworks.

**Whisper-based or LLM-based endpointing** (asking a text model "is this sentence finished?") is rejected for cost and
latency: it adds a generation to every silence event and duplicates state the ASR was already encoding.

### 4.12 ASR-stability endpointing is cheap and unpublished

Using partial-transcript behaviour as the end-of-turn signal — "the last N partials are identical", "the partial ends in
terminal punctuation", "the partial's word count stopped growing" — costs nothing and needs no model. It is used in
practice by several voice agents, but I found **no published benchmark** for it and it inherits every ASR error: Whisper
happily emits a complete-looking sentence mid-thought, and with a fixed 1,000–1,500 ms cadence a two-sample stability
check costs at least one full interval. It is worth using as an *accelerator* (end early when stable) and as a *guard*
(don't end while wildly changing), never as the only signal.

## 5. Integration sketch for the recommended stack

### 5.1 What runs where

```
cpal input stream (native rate, mono-downmixed)            [existing capture.rs]
   └─ CaptureBuffer: samples + energy VAD                  [existing buffer.rs, vad.rs]
        ├─ (new) 16 kHz VAD stream → Silero VAD            [new vad_silero.rs, worker thread]
        │        every 20 ms frame, trailing-window run every 100 ms
        ├─ (new) EndpointState machine                     [new endpointing.rs]
        │        silence hangover, ASR-stability gates, hard caps
        └─ TranscriptionJob snapshots                      [existing transcription.rs]
                 └─ whisper-rs thread (Whisper base-q5_1)   [existing worker]
                        └─ bounded transcript ranges → IPC → Svelte
```

Everything stays in the one process, one worker thread, no new runtime. The only new files on disk are
`ggml-base-q5_1.bin` (59,707,625 B) and `ggml-silero-v6.2.0.bin` (885,098 B).

### 5.2 Module changes

| Module | Change |
| --- | --- |
| `src-tauri/src/microphone/vad.rs` | Keep the energy detector as the **onset** gate (3 × 20 ms confirmation is already good, and it is free). Add a second stage struct that owns the Silero decision for **release**, because that is the failure mode energy thresholds get wrong in a noisy room. Do not delete the energy path: it is deterministic, tested, and gives a 60 ms onset. |
| new `src-tauri/src/microphone/vad_silero.rs` | Owns the `whisper_rs::WhisperVadContext` and a resampler to 16 kHz. Runs on its own worker so a VAD pass can never block the cpal callback. Feeds `EndPointer` one decision per 20 ms frame. |
| new `src-tauri/src/microphone/endpointing.rs` | The state machine in §5.4. Pure logic over (activity, transcript snapshot, monotonic time) so it is unit-testable without audio, mirroring `vad.rs`'s style. |
| `src-tauri/src/microphone/transcription.rs` | (a) `TRANSCRIPTION_INTERVAL_MS` 1,500 → **1,000**, and reset the next-deadline at speech onset rather than only at capture start; (b) hold the pinned asset contract behind an enum so `whisperTinyQ5` / `whisperBaseQ5` can both validate; (c) on the endpoint release, schedule a final pass immediately (the existing `schedule_transcription(is_final = true)` path already does the right thing). |
| `src-tauri/src/microphone.rs` | `CaptureState` gains an `EndPoint` sub-state and a `Listening → SpeechDetected → EndpointPending → Finalizing` sub-phase on top of `MicrophonePhase::Recording`; expose only path-free fields (`turn_state`, `silence_ms`, `endpoint_reason`) to the WebView. Relay the endpoint decision into the existing stop path so there is exactly one final-transcript route. |
| `src-tauri/src/microphone/capture.rs` | No structural change: the 40 ms poll loop already calls `schedule_transcription` every iteration. It gains the VAD hand-off (send the downmixed samples to the VAD worker) and honours an endpoint-initiated stop. |
| `src-tauri/src/microphone/latency.rs` | Add two intervals that endpointing makes meaningful: **end-of-speech → final transcript** and **speech onset → first partial**. The tracker already has the shape; do not reuse `first_transcript_ms` (that one is anchored at Record). |
| `src-tauri/src/microphone/buffer.rs` | Unchanged limits (60 s / 32 MiB). Optionally expose the last speech sample index so the endpointing pass can trim the snapshot without copying. |
| `runtime-assets.json` | Two new entries (§5.3). |
| `DEPENDENCY-LICENCES.md` / `MODEL-NOTICE.txt` / `THIRD-PARTY-NOTICES.txt` | Record the Silero VAD asset (MIT) and the base-q5_1 file (MIT, same repository as tiny) exactly as the whisper tiny contract is recorded today. |

### 5.3 Download contract

Keep the existing discipline: immutable revision, exact byte count, SHA-256 verified before load, app-owned cache,
download only after explicit user action (today: after first speech). Two additions:

```jsonc
"sileroVadGgml": {
  "repository": "ggml-org/whisper-vad",
  "revision": "9ffd54a1e1ee413ddf265af9913beaf518d1639b",   // checked 2026-11-01
  "variant": "silero-v6.2.0-ggml",
  "file": {
    "path": "ggml-silero-v6.2.0.bin",
    "sha256": "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987",
    "size": 885098
  },
  "licence": { "spdx": "MIT", "source": "https://huggingface.co/ggml-org/whisper-vad" }
},
"whisperBaseQ5": {
  "repository": "ggerganov/whisper.cpp",
  "revision": "5359861c739e955e79d9a303bcbc70fb988958b1",   // same revision already pinned for tiny
  "variant": "base-q5_1-multilingual",
  "file": {
    "path": "ggml-base-q5_1.bin",
    "sha256": "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    "size": 59707625
  },
  "licence": { "spdx": "MIT", "source": "https://huggingface.co/ggerganov/whisper.cpp" }
}
```

Behavioural notes: the VAD asset is required before endpointing can be enabled, so it must be part of the same
"speech model" download gate rather than a second surprise download; the checkpoint choice (tiny vs base) should be an
explicit user-visible option, not a silent substitution, because the download size triples. If the asset is absent or its
digest fails, endpointing must degrade to the energy-VAD timeout path rather than disabling voice input.

### 5.4 Concrete endpointing design

All timers are relative to the **last confirmed speech frame**, never to a transcript result, so a slow ASR pass cannot
extend a turn indefinitely.

**Frames and windows.** 20 ms decision frames (matches `vad.rs`). Silero thresholds: `threshold = 0.5`,
`min_speech_duration_ms = 250`, `min_silence_duration_ms = 100` (segment splitting inside the VAD), `speech_pad_ms = 100`.
Silero is evaluated on a trailing 2,000 ms window every 100 ms (5 frames), which is well inside its per-frame cost
(RTF ≈ 0.0127 on one core) and avoids any assumption about incremental VAD state across calls — `whisper-rs` exposes the
whole pipeline (`detect_speech` → `segments_from_samples`), not a frame-probability stream.

**Onset.** Energy VAD: 3 consecutive frames at RMS ≥ 0.02 (60 ms, unchanged) → `SpeechDetected`; Silero must agree within
the next 100 ms window or the onset is discarded. On confirmed onset: reset the ASR deadline so the first partial lands
one interval later, and cancel a pending end-of-turn evaluation.

**End-of-turn pipeline**, evaluated on each 20 ms frame once silence begins:

1. **Silence timer** starts at the first frame Silero calls non-speech. If speech returns, reset to zero.
2. **Gate A — no transcript yet.** If the ASR partial for this turn is empty, wait `silence ≥ 700 ms`, then finish with
   an empty transcript (the current UI already handles empty results) or, if the client prefers, return to `Listening`.
3. **Gate B — stability.** If the last two applied partials are byte-identical, finish at `silence ≥ 480 ms`.
4. **Gate C — terminal punctuation.** If the newest partial ends in `.` `?` `!` (Whisper's own punctuation), finish at
   `silence ≥ 480 ms` even if it changed between passes.
5. **Gate D — still changing.** Extend the deadline to `750 ms`, then `1,000 ms`, then `1,200 ms` absolute. Never end
   earlier than 480 ms.
6. **Gate E — stale transcript.** If a new partial arrives after the deadline, wait for it, bounded to +500 ms; the final
   transcript must be built from audio that includes every frame before the last speech frame.
7. **Hard stop.** 1,200 ms without confirmable stability ends the turn anyway. Independent hard cap: the existing 60 s
   capture limit.
8. **Optional semantic stage (runner-up only).** When Gate D triggers, run smart-turn v3.2 int8 on the turn recording
   (zero-padded at the front to ≤8 s). `p(endpoint) ≥ 0.5` → end immediately; otherwise fall through to Gate D's timer.
   Budget it as ~10–65 ms and never let it block the frame loop.

Ordering constraint worth writing into code review: **VAD decides silence, ASR decides completeness, timers decide when
to stop waiting.** Reversing any two of those is how these systems end up cutting users off.

**What must not be done.** Do not end a turn from transcript text alone at 1,000–1,500 ms; do not use an interval-aligned
schedule as the endpoint (that is the current bug in disguise); do not let `min_silence_duration_ms` inside the VAD model
*be* the endpointing timeout — it is a segment splitter whose default is far shorter than a natural turn gap.

### 5.5 Barge-in: the VAD is not the hard part, the echo is

The mechanics are already in place: reading and generation share one cancellation boundary, the composer already offers
**Interrupt & record** which cancels the provider, stops local playback, and only then starts capture, and `CaptureState`
already rejects generations that race with an active capture. Making barge-in automatic means firing that same sequence
from a VAD onset instead of a click:

- Detect "user started speaking while the assistant is speaking or generating" as Silero speech for ≥120 ms after the
  energy onset gate (≈180 ms total, so a cough or a chair creak does not kill a reply).
- Order: request provider cancellation → stop local playback → start capture → begin the turn normally. Never start the
  TTS stop and the capture open in parallel; a failed playback stop must keep capture closed.
- Gate the trigger on the assistant's playback being audible to the microphone. Bottie has **no acoustic echo
  cancellation** — `ROADMAP.md` lists it as its own open Milestone 7 item — so automatic barge-in will hear Bottie's own
  voice and interrupt itself. Two honest options: (a) ship endpointing first with barge-in still manual, then (b) enable
  automatic barge-in behind an explicit setting with a conservative gate (300 ms onset, energy above a threshold derived
  from the measured playback level, and a short lockout right after playback starts). Do not promise hands-free barge-in
  in the same release as endpointing; the blocker is physics, not policy.

## 6. Latency

Two intervals matter, and Bottie can already measure both in-app (`MicrophoneLatency`):

| Interval | Recommended stack | Status | Basis |
| --- | --- | --- | --- |
| **End of user speech → final transcript** | ≈ **0.6–0.7 s** typical, ≤ 1.4 s worst case | estimated | 480 ms hangover + immediate release-triggered ASR pass + ~80–200 ms tiny/base inference on a ≤ 8 s window [+ 0–250 ms if a gate extends the hangover] |
| same, without the release-triggered pass | ≈ 1.1 s typical, ≤ 1.7 s | estimated | 480 ms hangover + up to one 1,000 ms interval + inference |
| **Speech onset → first partial transcript** | ≈ **1.1 s** typical (10th-percentile ~0.3 s if onset aligns with a scheduled pass) | estimated | deadline reset at onset + 1,000 ms interval + inference; today's 1,500 ms cadence with capture-start anchoring gives ≈ 1.6 s |
| **First byte of audio → VAD onset decision** | ≈ 60 ms | estimated | 3 × 20 ms energy frames, then a ≤100 ms Silero confirm window |
| **Silence → endpoint decision (no semantic stage)** | 480 ms – 1,200 ms | derived from design | §5.4 gates B–D |
| **Silero VAD frame cost** | RTF ≈ 0.0127 (one core) | third-party | TEN VAD's published comparison table |
| **smart-turn v3.2 int8 (optional stage)** | ~10 ms best case, ~65 ms on a 1× cloud instance | vendor-claimed | Pipecat blog/README |
| **Whisper tiny inference, 5 s window** | unknown | unmeasured | no measured number exists in this repository; the `final_transcript_ms` field produces it on the next probe |

Honesty notes. There is **no measured voice latency in this repository**: `microphone/latency.rs` records
`first_transcript_ms` (anchored at Record) and `final_transcript_ms` (anchored at native Stop handling), and the only
numbers in the tree (`1_725` / `245` in `microphone/tests.rs`) come from a synthetic clock fixture, not a real capture.
Every figure above is arithmetic over the design and over vendor numbers for the models, not a measurement on Bottie. The
first thing to do with this design is run it on the M3 Max and record both fields; if the measured
end-of-speech→final number is above ~1 s, move the ASR interval to 800 ms before touching anything else.

## 7. Risks and open measurements

1. **Licence review is now on the critical path for every ASR upgrade.** Whisper stays MIT and clean; everything else
   interesting (Parakeet, Nemotron, SenseVoice) needs a licence decision before engineering. If the reviewer concludes
   `cc-by-4.0` is acceptable with attribution, Parakeet q4_0 becomes the strongest accuracy candidate and this
   recommendation changes for the model, not for the architecture.
2. **The Silero path in `whisper-rs` is unexercised.** `WhisperVadContext` exists in the pinned crate and the model file is
   verified, but Bottie has never loaded either, and `whisper-rs` documents its VAD pipeline as batch-shaped
   (`segments_from_samples`) with "undocumented use" on `probabilities()`. The trailing-window workaround is a design
   accommodation, not a verified streaming API. Measure: does it hold 5 Hz on the M3 Max with a 2 s window, and what is
   the per-call allocation cost?
3. **`whisper-rs-sys` vendored-whisper.cpp version is unknown.** The crate is what Bottie builds, not upstream master, and
   I could not determine which whisper.cpp commit 0.15.0 vendors. If it predates the VAD model-load validation or the
   Parakeet work, both "available upstream" claims are unavailable in Bottie until the binding updates. Probe: print
   `WHISPER_CPP_VERSION` from the crate at build time and record it.
4. **No measured RSS for any Whisper checkpoint in Bottie.** The 32 MB → 60 MB model change is documented as weights; the
   resident-set delta is unknown, and the memory story is one of the reasons `audio-cpp-evaluation.md` rejected Nemotron.
   Measure tiny vs base vs small RSS in the same process shape before choosing.
5. **ORT's macOS x64 gap blocks the whole semantic-turn-detector runner-up.** `runtime-assets.json` pins three archives;
   `x86_64-apple-darwin` is missing. Adding smart-turn (or Silero via `ort`) requires a fourth verified archive, a licence
   review for that target, and a packaging check — a real cost, and the main reason the recommendation prefers
   `whisper-rs`'s own ggml VAD.
6. **The energy VAD's thresholds are untested against real rooms.** 0.02 / 0.012 RMS are deterministic and tested, but
   there is no evidence they survive a noisy room, a headset with AGC, or a fan. The Silero stage exists to fix release
   detection specifically; if it is deferred, endpointing quality is speculative.
7. **Barge-in without AEC will self-interrupt.** Until `ROADMAP.md`'s AEC item is closed, automatic barge-in must stay
   opt-in and gated. Do not merge the two capabilities into one release note.
8. **A 1,000 ms interval doubles ASR CPU while the user speaks.** Whisper tiny is cheap on a desktop, but "cheap × 2 per
   second continuously" is a battery/thermal claim nobody has measured on the release targets.
9. **`sherpa-rs` staleness.** The runner-up's Rust wrapper last released 2025-10-05 against a moving upstream, and it
   downloads prebuilt binaries — a supply-chain review of its own, distinct from the Apache-2.0 model licences.
10. **Cheap fix if the recommendation is wrong.** Because the whole design hangs off two constants
    (`TRANSCRIPTION_INTERVAL_MS`, the hangover), a retreat to today's push-to-talk behaviour is a config change, not a
    revert. That property is deliberate: it is why the in-process Whisper path wins over a better model that requires new
    infrastructure.

## 8. Sources

Primary sources checked 2026-11-01.

- whisper.cpp repository and VAD/stream documentation — <https://github.com/ggml-org/whisper.cpp>
- whisper.cpp README: memory table, Silero VAD models and options — <https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/README.md>
- whisper.cpp `examples/stream` README (step 0 / "very basic VAD detector") — <https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/examples/stream/README.md>
- whisper.cpp `examples/parakeet-cli` README (Parakeet TDT conversion and CLI) — <https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/examples/parakeet-cli/README.md>
- whisper.cpp v1.9.5 release notes (Parakeet, VAD validation, ANEForge) — <https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.5>
- whisper.cpp model repository (exact file sizes, pinned revision `5359861c…`) — <https://huggingface.co/ggerganov/whisper.cpp>
- whisper.cpp Silero VAD ggml models (v5.1.2 and v6.2.0, 885,098 bytes, MIT) — <https://huggingface.co/ggml-org/whisper-vad>
- `whisper-rs` 0.16.0 crate metadata (newest version, features) — <https://crates.io/crates/whisper-rs>
- `whisper-rs` 0.16.0 API index — <https://docs.rs/whisper-rs/0.16.0/whisper_rs/all.html>
- `WhisperVadContext` documentation — <https://docs.rs/whisper-rs/0.16.0/whisper_rs/struct.WhisperVadContext.html>
- `whisper-rs-sys` 0.15.0 crate metadata (vendored C/C++ line counts) — <https://crates.io/crates/whisper-rs-sys>
- `faster-whisper` (MIT) — <https://github.com/SYSTRAN/faster-whisper>
- CTranslate2 (MIT) — <https://github.com/OpenNMT/CTranslate2>
- distil-whisper `distil-large-v3` (MIT, English-only, 756M params) — <https://huggingface.co/distil-whisper/distil-large-v3>
- Parakeet-TDT-0.6B-v3 model card (`cc-by-4.0`, 25 languages) — <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3>
- Parakeet ggml conversions (q4_0 / q8_0 / f16 byte sizes) — <https://huggingface.co/ggml-org/parakeet-GGUF>
- Nemotron streaming ASR model card (`nvidia-open-model-license`, English, 1.12 s frame size) — <https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b>
- SenseVoice-Small model card (`license: other`, FunASR MODEL_LICENSE) — <https://huggingface.co/FunAudioLLM/SenseVoiceSmall>
- FunASR model licence text — <https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE>
- Moonshine model repository (MIT, split ONNX graphs, int8 sizes) — <https://huggingface.co/moonshine-ai/moonshine>
- Moonshine paper — <https://arxiv.org/abs/2410.15608>
- Vosk models list (sizes, per-language WER, Apache-2.0) — <https://alphacephei.com/vosk/models>
- `vosk` Rust crate (0.3.1, 2024-10-27) — <https://crates.io/crates/vosk>
- sherpa-onnx — <https://github.com/k2-fsa/sherpa-onnx>
- `sherpa-rs` crate (0.6.8, MIT, 2025-10-05) — <https://crates.io/crates/sherpa-rs>
- sherpa-onnx streaming zipformer English int8 model (exact encoder/decoder/joiner sizes, Apache-2.0) — <https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26>
- Pipecat smart-turn v3.2 code and model usage notes (BSD-2-Clause, 8 s mono 16 kHz input contract) — <https://github.com/pipecat-ai/smart-turn>
- smart-turn v3 model card (BSD-2-Clause, Whisper Tiny backbone, 8M params, int8/fp32 sizes) — <https://huggingface.co/pipecat-ai/smart-turn-v3>
- smart-turn v3.2 CPU benchmark (92.63 % accuracy, per-language table) — <https://huggingface.co/pipecat-ai/smart-turn-v3/blob/main/benchmarks/smart-turn-v3.2-cpu.md>
- LiveKit turn detector model card — <https://huggingface.co/livekit/turn-detector>
- LiveKit model licence (standalone use prohibited) — <https://huggingface.co/livekit/turn-detector/blob/main/LICENSE>
- LiveKit turn-taking documentation index — <https://docs.livekit.io/agents/logic/turns/>
- TEN VAD (frame-level streaming VAD, RTF/lib-size comparison) — <https://github.com/TEN-framework/ten-vad>
- TEN VAD licence (Apache-2.0 with Agora conditions) — <https://github.com/TEN-framework/ten-vad/blob/main/LICENSE>
- Silero VAD — <https://github.com/snakers4/silero-vad>
- `webrtc-vad` Rust crate (MIT, 0.4.0, 2019) — <https://crates.io/crates/webrtc-vad>
- `py-webrtcvad` (combined licence reported NOASSERTION) — <https://github.com/wiseman/py-webrtcvad>
- OpenAI Realtime VAD guide (`server_vad` defaults: threshold 0.5, `prefix_padding_ms` 300, `silence_duration_ms` 500; `semantic_vad` eagerness) — <https://developers.openai.com/api/docs/guides/realtime-vad>
- Apple `SFSpeechRecognizer` (network-based service, per-locale on-device, one-minute cap) — <https://developer.apple.com/documentation/speech/sfspeechrecognizer>
- Apple `SpeechAnalyzer` (macOS/iOS 26.0+, asset inventory, volatile ranges) — <https://developer.apple.com/documentation/speech/speechanalyzer>
- Windows `SpeechRecognizer` (WinRT: `ContinuousRecognitionSession`, `HypothesisGenerated`, `Timeouts`) — <https://learn.microsoft.com/en-us/uwp/api/windows.media.speechrecognition.speechrecognizer>
- Bottie's own measurements reused rather than repeated — [audio.cpp evaluation](../audio-cpp-evaluation.md)
