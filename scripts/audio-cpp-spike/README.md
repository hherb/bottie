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
this repository. `supertonic_3_orig` is a package id in upstream's
`model_specs/supertonic.json`, not a path — it resolves to the single file
`Supertonic-3-GGUF/supertonic-3-orig.gguf` (454,072,836 bytes) in the Hugging
Face repository `audio-cpp/audio.cpp-gguf`. Fetch it into a directory and point
the probe at the directory:

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
| `AUDIOCPP_SPIKE_OPENMP` | `OFF` on macOS, `ON` elsewhere | `ENGINE_ENABLE_OPENMP` (build script) |

The probe only covers text-to-speech. Speech-recognition figures in the
evaluation came from upstream's own `audiocpp_cli` built from the same tree
(`cmake --build <build dir> --target audiocpp_cli`), run with `--task asr
--family nemotron_asr --metrics`.

## Platform notes

`ENGINE_ENABLE_OPENMP` defaults ON upstream and is a `find_package(OpenMP
REQUIRED)`, which aborts configure on macOS because Apple clang ships no OpenMP
runtime. The build script defaults it OFF on Darwin, matching upstream's own
`scripts/build_metal.sh`. Homebrew's `libomp` also satisfies it:

```sh
AUDIOCPP_SPIKE_OPENMP=ON OpenMP_ROOT="$(brew --prefix libomp)" \
    scripts/audio-cpp-spike/build-libaudiocpp.sh /path/to/audio.cpp
```

That builds, but the result links `libomp.dylib` by absolute Homebrew path and
so is not shippable to an end user's Mac.

The script pins no compute backend, so CMake enables whatever the host offers;
on macOS that silently adds Metal and Accelerate. The build summary prints the
enabled backends so two hosts are not compared as though they built the same
thing.
