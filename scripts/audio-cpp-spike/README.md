# audio.cpp FFI spike

Throwaway tooling behind [`docs/audio-cpp-evaluation.md`](../../docs/audio-cpp-evaluation.md),
which evaluates [audio.cpp](https://github.com/0xShug0/audio.cpp) as a source of
local speech recognition and text-to-speech for Bottie.

Nothing here is part of Bottie's build, packaging, tests, or CI. The probe crate
is deliberately outside the `src-tauri` workspace, and no Bottie source file
depends on it. Delete the directory when the evaluation is settled.

## What it answers

1. Can audio.cpp's C ABI be built with only the model families Bottie needs, and
   how large and how slow is that build?
2. Does the resulting `libaudiocpp` bind from Rust?
3. Can its statically linked ggml coexist in one process with the ggml that
   whisper-rs already links into Bottie?
4. With weights, what is the text-to-speech realtime factor on CPU, and what
   chunk cadence does the streaming path ask for?

## Running it

```sh
git clone https://github.com/0xShug0/audio.cpp /path/to/audio.cpp
scripts/audio-cpp-spike/build-libaudiocpp.sh /path/to/audio.cpp

cd scripts/audio-cpp-spike/ffi-probe
AUDIOCPP_LIB_DIR=/path/to/audio.cpp/build-spike/bin cargo run --release
```

That covers questions 1 to 3 and prints JSON.

Question 4 needs a Supertonic 3 package, which is not redistributable through
this repository. Install one (`audio-cpp/audio.cpp-gguf` on Hugging Face,
package `supertonic_3_orig`) and point the probe at it:

```sh
AUDIOCPP_LIB_DIR=/path/to/audio.cpp/build-spike/bin \
AUDIOCPP_SPIKE_MODEL=/path/to/models/Supertonic-3-GGUF \
cargo run --release
```

Without `AUDIOCPP_SPIKE_MODEL` the probe says so on stderr and reports
`"textToSpeech": null` rather than failing.

| Variable | Default | Meaning |
| --- | --- | --- |
| `AUDIOCPP_LIB_DIR` | *required* | Directory holding the built `libaudiocpp` |
| `AUDIOCPP_SPIKE_MODEL` | unset | Weights directory; unset skips inference |
| `AUDIOCPP_SPIKE_FAMILY` | `supertonic` | Model family |
| `AUDIOCPP_SPIKE_VOICE` | `M1` | Built-in voice id |
| `AUDIOCPP_SPIKE_BACKEND` | `cpu` | `cpu`, `cuda`, `metal`, `vulkan`, `hip` |
| `AUDIOCPP_SPIKE_THREADS` | `4` | Inference threads |
| `AUDIOCPP_SPIKE_TEXT` | a fixed paragraph | Text to synthesize |
| `AUDIOCPP_SPIKE_OUT` | `spike-tts.wav` | Where to write the audio |
| `AUDIOCPP_SPIKE_MODELS` | `supertonic,nemotron_asr` | Families to link (build script) |
