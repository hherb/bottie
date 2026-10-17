# MOSS-TTS-Nano ONNX trial

Opt-in tooling for the [Nano evaluation](../../docs/moss-nano-evaluation.md). It is outside Bottie's application,
packaging, and normal test graphs. It does not change the shipped OS speech backend or contained `run_python` runner.

## Reproduce on macOS

Use Python 3.12 in a dedicated environment. The measured host was macOS arm64; Linux and Windows were not qualified.
The asset cache needs about 763 MB plus temporary download space; the environment and audition files need extra space.

```sh
python3.12 -m venv /tmp/bottie-nano-venv
/tmp/bottie-nano-venv/bin/python -m pip install -r scripts/moss-nano-spike/requirements.txt

# Explicit network acquisition, with exact immutable revisions, sizes, and SHA-256 verification.
python3 scripts/moss-nano-spike/assets.py --directory /tmp/bottie-nano-assets --download

# Verification alone is offline and never downloads missing files.
python3 scripts/moss-nano-spike/assets.py --directory /tmp/bottie-nano-assets

# Choose a fresh output directory for each run; it contains synthetic audition WAVs and diagnostics.
/tmp/bottie-nano-venv/bin/python scripts/moss-nano-spike/probe.py \
  --directory /tmp/bottie-nano-assets \
  --output src-tauri/target/moss-nano-trial/run-1

# Small boundary tests need only Python's standard library, not the models or inference environment.
python3 -m unittest discover -s scripts/moss-nano-spike
```

`--quick` runs only the short English/German cases plus active cancellation and fresh-worker recovery. `--threads`
accepts 1–8; the measurement uses four CPU threads per ONNX session. Read `cases.json` for the exact synthetic prompts,
voice presets, and seeds. The `language` field labels evidence; Nano's core is conditioned by the text and preset audio
codes, and this trial does not invent a language-tag parameter. German uses the English Ava/Adam reference presets.

## What runs

- `assets.lock.json` pins twenty code, licence, model-card, metadata, tokenizer, and weight files. `assets.py` reuses
  only verified files and atomically promotes completed downloads. Model weights and environments stay outside Git.
- `worker.py` checks the full inventory before importing the upstream `ort_cpu_runtime.py`. It imports NumPy,
  SentencePiece, and ONNX Runtime, disables ONNX telemetry, selects only `CPUExecutionProvider`, and installs a Python
  socket audit guard. That guard is not an operating-system sandbox or a native-network containment proof.
- The convenience `onnx_tts_runtime.py` wrapper is deliberately unused because its current module imports Torch and
  TorchAudio. The pinned low-level core imports neither. No WeTextProcessing, Transformers, web server, reference
  audio upload, or user conversation is involved.
- Requests are serial newline-delimited JSON objects with exactly `text`, `voice`, and `seed`. Text is limited to
  2,000 UTF-8 bytes and split into at most 75 SentencePiece tokens per chunk. Each chunk has a hard 375-frame ceiling;
  hitting it produces `failed/frame_limit`, never `complete`. Chunk PCM is concatenated without added pauses.
- The worker emits `ready`, ordered `audio` frames (48 kHz stereo interleaved float32 LE, base64), and exactly one
  request outcome (`complete` or `failed`). It resets the streaming codec and RNG for each request. Diagnostics use
  stderr. A failed request aborts the probe, which terminates/reaps the worker.
- `client.py` enforces bounded events and deadlines even for partial lines. `probe.py` saves a PCM16 WAV only after a
  validated completion, caps output at 120 seconds, and retains the synthetic input and metrics in `measurements.json`.
  After synthesis it kills/reaps a live worker after the first PCM, discards that cancellation's audio, and checks
  successful generation from a fresh worker. These are inference-process checks, not app playback or barge-in checks.

First PCM is measured from request processing until decoded audio is ready. `firstReceivedSeconds` includes delivery
through the pipe. Neither measures sound at the speaker. RTF is generation seconds / output-audio seconds (lower is
better). Startup includes full asset hashing, imports, and session creation in a fresh process, without clearing OS
disk caches. Peak RSS is the process-lifetime high-water mark, not an isolated allocation measurement per case.

## Follow-up boundary

The next product slice needs a Rust-owned optional speech engine, bounded native playback, model acquisition and
runtime packaging, opaque voice preferences, and stop/capture synchronization. Reuse the existing speech controller's
separation from microphone capture and OS voices. Do not route this worker through the user-code `run_python` tool.
The trial has no Tauri command, settings control, automatic startup/download, application fallback, or release approval.

The upstream repository and model cards declare Apache-2.0; the exact source licence and model cards are included in
the locked cache. This trial is not a distribution review of inference wheels, bundled voices, or a shipping runtime.
