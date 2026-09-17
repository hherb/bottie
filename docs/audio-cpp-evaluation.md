# audio.cpp evaluation

Last reviewed: 2026-09-17

This document evaluates [audio.cpp](https://github.com/0xShug0/audio.cpp) as a source of local speech recognition and
text-to-speech for Bottie. It is a feasibility evaluation, not a decision and not a commitment to integrate. Nothing in
Bottie depends on audio.cpp, no dependency was added, and Milestone 7 remains complete on its existing stack.

The measurements below are reproducible through `scripts/audio-cpp-spike/`, and the machine-readable record is
[`audio-cpp-measurements.json`](audio-cpp-measurements.json). The spike tooling is deliberately outside the `src-tauri`
workspace and is not built by Bottie's release, packaging, or test pipelines.

Two hosts are recorded: Linux x64, which measured the build and the linkage but could not reach `huggingface.co`, and
macOS arm64 on an Apple M3 Max, which repeated those and then ran both models against real weights. Everything below
that quotes a realtime factor, a transcript, or a memory figure comes from the macOS run.

## What Bottie has today

Voice already works. The composer captures through `cpal`, a Rust worker runs bounded native voice activity detection,
streaming recognition uses `whisper-rs` `=0.16.0` against the pinned multilingual Whisper tiny Q5 model
(`ggml-tiny-q5_1.bin`, 32,152,673 bytes, SHA-256 contract in `runtime-assets.json`), and playback goes through the `tts`
crate `=0.26.3` to whatever voices the operating system provides. Recognition runs on a fixed 1,500 ms interval
(`TRANSCRIPTION_INTERVAL_MS`).

Two weaknesses motivated looking further. Whisper tiny is the smallest useful multilingual model and its accuracy shows
it. And text-to-speech quality is not Bottie's at all: it is Speech Dispatcher with eSpeak NG voices on Linux and system
voices elsewhere, so the same response sounds different on every machine and is worst on Linux.

## What audio.cpp is

A C++17 audio inference framework on vendored `ggml`, Apache-2.0, from ShugoAI LLC. It claims 80+ model families across
text-to-speech, speech recognition, voice conversion, diarization, alignment, and separation. The project moves very
fast — v0.1 to v0.8 in roughly three months.

Both hosts built commit `c0b26a5`, which is main a little past v0.8.0 rather than v0.8.0 itself: the only tag on it is
`last-docker-build`, which moves, and the nearest release tag `v0.8.0` (`4af14322`) is an ancestor. That is a fair
illustration of the churn — an integration would pin a release tag, not this.

Its primary surfaces are `audiocpp_cli`, `audiocpp_server`, and a WebUI, none of which Bottie wants. The relevant
surface is the C ABI in `include/audiocpp.h`, off by default behind `-DAUDIOCPP_BUILD_C_API=ON`.

## Measured: the C ABI builds small and fast when the model set is restricted

Configuration is in `scripts/audio-cpp-spike/build-libaudiocpp.sh`: Release, C ABI on, portable rather than host-native
CPU kernels, examples and tests off, and `-DAUDIOCPP_MODEL_SET=custom -DAUDIOCPP_MODELS=supertonic,nemotron_asr`.

| | Linux x64 | macOS arm64 |
| --- | --- | --- |
| Toolchain | GCC 13.3.0, CMake 3.28.3 | AppleClang 21.0.0, CMake 3.31.1 |
| Machine | 4-core Xeon @ 2.10 GHz | Apple M3 Max, 16 cores |
| Backends CMake enabled | CPU | CPU, BLAS (Accelerate), Metal |
| Object files compiled | 246 | 256 |
| Clean build wall time | 80, 82, 88 s | 23, 23, 23 s |
| Build tree | 94,918,654 bytes | 116,715,520 bytes |
| Library | 5,663,520 bytes | 5,182,592 bytes |
| ABI version reported | 0.2.0, SOVERSION 0 | 0.2.0, SOVERSION 0 |

Restricting the model set is what makes this cheap. `full` links all 80+ families; `core` links none; `custom` takes a
list. The two families Bottie would plausibly use cost well under two minutes on either host. The cost of `full` was
not measured.

The two builds are **not backend-equivalent**, and the spike is what made them differ. It pins no compute backend, so
CMake takes whatever the host offers: on macOS that silently added Accelerate and Metal. The build script now reports
the enabled backends in its summary so the difference cannot pass unnoticed again.

`ggml`, `sentencepiece`, `cJSON`, and `libyaml` are absorbed as static archives. On Linux the library then needs only
`libgomp`, `libstdc++`, `libm`, `libgcc_s`, and `libc`; on macOS it needs only operating-system frameworks
(Accelerate, Metal, MetalKit, Foundation, CoreFoundation, `libc++`, `libobjc`, `libSystem`) and **no third-party
dylib at all**.

### OpenMP is a portability trap, and on macOS a packaging one

`ENGINE_ENABLE_OPENMP` defaults ON and is a `find_package(OpenMP REQUIRED)`. Apple clang ships no OpenMP runtime, so
the original script aborted configure on macOS before compiling anything. Upstream hits the same wall and resolves it
the same way: its own `scripts/build_metal.sh` defaults `--openmp OFF`. The script now does that on Darwin.

Homebrew's `libomp` does satisfy the requirement, and that build works — 20 seconds, 5,186,720 bytes, the same 74
exported symbols. But it links `/opt/homebrew/opt/libomp/lib/libomp.dylib` by absolute Homebrew path, which is not
present on an end user's Mac. **OpenMP off is the only configuration a shipped macOS build could use** without
vendoring `libomp` and rewriting its install name. On Linux, `libgomp` remains the one packaging consideration.

## Measured: it can coexist with whisper-rs in one process

This was the question that most threatened an in-process integration. Bottie already links `whisper-rs`, which
statically links its own copy of `ggml`. A second `ggml` in the same process is a plausible way to get silent symbol
interposition.

audio.cpp handles this deliberately. `src/capi/audiocpp.map` restricts the ELF dynamic symbol table to `audiocpp_*`,
with a source comment recording that without it the library re-exported 2,767 dynamic symbols where the header declares
69.

The built library confirms it: 74 exported `audiocpp_*` symbols, zero other exported symbols besides the `AUDIOCPP_0`
version node itself, and zero exported `ggml` or `gguf` symbols. macOS produces the same 74 and the same zeroes through
a different mechanism — there is no ELF version script on Mach-O — so the encapsulation is not an artefact of one
linker.

`scripts/audio-cpp-spike/ffi-probe` then builds a single Rust binary that links both `libaudiocpp` and Bottie's pinned
`whisper-rs`. It links on both hosts. The executable statically contains whisper-rs's own `ggml` symbols (288 on
Linux, 562 on macOS) and resolves **zero** `ggml` symbols dynamically, against 20 `audiocpp_*` symbols that do resolve
through the dylib — so whisper's copy stays inside the executable, audio.cpp's stays inside its shared library, and
there is no path between them.

The Linux run had to hedge here: without weights no audio.cpp `ggml` compute graph ran, so the claim was that the two
copies load and stay isolated, not that both survive inference. **The macOS run discharges that hedge.** audio.cpp
loaded Supertonic 3, ran a full synthesis through its own `ggml` and Metal, freed the registry, and whisper.cpp then
returned its CPU feature report from the same process. Still not concurrent load from two threads, but no longer
merely structural.

## Measured: the GPL surface is absent from the artifact, not merely disabled

`AUDIOCPP_STATIC_ESPEAK=ON` statically links GPL-3.0-or-later eSpeak-ng. `DEPENDENCY-LICENCES.md` records that no GPL,
AGPL, LGPL, or SSPL declaration appears in Bottie's reviewed graphs, so that option must stay off.

Off is the default, and the built library goes further than the option implies: although `espeak_phonemizer.cpp`
compiles into `engine_core`, neither Supertonic nor Nemotron references it, so the linker discards it. The shipped
library contains zero eSpeak symbols, zero eSpeak strings, and no `libespeak` name for the runtime `dlopen` path to
find. The macOS artifact repeats this exactly: zero eSpeak strings and zero `phonemi*` strings. The families that do
need it — SanoTTS, Inflect v2, and optionally Kokoro — are precisely the ones Bottie would not link.

The framework itself is Apache-2.0, compatible with Bottie's MIT licence.

## Measured: text-to-speech is fast, and the voice is the point

Supertonic 3 (`supertonic_3_orig`, 454,072,836 bytes), voice `M1`, 224 characters of English producing 14.7 seconds of
44.1 kHz mono audio.

| Backend | Load | Synthesis | Realtime factor |
| --- | --- | --- | --- |
| CPU, 4 threads | 12.8 s | 3.26 s | **4.5×** |
| Metal | 4.8 s | 1.15 s | **12.7×** |

The output measures as plausible speech rather than noise or silence — −6.4 dBFS peak, −25.8 dBFS RMS, 28 % of samples
near-silent, which is the inter-word structure of speech. But the measurement is not the finding. **Listened to, the
voice is superb**, and that is the answer the whole evaluation was waiting on: the gap against Speech Dispatcher with
eSpeak NG on Linux is not incremental.

Two costs sit behind the realtime factor. Load is dominated not by weights but by Metal: 8.4 of the first run's 12.8
seconds were `ggml_metal_library_init` compiling the embedded shader library, and it runs **even when the requested
backend is `cpu`**, because the backend is compiled in. The operating system caches it, so later runs load in 4.8 s.
Any integration pays this once per process, not per utterance, and would want to warm it off the interaction path.

## Measured: speech recognition is far faster than it needs to be

Three families were measured against the same 14.7 seconds of 16 kHz mono audio, 4 threads, median of three runs.
All three are far faster than realtime, so speed is not the axis that decides this — size is.

| Family | Weights | CPU | Metal | Peak RSS | Languages | Status |
| --- | --- | --- | --- | --- | --- | --- |
| `nemotron_asr` | 930.6 MB | 21.3× | **72.1×** | 2,244 MB | 40 | supported |
| `sense_asr` (SenseVoice-Small) | 254.2 MB | 39.7× | 42.6× | 556 MB | 22 + auto-detect | community |
| `kroko_asr` | 167.8 MB | **33.4×** | 19.7× | **441 MB** | English only as shipped | community |
| *Whisper tiny Q5, for reference* | *32.2 MB* | — | — | — | *multilingual* | *shipping today* |

Two things in that table are worth stating plainly. Metal helps the large model a great deal and **hurts the small
ones** — Kroko is nearly twice as fast on CPU as on Metal, because GPU dispatch overhead dominates when the model is
this small. And peak resident set tracks weight size at roughly 2.4×, so the memory question is decided at model
choice, not at runtime.

The transcripts were near-identical. Every family made the same single error — the proper noun `Bottie` heard as
`Body` — and otherwise differed only in formatting:

- `nemotron_asr` punctuates sentences but writes `real time`, `one off`;
- `kroko_asr` punctuates and hyphenates correctly (`real-time`, `one-off`), but dropped the final period;
- `sense_asr` gets the word forms right but **emits almost no punctuation** — no commas, no sentence breaks — which
  matters for dictated text that a human then reads.

**This is an upper bound, not a field result**: the input was this spike's own Supertonic output, which is clean
synthetic studio speech. Microphone audio with room noise is the case that matters and is still unmeasured.

**The streaming cadences disagree.** `audiocpp_stream_policy` reports that `nemotron_asr` wants 16,000 samples per
push — exactly 1,000 ms at 16 kHz. Bottie recognizes on a fixed 1,500 ms interval (`TRANSCRIPTION_INTERVAL_MS`).
That is a retune, not a redesign, but it is not free. Note also an ABI wart worth guarding against: the call fills
`out_preferred_chunk_samples` and leaves `out_preferred_chunk_seconds` at `0.0`, so a caller reading only the seconds
field silently gets no cadence at all.

Memory is the real cost, and it is the one lever that moves. Peak resident set was **2.24 GB for Nemotron** against
1.06 GB for synthesis, in separate processes. Dropping to SenseVoice-Small takes recognition to 556 MB and to Kroko
441 MB — a 4 to 5× reduction — which changes a Bottie process holding both models from roughly 3.3 GB to under 1.7 GB.

### Two caveats on the small models

`kroko_asr` advertises ten languages in `model_specs/kroko_asr.json`, but the only GGUF package audio.cpp ships is
`kroko-en-community-64-l-q8_0.gguf` — **English only**. The language list describes Kroko's upstream model line, not
the weight file available here. As shipped it cannot replace multilingual Whisper tiny; it could serve an English fast
path.

`sense_asr` has a hard runtime dependency the other two do not: it segments with Silero VAD and loads
`assets/framework/models/silero_vad/silero_vad_16k.safetensors` (1.2 MB) **from a path relative to the working
directory**, failing with `Silero VAD model path does not exist` otherwise. Bottie would have to ship that asset and
control the working directory. It also requires 16 kHz input exactly, erroring on 44.1 kHz where Nemotron and Kroko
resample internally.

Both are `status: community` rather than `supported`, which is upstream's own signal about how much to rely on them.

## Not measured

- Windows x64 and macOS x64 builds; CUDA, Vulkan, and HIP backends.
- The cost of `AUDIOCPP_MODEL_SET=full`.
- Accuracy on real microphone audio, long-form stability, and any language other than English.
- Concurrent inference from audio.cpp and whisper-rs on two threads at once.

## What integration would still cost

**Bottie builds the library itself.** `BUILD_TARGETS` in upstream's `release.yml` is `audiocpp_cli audiocpp_server
audiocpp_gguf`. Releases do not ship `libaudiocpp`, so Bottie's CI would own CMake builds for macOS arm64 and x64,
Windows x64, and Linux x64. At 23 to 88 seconds per clean build this is small in wall time, but it is new CI surface,
it needs a per-platform OpenMP decision (off on macOS, `libgomp` on Linux), and Windows and the GPU backends remain
unmeasured.

**Speech recognition is a model swap, not a drop-in.** There is no Whisper family, and upstream states whisper.cpp GGUF
files are not loadable. The pinned `ggml-tiny-q5_1.bin` contract does not carry over. Streaming multilingual candidates
marked `supported` are `nemotron_asr` (Nemotron 3.5 Streaming 0.6B) and `qwen3_asr` (0.6B/1.7B); `moonshine_asr` is
English-only and marked `experimental`. The numbers are now concrete: `nemotron_asr_q8_0` is 930,620,256 bytes against
Whisper tiny Q5's 32,152,673 — **29 times larger** — and costs 2.24 GB resident. The "downloaded only after capture
first produces speech" story in the README survives, but it is now a near-gigabyte download and a serious memory
commitment rather than a 32 MB one. Against that, recognition runs at 21× realtime on CPU and transcribes clean speech
almost perfectly.

The smaller families narrow that gap without closing it. SenseVoice-Small is 254 MB and 556 MB resident for 22
languages; Kroko is 168 MB and 441 MB resident but English-only as packaged. Both are `community` status and both are
still five to eight times Whisper tiny's download. Of the 21 ASR families upstream ships, none is both multilingual
and comparable to 32 MB.

**Text-to-speech needs a playback path Bottie does not have.** `audiocpp_result_audio` returns borrowed interleaved
`f32` plus a sample rate — 44.1 kHz mono in practice. Replacing the `tts` crate means Bottie owns playback — `cpal` is
already a dependency — and `speech/voices.rs` shifts from enumerating operating-system voices to enumerating model
voices. Supertonic 3 is the candidate and it measured well: `status: supported`, offline and streaming, 30+ languages,
454 MB, 4.5× realtime on CPU and 12.7× on Metal, 1.06 GB resident. Kokoro is more prominent upstream but is
`status: community` and offline-only.

**Streaming would need a cadence change.** `nemotron_asr` asks for 1,000 ms chunks; the capture loop pushes on 1,500 ms.
Either the interval moves or Bottie pushes at the model's cadence and keeps its own decision window.

**Model licences are Bottie's to track.** `model_specs/*.json` carries no licence field. The pinning discipline in
`runtime-assets.json` stays manual per model. Packages are single-file GGUFs on `audio-cpp/audio.cpp-gguf`, which suits
that discipline: one file, one SHA-256, exactly as Whisper tiny is pinned now. Upstream's Python `model_manager_v2.py`
is not needed; `hf-hub` from Rust already does this.

**The trust boundary is the real decision.** The C ABI is the only one of upstream's three surfaces that keeps a session
warm in-process, which is what streaming recognition latency wants. But it puts a large C++ and `ggml` surface inside
the process that owns credentials, the SQLite store, and provider traffic, against Bottie's stated posture that Rust
owns trust. The `local-image-worker` sidecar is the established precedent and the safer shape; its cost is a weight
reload per CLI invocation, or a supervised local port.

**Upstream churn.** The ABI is 0.x, `audiocpp_build_version()` defaults to the literal `dev` unless `AUDIOCPP_VERSION`
is set at configure time, and upstream documents that ABI `0.1` covers two different surfaces. Any integration should
pin a released tag and gate on `audiocpp_abi_version()`.

## Two findings worth noting

Requesting `supertonic,nemotron_asr` produced a registry reporting four families: `marblenet_vad` and `silero_vad` are
linked unrequested. Both carry upstream's `Bundled` runtime tag, meaning their weights ship inside the library and need
no download. Bottie has its own voice activity detection in `microphone/vad.rs` and does not need these, but they cost
nothing and arrive regardless.

The C ABI is better designed than the project's CLI-first presentation suggests: opaque handles only, no exception
escapes, thread-local error detail, parent handles kept alive so free order is irrelevant, runtime option introspection
so adding a model family never changes the header, and pull-based streaming with no callback crossing the FFI boundary.
That last property maps directly onto the existing capture loop in `microphone/transcription.rs`.

## Assessment

Nothing measured here blocks an integration, and two things that could have blocked it — symbol collision with
whisper-rs, and GPL contamination through eSpeak — are measurably absent on both hosts.

The asymmetry between the two capabilities is now sharper, not softer, because both sides have numbers.

**Text-to-speech is worth it.** This was the open question the Linux run closed with "listen to the output", and the
output has now been listened to: the voice is superb, it is the same voice on every platform, and it replaces the
weakest thing Bottie ships. 454 MB, 4.5× realtime on CPU, 1.06 GB resident. The costs are real but bounded, and they
buy a capability Bottie cannot get any other way.

**Speech recognition is not, yet.** It is technically excellent — 21× realtime on CPU for Nemotron, 40× for
SenseVoice-Small, near-perfect transcripts from all three families tried — but those transcripts were of synthetic
studio speech, which is the easy case, and the thing they would replace already works. The price is a model 5 to 29
times larger, 441 MB to 2.24 GB resident, a rewritten download contract, and a cadence change, for an accuracy gain
over Whisper tiny that has still not been measured on real microphone audio.

Surveying the smaller families did change the shape of that trade. If recognition is ever revisited,
**SenseVoice-Small is the candidate to beat, not Nemotron**: 3.7× less memory, twice as fast on CPU, 22 languages plus
auto-detect, at the cost of `community` status, a Silero VAD asset dependency, a strict 16 kHz input requirement, and
missing punctuation. The deciding measurement is still the same one and it is not a code problem: run the candidates
against real microphone audio in the languages Bottie's users actually speak, and compare against Whisper tiny.

So the two halves need not move together. Adopting Supertonic 3 for text-to-speech while leaving `whisper-rs` in place
is a coherent position, and the coexistence result is precisely what makes it available: both `ggml` copies proved
they can share a process, including with audio.cpp actually running inference.

The trust-boundary question in the section above is untouched by any of this and remains the real decision.
