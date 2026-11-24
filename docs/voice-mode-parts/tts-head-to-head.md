# TTS head-to-head: Supertonic 3, Kokoro-82M, and Piper measured

Measured 2026-11-01 on the Apple M3 Max host used by [`docs/audio-cpp-evaluation.md`](../audio-cpp-evaluation.md).
This is the measurement that [`docs/voice-mode-evaluation.md`](../voice-mode-evaluation.md) deferred. It turns the
"which engine" question from a licence argument into numbers, and it records the owner's licence ruling that made the
comparison possible.

Reproduction material: `/tmp/voice-eval/{supertonic,kokoro,piper}/` contains each engine's `measurements.json`,
`report.md`, harness script, raw runs and WAVs. Short samples for audition are in
[`samples/`](samples/).

## 0. Premise: the owner's licence ruling, and a correction

The owner has decided that the Supertonic weights' restrictions are acceptable and that GPL is not a hindrance. That
changes the question from "what is licence-clean" to "what is best, and what obligations does it carry".

One correction for the record: the repository declares **MIT** in `LICENSE`, `package.json` and
`src-tauri/Cargo.toml`, and there is no Apache License 3.x (the Apache versions are 1.0, 1.1 and 2.0). This document
therefore proceeds on the actual ruling — GPL and OpenRAIL components are acceptable for consideration — without
assuming a relicense happened. If Bottie's own project licence is meant to change, that is a separate decision with
its own record; it is not needed for the engine choice.

What each engine costs in obligations if it ships:

| Engine | Code | Weights / voices | Obligations if bundled |
| --- | --- | --- | --- |
| Supertonic 3 | MIT (SDK) | **Open RAIL-M** | Ship the licence; bind users to Attachment A as an enforceable provision; expressly disclaim machine-generated content — a spoken reply is generated content; no medical advice or interpretation, no fully automated decisions affecting legal rights; no impersonation. §7 reserves a remote-restriction/update right, at odds with pin-and-verify (moot: upstream reference implementation archived 2026-07). |
| Kokoro-82M | MIT (`kokoro-onnx`), MIT (ORT) | **Apache-2.0** weights and voice table | None beyond notices — *if* the phonemiser is replaced. The measured Python path dlopens GPL-3.0 `libespeak-ng` 1.52.0 in-process (see §3), which drags GPL obligations into the shipped binary unless replaced. |
| Piper | **GPL-3.0-or-later** | `piper-voices` files MIT, but per-voice weight provenance varies | Engine and its statically embedded eSpeak NG 1.52.0 are GPL: bundling ships a GPL binary and requires offering its source. The voice must be chosen for dataset licence *and* weight provenance — `en_US-lessac-medium` is research-only and cannot ship; `en_US-ljspeech-medium` is clean but was rejected on quality; three clean voices remain as candidates (§4). |

What changes in the repository if any GPL component is accepted: `DEPENDENCY-LICENCES.md` gains a policy paragraph and
the exact reviewed identities; `scripts/dependency-inventory.mjs` must record them through the existing
`REVIEWED_NOTICE_PACKAGES` mechanism (GPL currently falls through to `review-required`); `THIRD-PARTY-NOTICES.txt`,
`MODEL-NOTICE.txt` and `runtime-assets.json` gain the assets; and the release notes must stop implying a permissive-only
dependency graph. The shipped artifact's licence obligations change even though the repository's own code stays MIT.

## 1. Method

Identical text for every engine, both directions of the comparison:

- **long** — the exact paragraph used by `docs/audio-cpp-evaluation.md`, so the Supertonic RTF is directly comparable
  to that document's 4.5× CPU figure;
- **short** — `Bottie keeps local inference and storage as first-class.` — a realistic first sentence for streaming.

Host: Apple M3 Max, 16 cores, 128 GB, macOS 27.2, arm64. All engines: **CPU only**, ONNX Runtime 1.31.1, **4 intra-op
threads**, engine defaults otherwise, one voice each (Supertonic `M1`, Kokoro `af_heart`, Piper `en_US-lessac-medium`).
TTFA is time from the synthesis call to the first audio buffer available in-process; where an engine only returns whole
audio, TTFA equals synthesis time and the report says so.

**The host was not idle.** It carries 10+ unrelated python processes; load average sat between 6.8 and 28 all session.
A cross-engine mutex (`/tmp/voice-eval/measure.lock`) serialised the final runs and every run records its load stamp
and, where instrumented, process CPU-seconds and parallelism. Each engine's `report.md` carries the full spread across
passes; the headline here is the best-available window for each engine, and the numbers should be read as ±10–18%, not
three significant figures. Kokoro and Piper were measured under *higher* load than Supertonic, so their wall times are
if anything pessimistic.

## 2. Results

| | **Supertonic 3** | **Kokoro-82M fp32** | **Kokoro-82M int8** | **Piper 1.8** |
| --- | --- | --- | --- | --- |
| Implementation | `supertonic` 1.3.1 SDK | `kokoro-onnx` 0.6.1 | `kokoro-onnx` 0.6.1 | `piper-tts` 1.8.0 |
| Voice / rate | M1, 44.1 kHz | `af_heart`, 24 kHz | `af_heart`, 24 kHz | `lessac-medium`, 22.05 kHz |
| Model bytes on disk | 398,653,124 | 353,719,767 | **142,333,725** | **63,206,179** |
| Cold load | **0.45 s** | 0.80 s | 0.74 s | 1.11 s (first utterance 0.14 s) |
| Long text synthesis | 3.073 s | 3.310 s | 1.681 s | **0.335 s** |
| Long text audio | 14.70 s | 14.16 s | 14.22 s | 12.97 s |
| **Long-text RTF** | 0.209 | 0.234 | 0.118 | **0.026** |
| Realtime multiple | 4.8× | 4.3× | 8.5× | **39×** (best pass 0.021 ≈ 47×) |
| Short text synthesis | 0.902 s | 0.884 s | 0.446 s | **0.095 s** |
| Short text audio | 4.11 s | 3.30 s | 3.27 s | 3.33 s |
| **First audio, short text (measured)** | 0.902 s | 0.826 s | 0.407 s | **0.095 s** |
| Streaming API | **none** — whole buffer only | `create_stream`, but one batch for ≤510 phonemes | same | per-sentence raw output |
| Peak RSS, process high-water | **608 MiB** | 1,097 MiB | 1,028 MiB | **375 MiB** |
| RSS right after load | 502 MiB | 622 MiB | 375 MiB | — |
| Code licence | MIT | MIT | MIT | **GPL-3.0-or-later** |
| Weights licence | **Open RAIL-M** | Apache-2.0 | Apache-2.0 | per-voice (LJ Speech = public domain) |
| G2P | in-package, no eSpeak | GPL-3.0 libespeak-ng dlopen'd | same | GPL-3.0 eSpeak NG statically linked |
| **Owner audition, 2026-11-01** | **superior** | acceptable | **not acceptable** | superior (Lessac) / **not acceptable** (LJSpeech) |
| Redistributable voice | yes (Open RAIL, accepted) | yes (Apache-2.0) | yes (Apache-2.0) | **no for Lessac** (research-only); clean voices exist but none passed the audition |
| Host load during headline pass | 6.8–8.7 | 18.2–18.9 | 18.2–18.9 | 20.2–22.0 |

Cross-check: Supertonic through the official SDK measured **RTF 0.209**, against **0.222** through audio.cpp on this
same host and text. A 5.8% difference is within host noise, so the two independent paths agree — the audio.cpp TTS
figures were real.

### What the table says

1. **Supertonic and Kokoro fp32 are effectively tied on speed** (0.209 vs 0.234, with Kokoro measured under roughly
   double Supertonic's host load). Neither has a latency story over the other at 4 threads. Their real differences are
   voice, disk, and licence.
2. **Kokoro int8's speed win is not usable**: the owner's audition rejected it as audibly worse than fp32, so the
   viable Kokoro variant is the 354 MB fp32 pack at 4.3× realtime — which removes Kokoro's size and speed advantages.
3. **Piper's raw speed is real but it has no shippable voice.** The 39× realtime and 0.1 s first audio stand, but
   `en_US-lessac-medium` is trained on data under a research-only licence and the three clean voices were all judged
   not good enough at a second audition — so Piper drops out (§4).
4. **Streaming is chunk-level in all three.** Supertonic's SDK has no streaming API at all; Kokoro's `create_stream`
   yields one chunk for anything ≤510 phonemes, which is every sentence a chatbot would send; Piper synthesises one
   ONNX call per sentence. In voice mode, *we* must cut at clause/sentence boundaries and call the engine per chunk —
   the first-audio column is then the per-sentence synthesis cost, not the whole reply's.
5. **Voice-mode latency is decided by the first chunk, and Supertonic's fixed ~100 ms per call sets a floor.** With a
   0.5–0.7 s endpoint decision, Supertonic's short-opening-clause strategy lands first audible at **0.93–1.16 s**
   (1–4 words) or 1.15–1.35 s (8 words); Piper medium would land around 0.65–0.85 s; Kokoro fp32 around 1.0–1.2 s. All
   are inside the 1.5 s median target except a 20-word opening (2.0 s) — which is why the chunker must start short.

### Supertonic clause-chunk latency (measured separately)

Because the SDK only returns whole audio, the first-chunk policy *is* the latency design. Three sessions × three warm
runs, CPU, 4 threads, voice M1, defaults:

| First chunk | words | Synth | Audio | RTF | First audible @0.5 s endpoint | @0.7 s |
| --- | --- | --- | --- | --- | --- | --- |
| "Sure." | 1 | 0.372 s | 1.323 s | 0.281 | **0.93 s** | 1.13 s |
| "Sure, done." | 2 | 0.369 s | 1.393 s | 0.265 | **0.93 s** | 1.13 s |
| "Here's the quick answer." | 4 | 0.397 s | 1.811 s | 0.219 | **0.96 s** | 1.16 s |
| 8-word opening | 8 | 0.589 s | 2.995 s | 0.197 | 1.15 s | 1.35 s |
| 12-word opening | 12 | 0.772 s | 4.458 s | 0.173 | 1.33 s | 1.53 s |
| 20-word opening | 20 | 1.449 s | 8.150 s | 0.178 | 2.01 s | 2.21 s |

Cost is proportional plus a real fixed component: `synth ≈ 0.106 s + 0.165 s per audio-second`, **R² = 0.998**,
reproduced within ±20 ms across sessions. There is also an audio-duration floor — even the single word "Sure." renders
1.32 s of audio — so no chunk can synthesise below ~0.37 s. Twelve tiny calls for the same content cost **+27%**
(+1.02 s) versus two larger calls, i.e. ~102 ms per extra call. **Policy: one short opening clause (1–4 words), then
12–20-word sentence chunks; never word-by-word.** Sub-1 s first-audio is reachable; sub-0.5 s is not achievable by
chunking at all (a 0.5 s endpoint plus 60 ms prefill already spends 0.56 s before synthesis begins) — that would need
speculative synthesis overlapped with the endpoint window.

This session also measured the long text at 2.55 s (5.76×) versus the locked headline's 3.07 s (4.78×), and the
fastest runs occurred at *higher* load — so the published headline is conservative and load average is a poor predictor.
P/E-core placement appears to matter more on this host; the cost *structure* is what reproduced.

## 3. The G2P finding, which decides Kokoro's licence story

Kokoro's weights are clean, but the measured Python path is not: every run was executed with `PATH` stripped of
`espeak-ng`, synthesis still worked, and the process had loaded
`espeakng_loader/libespeak-ng.dylib` (1.52.0) via `phonemizer`'s ctypes backend. So the package does not *spawn*
eSpeak — it **dlopens a GPL-3.0 library in-process**, and `espeakng-loader` redistributes that binary plus 19 MB of
`espeak-ng-data`. Piper is worse in the same direction: its arm64 wheel statically compiles eSpeak NG into
`espeakbridge.so`, so bundling is unavoidably GPL.

The practical consequence: to ship Kokoro as an Apache-2.0 path (and drop the obligation entirely), Bottie replaces
the phonemiser. The cost looks small: **CMUdict (126,052 entries) covers 31 of the 32 unique words in the long protocol
text** — the only OOV is "Bottie" — and `misaki` is Apache-2.0 for the rule layer. English G2P is then a lexicon plus
rules for names, compounds, numbers, and dates. G2P cost on the measured path was ~0.4 ms per text, so speed is not a
constraint either way. Under the owner's ruling this is optional, not mandatory: keeping GPL eSpeak is acceptable, and
replacing it is the cheaper long-term option that also removes a 19 MB data dependency.

## 4. The owner's audition, and the Piper licence trap

The owner listened to the samples on 2026-11-01:

| Engine | Verdict |
| --- | --- |
| Supertonic 3 (`M1`) | **superior** |
| Piper `en_US-lessac-medium` | **superior** |
| Kokoro fp32 (`af_heart`) | acceptable |
| Kokoro int8 (`af_heart`) | **not acceptable** — the quantisation is audible |
| Piper `en_US-ljspeech-medium` | **not acceptable** |

Two consequences follow immediately.

**Kokoro int8 is off the table.** Its 142 MB / 8.5× realtime advantage does not survive the listening test, so the
viable Kokoro variant is the 354 MB fp32 pack at 4.3× realtime — no longer smaller or faster than Supertonic.

**The Piper voice the owner liked cannot be shipped.** `en_US-lessac-medium`'s MODEL_CARD asserts no licence of its
own; it links the CSTR Edinburgh Blizzard-2013 licence, a **Research Licence Agreement** between Voice Factory
International and Lessac Technologies. Quoting the terms: the licence is granted *"only to the person or organisation
named"*, is *"non-exclusive and non-transferable … without the right to sub-licence"*, is *"exclusively for Research
Purposes only"*, and Research Purposes *"excludes … using the Materials for any commercial purpose, including the
development, marketing, commercialisation, sale or licencing of voice synthesis … products or services"*; the user *"is
not permitted to allow the Materials to be used by any third party"*. Shipping the voice in Bottie — free or not —
falls outside that grant, and being open source does not cure it. (Whether the `.onnx` weights are themselves a
"Derivative" is a legal grey area the agreement never addresses; the training use being non-Research is the clearer
problem. This is not legal advice.)

**And most "clean-looking" Piper voices are contaminated by descent.** Auditing the MODEL_CARD of every English and
German voice (26) found: 13 fine-tuned from the Lessac voice, 5 from the *Ryan* voice (CC BY-NC-SA), 7 from scratch.
So a CC0/CC-BY *dataset* label does not make the *weights* clean — `en_US-libritts_r-medium` (CC BY 4.0 data),
`de_DE-thorsten-medium/high`, `en_US-joe-medium` (CC0 data), `en_GB-alba` and the VCTK voices are all
Lessac-descended, and **there is no clean German voice in the catalogue at all**.

Only five voices are both trained from scratch and on redistributable data: `en_GB-cori-high`, `en_GB-cori-medium`,
`en_US-kristin-medium`, `en_US-norman-medium`, and `en_US-ljspeech-medium` (already rejected on quality). Three of them
were rendered for a second audition — `piper-kristin-medium-short.wav`, `piper-norman-medium-short.wav`,
`piper-cori-high-short.wav` in [`samples/`](samples/); `en_US-bryce-medium` was excluded because its weights descend
from an unreleased unnamed base with no stated provenance. Speed differs by tier: medium ~0.024–0.034 RTF (~30–40×
realtime, ~0.1 s first audio), while `cori-high` measured **0.206 RTF (~5×)** — so the high-quality clean voice is
about as fast as Supertonic, and Piper's latency advantage only exists at the medium tier.

**Owner verdict, second audition, 2026-11-01: none of the three is really good; `en_GB-cori-high` is the least bad.**
Piper therefore drops out as a shipped engine:

- its only voice the owner rates is legally unavailable;
- the best redistributable voice is not good enough to be the default experience;
- and `cori-high` measured ~5× realtime, so even the fallback's speed advantage over Supertonic disappears — the one
  reason to accept a weaker voice was latency, and at high tier that reason is gone. The medium-tier clean voices
  (30–40× realtime) are faster, but both were judged worse than `cori-high`.

Piper can be reopened only by (a) a future clean voice that passes an audition, (b) a negotiated licence for the Lessac
data, or (c) a user-supplied voice file — which binds the user, and Bottie should neither bundle nor advertise it.

## 5. Integration cost in Bottie

The engine choice is now almost independent of the Rust integration, because one Apache-2.0 dependency drives all
three model families: the official `sherpa-onnx` crate (`2.0.1`) exposes `OfflineTts` with
`OfflineTtsSupertonicModelConfig`, `OfflineTtsKokoroModelConfig`, and `OfflineTtsVitsModelConfig` (Piper's family),
with `generate`/callback-based streaming and a Tauri demo path. So Bottie could A/B all three voices behind one trait
(`begin/push/end/interrupt`) with one in-process native surface.

The three shapes, cheapest first:

| Shape | What Bottie adds | Trust surface | Notes |
| --- | --- | --- | --- |
| **`sherpa-onnx` crate (2.0.1)** + pinned model packs | one C++ dependency, one Rust crate | large in-process C/C++ — the trust-boundary question the repo's precedent avoids | drives Supertonic, Kokoro, and Piper families; text front-ends and eSpeak data come with the model packs; needs per-platform prebuilt libraries and a packaging review; models must be re-pinned in Bottie's own asset contract |
| **`ort` + own driver** | Rust code per family: Kokoro is one graph + voices + G2P; Supertonic is four graphs + style JSON + unicode indexer; Piper is one VITS graph + eSpeak phonemes | medium — ORT is already in the graph and already packaging | most control and smallest runtime; most Bottie-owned code (text front-end is the hidden cost); ORT's missing `x86_64-apple-darwin` archive becomes a blocker |
| **Supervised sidecar worker** (existing MOSS-Nano pattern) | a Python or CLI worker per engine | smallest in-process surface; the pattern is already shipping | simplest to prototype (the harnesses in `/tmp/voice-eval` are exactly this shape), worst runtime footprint: Python + 0.4–1.1 GB RSS measured, and the MOSS-Nano precedent already costs 763 MB + a dedicated Python 3.12 |

Recommended shape for phase 3: prototype with the sidecar harness to choose the voice, then land the chosen engine as
`ort`-driven Rust if the trust review allows a second ONNX workload, or as a supervised worker if it does not. Note
that whichever engine wins, the *playback* half (bounded PCM ring buffer, cancellation within one output callback) is
the same work, already designed in the pipeline part, and is also the prerequisite for AEC.

### Production path validated: `sherpa-onnx` + Supertonic 3

The chosen engine was then exercised through the actual Rust-path dependency, on CPU with 4 threads, voice M1 = `sid 5`,
against the int8 model pack `csukuangfj2/sherpa-onnx-supertonic-3-tts-int8-2026-05-11` @ `cca5a0e6…`
(**145,317,917 B** total, 10 files, all SHA-256 verified; note the older `…2026-03-06` pack is the v2 generation — do
not use it):

| Metric | `sherpa-onnx` + **int8** pack | `sherpa-onnx` + **official fp32** weights | Official SDK 1.3.1 (fp32) |
| --- | --- | --- | --- |
| Long-text RTF | **0.1046** | **0.1077** | 0.209 |
| Short-text RTF | 0.1097 | 0.1156 | 0.188 |
| Cost model | 0.070 + 0.0995·audio_s (R² 0.996) | — | 0.106 + 0.165·audio_s |
| Cold load | 0.269 s | 0.259–0.320 s | 0.45 s |
| Peak RSS | **452–506 MiB** | 643.5 MiB | 608 MiB |
| Model bytes | 145,317,917 | 398,652,950 (the SDK's own weight set) | 398,653,124 |
| Thread scaling | saturates at 4 (1→3.47 s, 2→1.79, 4→1.61, 8→1.60) | — | 1→6.53, 2→3.41, 4→3.07 |

**The ~2× speed-up is the implementation, not the quantisation.** Running the *same fp32 weights* through the C++ path
measured RTF 0.1077 — 1.94× faster than the Python SDK's 0.209 — and int8 adds only 3–8% on top of that. So Bottie can
keep the exact voice the owner approved and still get essentially all of the speed: **`sherpa-onnx` + first-party
conversion of the official fp32 weights is the production path**, not the third-party int8 pack.

That conversion is small and mechanical: sherpa-onnx wants binary sidecars where the official repository ships JSON.
`unicode_indexer.json` → raw int32 LE comes out **byte-identical** to the pack's `.bin`, and `voice_styles/M1.json` →
`voice.bin` is a 48-byte header (6× int64 `[1,50,256,1,8,16]`) followed by 12,800 ttl and 128 dp f32 (51,760 B total).
Doing it ourselves also removes the provenance gap described below.

Because the measured cost model changes (0.070 + 0.0995·audio_s versus 0.106 + 0.165·audio_s), first-audible
improves with the same clause policy: a 1–4-word opener (1.3–1.8 s of audio) costs 0.20–0.25 s to synthesise instead of
0.37–0.40 s, so at a 0.5 s endpoint the first audible word lands at **≈0.76–0.81 s** (≈0.96–1.01 s at a 0.7 s
endpoint) [derived from the measured cost model and clause audio durations].

No eSpeak anywhere on this path (verified: zero eSpeak symbols, zero eSpeak files in the wheel). The voice map is
byte-verified: `sid` 0–4 are F1–F5, 5–9 are M1–M5, so Bottie's M1 is **sid 5**. One operational trap: the sampler's
default is `seed = -1` (random), so unseeded runs are uncorrelated; any A/B comparison, cache key, or regression test
must pin `extra["seed"]`, which then gives bit-identical repeats.

**Streaming verdict: the engine callback does not solve first-audio.** `generate(..., callback)` (Rust:
`generate_with_config(text, &config, Option<F>)` with `F: FnMut(&[f32], f32) -> bool`) exists, but reading
`offline-tts-supertonic-impl.cc` and measuring showed it fires **once per `ChunkText(text, max_len)` chunk**, default
`max_len = 300` codepoints. The 224-character paragraph is one chunk, so one callback arrives after the full
1.62 s synthesis — identical to the SDK. Forcing `max_len = 60` does produce progressive audio (first chunk 0.488 s,
then one every 0.47–0.49 s carrying 3.7–4.4 s of audio) but inflates total audio 15.43 s → 18.24 s, inserts 0.3 s of
silence between chunks into the returned buffer, and degrades RTF to 0.114. A callback return of `0` cancels only
between chunks, not work in progress. **Conclusion: Bottie keeps its own clause chunker** — a 3-clause split measured
0.489 s first audio at RTF 0.115, and application-level cancellation is something the engine cannot offer anyway.

**Rust API is real.** `OfflineTtsSupertonicModelConfig` takes 7 `Option<String>` fields that map 1:1 to the Python
options — `duration_predictor`, `text_encoder`, `vector_estimator`, `vocoder`, `tts_json`, `unicode_indexer`,
`voice_style` — with `sid` passed through `GenerationConfig`. There is no `generate` convenience wrapper; the callback
form above is the surface. The `sherpa-onnx-sys` crate downloads prebuilt native libraries from GitHub release
`v2.0.1` at build time (or honours `SHERPA_ONNX_LIB_DIR`/`ARCHIVE_DIR`), and covers macOS arm64/x64, Windows
x64/arm64, Linux x64/aarch64.

**Two pre-distribution items this raised:**

1. **Licence metadata conflict — and the resolution is to stop using the pack.** The int8 pack's own `LICENSE` file says
   *MIT* (Supertone 2025) and its HF card carries no licence tag, while the README it copied says "the accompanying
   model is released under the OpenRAIL-M License"; upstream `Supertone/supertonic-3` is `license: openrail` with the
   BigScience Open RAIL-M text (15,007 B) whose Attachment A binds "the Model **or Derivatives**", and its §1(e) defines
   Derivatives to include "transfer of patterns of the weights" — which is exactly what int8 quantisation is. A
   third-party conversion cannot relicense weights it did not own, and MIT cannot carry RAIL's use restrictions. So:
   treat every copy of these weights as Open RAIL-M, ship the RAIL text and Attachment A ourselves with the paragraph-5
   notice, derive from the official repository, and **prefer the first-party fp32 conversion over the third-party pack** —
   which we now know is cheap, keeps the approved voice, and closes the provenance gap. This is a reading of the
   licences, not legal advice.
2. **Tauri packaging.** The Rust crate's build-time native-library download needs a supply-chain decision (pin the
   archive by digest, or vendor it), and the macOS dylib must be bundled and codesigned inside the `.app` — this was
   not exercised here and is phase-3 engineering, not an engine risk.

Quality note: int8 is not bit-transparent but close. With a pinned seed both variants are deterministic and agree
*exactly* on duration (4.2947 s / 15.4309 s), while the waveforms decorrelate (SNR 1.9–2.35 dB, gain 0.57–0.60) and the
long-term average spectrum stays close (corr 0.988/0.990, mean |Δ| 1.6–2.2 dB, p95 5.2–7.7 dB). Since fp32 costs only
~5% RTF, the quantisation is not worth the licence ambiguity or the quality risk — **ship fp32 through the C++ path**.
Both samples are in [`samples/`](samples/) (`supertonic3-sherpa-fp32-short.wav` and
`supertonic3-sherpa-int8-short.wav`). **Owner A/B verdict, 2026-11-01: the fp32 sample is the preferred voice; int8 is
acceptable.** So fp32 ships as the default (the approved voice, 1.94× the SDK), and int8 becomes an optional compact
pack (145 MB instead of 399 MB, 27% less RSS, 3–8% faster) for users or platforms where download size matters — not a
quality compromise, just a second acceptable option.

## 6. Recommendation

**Ship Supertonic 3 as the default voice and keep Kokoro fp32 as the licence-clean fallback. Piper is rejected: its
best voice cannot be redistributed and its clean voices did not pass the second audition.**

Supertonic is the only engine that is both top-tier by the owner's ear *and* shippable:

- its Open RAIL obligations are accepted, and they are ordinary notices/terms work plus a machine-generated-content
  disclaimer — no dataset restriction attaches, unlike Piper's best voice;
- it needs no external phonemiser, so there is no GPL artefact and no lexicon work;
- at 4.8× realtime on 4 threads, a 1–4-word opening clause yields **0.93–1.16 s** to first audible at a 0.5–0.7 s
  endpoint — inside the 1.5 s median target (the 8-word-and-longer openings are the ones that breach it);
- its costs are explicit: **399 MB** download and **608 MiB** RSS, ~0.45 s cold load, no streaming API (the clause
  chunker is mandatory, and word-by-word chunking wastes ~100 ms per call), and an **archived upstream** — the model
  weights and both reference repositories are frozen, so Bottie owns the integration permanently. Use `sherpa-onnx`'s
  `OfflineTtsSupertonicModelConfig` or an `ort` driver, not the archived Python SDK, and pin the weights in Bottie's own
  asset contract.

Kokoro fp32 stays as the fallback because the owner finds the voice acceptable and it is the only option whose code,
weights and voice table are all permissive: if Supertonic's obligations or its 399 MB prove unacceptable on a platform,
Kokoro costs quality points but no licence argument. Its catch is the GPL `libespeak-ng` in the reference path, which
requires the Bottie-owned phonemiser (CMUdict covers 31/32 protocol words); with that, the path is Apache/MIT end to
end. int8 is rejected — do not re-test it as a size optimisation.

Piper is **rejected**. Its only voice the owner rates (`en_US-lessac-medium`) cannot be redistributed, the best clean
voice (`en_GB-cori-high`) did not impress at a second audition and, at ~5× realtime, gives up the speed advantage that
would have justified accepting a weaker voice; the two fastest clean voices were judged worse still. Lessac must not be
bundled or advertised under any circumstances; a user-supplied voice file is the only grey alternative.

Next steps: (1) validate the production path — an Apache-2.0 `sherpa-onnx` Supertonic pack measured against the SDK
numbers, including whether its callback API streams audio and what chunk cadence it gives (running); (2) prototype the
clause chunker (1–4-word opening, then 12–20-word chunks) against the real generation stream and measure end-to-end
first audible — the number that decides whether the voice-mode target holds; (3) land the engine behind the streaming
backend trait with `sherpa-onnx` or `ort`, since both can drive Supertonic (and Kokoro as fallback) behind one
interface; (4) record the Open RAIL obligations in the notices, terms and the machine-generated-content disclaimer.

## 7. Limits of this measurement

- The host was shared and never quiet; each engine's load stamp is in the results table, and the ±10–18% spread is
  documented per engine. Cross-engine gaps below ~10–15% are not real. Load average proved a poor predictor: the
  fastest Supertonic sessions ran at load 20–34 and the slowest at ~7, which points at P/E-core placement.
- Single voice per engine, English only, two texts, one host, no multi-language or long-form stability work. The
  audition was one listener per file, not blind, and the files differ in engine, voice and sample rate at once; a
  preference for 44.1 kHz output or a particular timbre is part of the jury.
- TTFA for Kokoro and Supertonic is whole-buffer on these texts because they fit one synthesis batch; the clause
  chunking table is the measured substitute, and the end-to-end pipeline number is still to be taken.
- The Lessac licence analysis is a reading of the published Research Licence Agreement, not legal advice. The
  unsettled question is whether trained weights count as a Derivative; the clearer problem is that training and
  shipping a voice product is outside the Research Purposes grant. Treat the voice as non-redistributable until a
  lawyer says otherwise.
- The Python packages are the reference implementations, not the Rust integrations. A Rust path may differ in load
  time and thread behaviour; the RTF is dominated by the ONNX graph and should carry over within a few per cent, but
  the archived Supertonic SDK means the Rust path is Bottie's to write and validate.
- Piper's 4-thread setting is a protocol artefact: ORT auto-threads (~7) measured 23% faster (RTF 0.0198), so Piper's
  real ceiling is higher than the table shows.
- No measurement of AEC, playback, or barge-in; those are pipeline concerns, not engine concerns.
