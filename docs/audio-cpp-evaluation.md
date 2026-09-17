# audio.cpp evaluation

Last reviewed: 2026-09-17

This document evaluates [audio.cpp](https://github.com/0xShug0/audio.cpp) as a source of local speech recognition and
text-to-speech for Bottie. It is a feasibility evaluation, not a decision and not a commitment to integrate. Nothing in
Bottie depends on audio.cpp, no dependency was added, and Milestone 7 remains complete on its existing stack.

The measurements below are reproducible through `scripts/audio-cpp-spike/`, and the machine-readable record is
[`audio-cpp-measurements.json`](audio-cpp-measurements.json). The spike tooling is deliberately outside the `src-tauri`
workspace and is not built by Bottie's release, packaging, or test pipelines.

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

A C++17 audio inference framework on vendored `ggml`, Apache-2.0, from ShugoAI LLC. Release v0.8.0 (2026-09-15) claims
80+ model families across text-to-speech, speech recognition, voice conversion, diarization, alignment, and separation.
The project moves very fast — v0.1 to v0.8 in roughly three months.

Its primary surfaces are `audiocpp_cli`, `audiocpp_server`, and a WebUI, none of which Bottie wants. The relevant
surface is the C ABI in `include/audiocpp.h`, off by default behind `-DAUDIOCPP_BUILD_C_API=ON`.

## Measured: the C ABI builds small and fast when the model set is restricted

Built at commit `c0b26a505468152ec1d0fc06ac252c824408fc07` on Ubuntu 24.04, GCC 13.3.0, CMake 3.28.3, a 4-core Xeon at
2.10 GHz, with no compiler cache available. Configuration is in `scripts/audio-cpp-spike/build-libaudiocpp.sh`:
Release, C ABI on, CPU backend, portable rather than host-native CPU kernels, examples and tests off, and
`-DAUDIOCPP_MODEL_SET=custom -DAUDIOCPP_MODELS=supertonic,nemotron_asr`.

| | Measured |
| --- | --- |
| Object files compiled | 246 |
| Clean build wall time | 80, 82, and 88 seconds across three clean runs |
| Build tree | 94,918,654 bytes |
| `libaudiocpp.so.0.1.0` | 5,663,520 bytes |
| ABI version reported | 0.2.0, SOVERSION 0 |

Restricting the model set is what makes this cheap. `full` links all 80+ families; `core` links none; `custom` takes a
list. The two families Bottie would plausibly use cost 80 seconds and 5.4 MiB. The cost of `full` was not measured.

`ggml`, `sentencepiece`, `cJSON`, and `libyaml` are absorbed as static archives, so the library needs only `libgomp`,
`libstdc++`, `libm`, `libgcc_s`, and `libc` at runtime. `libgomp` is the one packaging consideration; `ENGINE_ENABLE_OPENMP=OFF`
would remove it.

## Measured: it can coexist with whisper-rs in one process

This was the question that most threatened an in-process integration. Bottie already links `whisper-rs`, which
statically links its own copy of `ggml`. A second `ggml` in the same process is a plausible way to get silent symbol
interposition.

audio.cpp handles this deliberately. `src/capi/audiocpp.map` restricts the ELF dynamic symbol table to `audiocpp_*`,
with a source comment recording that without it the library re-exported 2,767 dynamic symbols where the header declares
69.

The built library confirms it: 74 exported `audiocpp_*` symbols, zero other exported symbols besides the `AUDIOCPP_0`
version node itself, and zero exported `ggml` or `gguf` symbols.

`scripts/audio-cpp-spike/ffi-probe` then builds a single Rust binary that links both `libaudiocpp` and Bottie's pinned
`whisper-rs`. It links. The executable statically contains 288 defined `ggml_*` symbols from whisper-rs's own build, and
resolves zero `ggml` symbols dynamically — so whisper's copy stays inside the executable, audio.cpp's stays inside its
shared library, and there is no path between them. Both libraries ran in the same process: audio.cpp created and
enumerated its registry, after which whisper.cpp returned its CPU feature report.

This is a structural result, not a stress test, and the wording matters. Without weights no audio.cpp `ggml` compute
graph ran, and the probe calls only whisper.cpp's system-info entry point, so the linker keeps just 2 `whisper_*`
symbols. The claim it supports is that the two `ggml` copies load, bind, and stay isolated — not that both have been
run under concurrent inference load. Confirming that needs weights for both.

## Measured: the GPL surface is absent from the artifact, not merely disabled

`AUDIOCPP_STATIC_ESPEAK=ON` statically links GPL-3.0-or-later eSpeak-ng. `DEPENDENCY-LICENCES.md` records that no GPL,
AGPL, LGPL, or SSPL declaration appears in Bottie's reviewed graphs, so that option must stay off.

Off is the default, and the built library goes further than the option implies: although `espeak_phonemizer.cpp`
compiles into `engine_core`, neither Supertonic nor Nemotron references it, so the linker discards it. The shipped
library contains zero eSpeak symbols, zero eSpeak strings, and no `libespeak` name for the runtime `dlopen` path to
find. The families that do need it — SanoTTS, Inflect v2, and optionally Kokoro — are precisely the ones Bottie would
not link.

The framework itself is Apache-2.0, compatible with Bottie's MIT licence.

## Not measured

`huggingface.co` is denied by this environment's egress policy, so no model weights could be fetched and no inference
ran. The following are open:

- text-to-speech and speech-recognition realtime factors, and peak resident memory;
- model download sizes, and therefore the real cost of the download contract;
- audio quality, which for a text-to-speech swap is the entire point;
- the streaming chunk cadence each family asks for, and whether it agrees with Bottie's 1,500 ms interval;
- macOS arm64, Windows x64, Metal, and CUDA builds;
- the cost of `AUDIOCPP_MODEL_SET=full`.

`scripts/audio-cpp-spike/ffi-probe` already implements the first, third, and fourth of these; it reports them when
`AUDIOCPP_SPIKE_MODEL` points at a local package, and reports `"textToSpeech": null` when it does not.

## What integration would still cost

**Bottie builds the library itself.** `BUILD_TARGETS` in upstream's `release.yml` is `audiocpp_cli audiocpp_server
audiocpp_gguf`. Releases do not ship `libaudiocpp`, so Bottie's CI would own CMake builds for macOS arm64 and x64,
Windows x64, and Linux x64. At 80 seconds per clean CPU build this is small, but it is new CI surface, and the
non-Linux and GPU-backend costs are unmeasured.

**Speech recognition is a model swap, not a drop-in.** There is no Whisper family, and upstream states whisper.cpp GGUF
files are not loadable. The pinned `ggml-tiny-q5_1.bin` contract does not carry over. Streaming multilingual candidates
marked `supported` are `nemotron_asr` (Nemotron 3.5 Streaming 0.6B) and `qwen3_asr` (0.6B/1.7B); `moonshine_asr` is
English-only and marked `experimental`. All are far larger than 32 MB, so the "downloaded only after capture first
produces speech" story in the README gets more expensive.

**Text-to-speech needs a playback path Bottie does not have.** `audiocpp_result_audio` returns borrowed interleaved
`f32` plus a sample rate. Replacing the `tts` crate means Bottie owns playback — `cpal` is already a dependency — and
`speech/voices.rs` shifts from enumerating operating-system voices to enumerating model voices. Supertonic 3 is the
candidate: `status: supported`, offline and streaming, 30+ languages. Kokoro is more prominent upstream but is
`status: community` and offline-only.

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
whisper-rs, and GPL contamination through eSpeak — are measurably absent.

The asymmetry is between the two capabilities. Speech recognition already works, and replacing it costs a rewritten
download contract and a model roughly twenty times larger for a quality gain that is unmeasured here. Text-to-speech is
where Bottie is weakest, where the platform inconsistency is genuine, and where a bundled neural voice would sound the
same everywhere.

The next step that would settle it is not more code. It is running the existing probe against real Supertonic 3 weights
on target hardware and listening to the output.
