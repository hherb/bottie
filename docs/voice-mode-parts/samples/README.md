# Audition samples

WAVs from the measured session in [`../tts-head-to-head.md`](../tts-head-to-head.md). All speak the same protocol
sentence:

> Bottie keeps local inference and storage as first-class.

They exist so the voice decision is made by listening rather than by reading numbers.

## Round 1 — engine comparison (2026-11-01, owner verdicts recorded)

| File | Engine | Voice | Rate | Owner verdict |
| --- | --- | --- | --- | --- |
| `supertonic3-short.wav` | Supertonic 3, SDK 1.3.1 | `M1` | 44.1 kHz | **superior** |
| `kokoro-fp32-short.wav` | Kokoro-82M fp32, `kokoro-onnx` 0.6.1 | `af_heart` | 24 kHz | acceptable |
| `kokoro-int8-short.wav` | Kokoro-82M int8 | `af_heart` | 24 kHz | **not acceptable** (quantisation audible) |
| `piper-lessac-short.wav` | Piper 1.8.0 | `en_US-lessac-medium` | 22.05 kHz | **superior** — but not redistributable, see below |
| `piper-ljspeech-short.wav` | Piper 1.8.0 | `en_US-ljspeech-medium` | 22.05 kHz | **not acceptable** |

`en_US-lessac-medium` is trained on the Blizzard 2013 Lessac data under an Edinburgh **research-only, non-transferable**
licence that excludes developing voice synthesis products and use by third parties. It cannot be shipped in Bottie; the
file stays here only as the quality reference the clean voices must beat.

## Round 2 — Piper voices whose weights are redistributable

Only five voices in the Piper catalogue are both trained from scratch and on redistributable data; LJ Speech was
already rejected on quality, so these three are the remaining candidates. Medium tier is ~30–40× realtime
(~0.1 s first audio); `cori-high` measured RTF 0.206 (~5×), i.e. no faster than Supertonic.

| File | Voice | Tier | Training data | Licence |
| --- | --- | --- | --- | --- |
| `piper-kristin-medium-short.wav` | `en_US-kristin-medium` | medium | LibriVox, public domain | redistributable, commercial OK |
| `piper-norman-medium-short.wav` | `en_US-norman-medium` | medium | LibriVox, public domain | redistributable, commercial OK |
| `piper-cori-high-short.wav` | `en_GB-cori-high` | high | LibriVox, public domain | redistributable, commercial OK |

**Owner verdict on round 2 (2026-11-01): none of the three is really good; `piper-cori-high` is possibly the least
bad.** That closes Piper as a shipped engine: the voice worth shipping cannot be licensed, and the licensed voices are
not worth shipping.

Excluded: `en_US-bryce-medium` (public-domain data, but weights descend from an unreleased unnamed base with no stated
provenance). The audit — including the finding that 13 of 26 English/German voices are fine-tuned from the Lessac voice
and five from the CC BY-NC-SA Ryan voice, so a clean *dataset* label does not make the *weights* clean — is quoted in
[`../tts-head-to-head.md`](../tts-head-to-head.md) §4.

## Production path samples — `sherpa-onnx` with the official fp32 weights (and int8 for comparison)

The production path is `sherpa-onnx` 2.0.1 driving the **same fp32 weights** the owner approved. It measured
**RTF 0.1077 vs the Python SDK's 0.209 (1.94× faster)** — the speed-up is the C++ implementation, not quantisation.

| File | Variant | Why it exists |
| --- | --- | --- |
| `supertonic3-sherpa-fp32-short.wav` | official fp32 weights via sherpa-onnx | the recommended production sample: same voice as `supertonic3-short.wav`, ~2× faster engine |
| `supertonic3-sherpa-int8-short.wav` | third-party int8 pack | optional A/B: int8 adds only 3–8% over fp32 and is close spectrally (LTAS corr 0.99) but not bit-transparent; also carries a licence-metadata conflict |

Both are seed-matched to each other, not to `supertonic3-short.wav`, because the sampler's default seed is random
(`seed = -1`); any comparison, cache key, or regression test must pin `extra["seed"]`.

**Owner A/B verdict, 2026-11-01: the fp32 production sample is preferred; int8 is acceptable.** fp32 is the default;
int8 remains a legitimate compact option where the 145 MB download (vs 399 MB) matters.

Long-form samples (the 224-character paragraph used by the audio.cpp evaluation) are **not** stored in the repository
because each is 1–2 MB; they are at `/tmp/voice-eval/{supertonic,kokoro,piper}/long.wav` for as long as that scratch
directory survives, and are reproducible from each engine's `report.md` command lines.

Provenance: files were copied byte-for-byte from the benchmark directories, which record exact package versions, model
SHA-256 values, engine settings, and host load at measurement time. Generated audio is evaluation evidence, not
shipped assets: Supertonic's weights (Open RAIL-M) claim no rights in the output (§6 of that licence), Kokoro's are
Apache-2.0, and Piper's output carries no restriction.
