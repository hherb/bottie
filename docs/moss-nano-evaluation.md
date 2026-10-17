# MOSS-TTS-Nano evaluation

Measured: 2026-10-17. Scope: the owner's requested first Nano trial for Bottie. That trial changed no application
source, Tauri command, settings, runtime packaging, or release behavior. The optional native backend was subsequently
implemented on 2026-10-18; see [Nano integration](moss-nano-integration.md) for current setup and validation.

## Result

The pinned ONNX CPU route is technically promising enough to take to an audition and a native-backend slice. All six
synthetic English/German cases completed, including numbers and long replies. Active inference cancellation and
fresh-worker recovery passed. This is not a claim about naturalness, pronunciation, acoustic latency, or quality
under a competing chat/image workload.

The checked-in [measurement record](moss-nano-measurements.json) contains exact synthetic text, preset voices, seeds,
runtime versions, model/code revisions, waveform hashes, level measurements, and terminal events. The
[trial README](../scripts/moss-nano-spike/README.md) contains reproducible acquisition, generation, and test commands.

## Measured host and settings

- macOS 27.0.1 arm64, Python 3.12.6, four CPU threads per ONNX session;
- NumPy 2.5.4, ONNX Runtime 1.31.0, SentencePiece 0.2.2, only `CPUExecutionProvider`, no Torch import;
- fixed sampled-frame inference with upstream sampling constants and per-request seeds;
- 48 kHz stereo, 80 ms codec frames, 75 text tokens per chunk, 375 audio frames maximum per text chunk;
- built-in Ava and Adam reference codes; German is cross-language synthesis from those English presets;
- no WeTextProcessing or separate number/abbreviation normalization, no reference-audio input, and no personal audio;
- 20 pinned assets total 763,254,539 bytes, including the upstream source licence and model cards.

Source: `OpenMOSS/MOSS-TTS-Nano@8b7bcc9341b3b4ef3a3a58ba1338a7d85ff133eb`.
TTS export: `OpenMOSS-Team/MOSS-TTS-Nano-100M-ONNX@f52645cb467506d8e18e746ddd59482685b74e58`.
Codec export: `OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX@ceff0d0749bfb3fa2d61149794ec6feef0d1e1ae`.
Individual sizes, SHA-256 values, and immutable URLs are in
[`assets.lock.json`](../scripts/moss-nano-spike/assets.lock.json).

## Results

| Case | First decoded PCM | Generation | Audio duration | RTF | Text chunks |
| --- | ---: | ---: | ---: | ---: | ---: |
| English short | 144 ms | 1.64 s | 6.16 s | 0.267 | 1 |
| German short | 148 ms | 2.90 s | 10.88 s | 0.266 | 1 |
| English numbers | 116 ms | 3.23 s | 12.72 s | 0.254 | 1 |
| German numbers | 124 ms | 3.18 s | 12.80 s | 0.249 | 1 |
| English long | 159 ms | 9.87 s | 37.20 s | 0.265 | 2 |
| German long | 157 ms | 15.10 s | 40.64 s | 0.372 | 5 |

RTF is generation time / audio duration: these runs generated approximately 2.7–4.0 times faster than playback.
First PCM starts at request processing in an already-ready worker. It does not include process startup or measure
speaker output. Parent-observed first-byte delivery is retained separately in the JSON.

Fresh-process startup, including full asset hashing, imports, and session creation, was 3.61 seconds. OS disk caches
were not cleared, so this is not a cold-storage benchmark. The maximum process-lifetime RSS was 1,790,836,736 bytes
(1.67 GiB). The six audition WAVs contain nonzero finite PCM and zero samples outside the float [-1, 1] range.
Those checks do not establish intelligibility or correct numeric pronunciation.

After a real first-PCM event, cancellation terminated and reaped the worker in 42.9 ms (SIGTERM, exit -15). No audio
from that canceled request was retained. A fresh worker completed the English request with the same WAV SHA-256 as
the initial run. This establishes worker recovery, not same-process reuse after cancellation or app barge-in.

## Audition and integration boundary

On 2026-10-18 the owner approved the short English and German Ava samples: “These voices sound great!” This is
qualitative approval of those two samples. Numeric/abbreviation reading, other presets, and long-form continuity
still need listening evidence. Long German required five independently conditioned text chunks, which makes
voice/prosody continuity a particular listening target.

On the measured checkout, WAVs and diagnostic logs are retained under the ignored directory
`src-tauri/target/moss-nano-trial/2026-10-17/`. They are synthetic trial outputs, not bundled application assets.
Their hashes are retained in the checked-in JSON; regenerate them using the trial commands if that local directory
is absent.

The subsequent [native integration](moss-nano-integration.md) implements the optional backend, bounded PCM playback,
opaque preferences, stop/capture synchronization, and explicit local setup. The original trial does not itself prove
those behaviors, packaged-runtime qualification, or cross-platform readiness. The user-code `run_python` executor is
not the speech inference runtime.

The upstream [Nano code](https://github.com/OpenMOSS/MOSS-TTS-Nano) and both
[TTS](https://huggingface.co/OpenMOSS-Team/MOSS-TTS-Nano-100M-ONNX) and
[codec](https://huggingface.co/OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX) exports declare Apache-2.0. Their declarations
support this trial; distribution of a complete runtime and preset voices has not been reviewed here.
