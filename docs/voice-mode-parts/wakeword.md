# Automatic wakeword recognition

Research input for the voice-mode evaluation. Not product documentation; nothing in Bottie's source, build,
packaging, or test path reads this file.

Checked: **2026-11-01**. Versions named below are the ones I actually looked at on that date, not necessarily the
latest. No wakeword code, model, crate, or dataset was installed, downloaded, built, or run for this part: every
number here is quoted from a source or derived by arithmetic from quoted numbers, and the derived ones say so. The
only Bottie-specific measurements that exist are in [`docs/audio-cpp-evaluation.md`](../audio-cpp-evaluation.md) and
none of them are wakeword measurements.

## 1. Answer first

Run a small **ONNX wakeword detector in Bottie's Rust process**, using the openWakeWord-embedding family of models
(mel spectrogram → frozen speech embedding → tiny classifier head), with a **"hey bottie" classifier trained in-house**
using LiveKit's Apache-2.0 `livekit-wakeword` training pipeline, and infer it in-process with `tract` (pure Rust,
already-available licence) rather than a new native surface. Activation is **not** always-on: the microphone is opened
only when the user explicitly arms **Voice mode**, that armed state is visibly indicated and time-limited, and the
wakeword detector shares the one already-open capture stream with the existing energy VAD.

Runner-up is **sherpa-onnx keyword spotting**: Apache-2.0, an official Rust crate with a `KeywordSpotter` API, and — the
decisive difference — a custom phrase needs **no training at all**, just a phoneme line in a keywords file. It loses on
model size (~19 MB fp32 / ~8 MB int8 plus a 3.2 MB phone table), unmeasured false-accept behaviour for English, and
320 ms or 160 ms chunk latency. Everything else in this comparison loses on licences, on a vendor licence gate, or
because the capability does not exist.

## 2. Constraints that decide it

These are the constraints from [`CONVENTIONS.md`](CONVENTIONS.md) that actually separate candidates here.

- **Local-only, no remote gate at runtime.** This alone removes Picovoice: the Porcupine SDK takes an `AccessKey` and
  "the AccessKey is validated when the engine is initialized, before offline data processing, and is also used to
  enforce usage limits. The SDK does not run without a valid key" ([Picovoice's reply, quoted by a user in the Home
  Assistant forum](https://community.home-assistant.io/t/fyi-picovoice-confirmed-free-tier-accesskeys-will-stop-working-after-june-30-2026/1012744);
  primary confirmation is that the [Porcupine quick start](https://picovoice.ai/docs/quick-start/porcupine-python/)
  lists "Picovoice Account & AccessKey" under **Requirements**). A wakeword that needs an activated key is a remote
  dependency in the hot path, and it is exactly the failure mode Bottie's design forbids.
- **MIT-compatible *weights*, not just code.** `DEPENDENCY-LICENCES.md` accepts Apache-2.0/BSD/MIT/ISC/MPL-2.0 and
  rejects non-commercial or research-only terms, and it explicitly covers "runtime assets" as well as crates. That
  rejects openWakeWord's shipped models (CC-BY-NC-SA-4.0, see §4) and forces the classifier to be Bottie-owned.
  It is the single most consequential finding in this slice.
- **Pinned, checksum-verified assets.** `runtime-assets.json` already pins a Whisper GGML file by repository revision +
  SHA-256 + size, and pins ONNX Runtime archives per target. A wakeword adds two or three small model files to that
  same contract; nothing about the mechanism is new.
- **Trust boundary.** Bottie's precedent for heavy C/C++ is a supervised sidecar (`local_image_worker.rs`, MOSS
  Nano). A wakeword does **not** need that: `ort 2.0.0-rc.13` and ONNX Runtime 1.28.0 are already in
  `src-tauri/Cargo.lock` and `runtime-assets.json` via `fastembed`, and `tract-onnx` (MIT OR Apache-2.0) is a
  pure-Rust alternative that adds no native library at all. So the wakeword surface can be strictly smaller than the
  embedding surface Bottie already ships. A C++ engine (sherpa-onnx, TFLite, Porcupine) *would* be a new in-process or
  sidecar C/C++ commitment and must be judged as such.
- **All four release targets.** macOS arm64/x64, Windows x64, Linux x64. `tract` builds everywhere Rust builds;
  `ort` is already built for all three targets in this repo. Anything OS-only (Apple Speech, Windows Speech) is
  therefore at best one third of an answer — and in fact §4 shows none of them offers keyword spotting at all.
- **Bounded always-on resources.** The budget case has to be made explicitly: always-open microphone, always-resident
  model, always-burning CPU. The published numbers in §4 (0.11 MB microWakeWord weights; ~2.4 MB openWakeWord/LiveKit
  front-end; "less than 4% of a single core on a Raspberry Pi 3" for Porcupine and "15–20 models in real time on one
  Raspberry Pi 3 core" for openWakeWord) are what make an *armed* mode defensible — but only an armed mode, and only
  with the actual numbers measured on Bottie's hosts (§6).

## 3. Candidate table

Disk figures are the model files a shipped detector must load. RAM is what the source claims for the engine, not for a
whole process. "Streaming" means it scores a continuous audio stream frame by frame.

| Candidate | What it is | Code licence | Weights licence | Disk | RAM | Platforms | Streaming / endpointing | Rust integration path | Evidence status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **openWakeWord** (dscripka) | 3-stage ONNX detector: mel → frozen Google speech embedding → per-word classifier; 80 ms frames | Apache-2.0 | **pretrained CC-BY-NC-SA-4.0** | mel 1,087,958 B + embedding 1,326,578 B + classifier 204 KB–1.7 MB + optional silero VAD 1,807,522 B | not stated | any (ONNX) | yes, 80 ms; no endpointing (it is a trigger, not a VAD) | third-party only: `oww-rs` (MIT, tract) 94,568 downloads; `aria-wake`; `phonix` is **AGPL-3.0** (unusable) | [vendor-claimed] metrics, **[measured by us: sizes from the v0.5.1 release assets]** |
| **livekit-wakeword** | Same front-end as openWakeWord, Conv-Attention classifier head, Apache-2.0 training pipeline in one YAML/CLI; ships a pretrained `hey livekit` | Apache-2.0 | repo Apache-2.0; **per-weight licence not stated** (see §4) | classifier `hey_livekit.onnx` 953,357 B; front-end (mel+embedding) compiled into the crate | not stated | macOS/Linux/Windows via crate; ORT/TFLite-compatible | yes, 80 ms frames; listeners add `debounce` | first-party crate `livekit-wakeword` 0.1.3, `ort` 2.0 + `ort-tract` (pure Rust default) | [vendor-claimed] DET/AUT/FPPH table, code and eval pipeline published |
| **Picovoice Porcupine** | Commercial on-device keyword engine, C binary + `.ppn` models | repo LICENSE Apache-2.0 but engine use is governed by **Terms of Use** | proprietary, key-gated | ~1 MB model | **~1 MB**, "<4% of a single core on a Raspberry Pi 3" | mac x64/arm64, Win x64/arm64, Linux x64 | yes, no endpointing | `pv_porcupine` crate is 0.0.0 (2024); C ABI | [vendor-claimed]; **rejected on licence + key gate** |
| **sherpa-onnx KWS** | Zipformer streaming transducer with keyword FSTs; open-vocabulary keyword list | Apache-2.0 | repo Apache-2.0; per-model licence not stated in the model docs | en model: encoder 12 MB fp32 / 4.6 MB int8 + decoder/joiner + tokens + `en.phone` 3.2 MB (≈19 MB unpacked) | not stated | mac x64/arm64, Win x64, Linux x64 (also Android/iOS) | yes; **160 ms** (chunk-8) or **320 ms** (chunk-16) latency | **official** crate `sherpa-onnx` 2.0.1 with `KeywordSpotter`/`KeywordSpotterConfig` | [vendor-claimed] latency; accuracy unmeasured, no published FPPH |
| **microWakeWord** (OHF) | TFLite-Micro streaming KWS; 40-bin feature front-end, 10 ms stride; ESP32 target | Apache-2.0 | **Apache-2.0, explicitly including weights** | 115,400 B per model | "low power" device class | any via TFLite | yes, 10 ms features / 30 ms windows | `micro-wakeword` crate 0.1.3 (MIT) for TFLite-compatible models; needs a TFLite runtime | [vendor-claimed]; README itself says training usable models "is still very difficult" |
| **Vosk** | Kaldi-based offline ASR kits; "keyword spotting" = constrained grammar on a recogniser | Apache-2.0 | Apache-2.0 (small en model) | 40 MB model | **~300 MB** runtime for a small model | all three + mobile | yes, but it is ASR, not a spotter | `vosk` crate 0.3.1 (MIT) but needs the native `libvosk` shared library | [vendor-claimed]; **rejected on footprint** |
| **rustpotter** | Pure-Rust wakeword spotter, MFCC/template matching, local custom-word training from recordings | Apache-2.0 | n/a (models built locally) | small | not stated | any (pure Rust) | yes | first-party crate, 3.0.2 (last release 2023-10-01) | [unverified]; unmaintained-looking, no accuracy evidence |
| **OS APIs** (Apple Speech, Windows Speech, Speech Dispatcher) | On-device STT / VAD modules | OS terms | n/a | n/a | n/a | per-OS only | **no keyword-spotting API on any of the three** (§4) | n/a | [measured by inspection of the API surface] |
| **Hugging Face "ONNX wakeword"** | hobby fine-tunes (`wake_word`, `hey-*`) | varies, usually undeclared | usually undeclared | varies | n/a | n/a | varies | n/a | **no mature, licence-clean option found** |

## 4. Candidate notes

### 4.1 openWakeWord — the right architecture, unusable weights

openWakeWord is the reference implementation of the approach worth taking. The pipeline, per its
[README](https://github.com/dscripka/openWakeWord/blob/main/README.md): a mel-spectrogram ONNX front-end, then a frozen
Google `speech_embedding` CNN re-implemented from the Apache-2.0 TFHub model, then a small per-word classifier on top
(the `alexa` head is a 3-layer DNN, 102,849 parameters,
[docs/models/alexa.md](https://github.com/dscripka/openWakeWord/blob/main/docs/models/alexa.md)). Audio is processed in
80 ms frames, one score per frame, and "a single core of a Raspberry Pi 3 can run 15-20 openWakeWord models
simultaneously in real-time" [vendor-claimed]. Every model shares the front-end, so a second wakeword costs only
another head.

The problem is the licence sentence at the bottom of that same README:

> All of the code in this repository is licensed under the **Apache 2.0** license. All of the included pre-trained
> models are licensed under the [Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International] license due
> to the inclusion of datasets with unknown or restrictive licensing as part of the training data.

That is a non-commercial weight licence, so under Bottie's rules the pretrained `alexa`/`hey jarvis`/`hey mycroft`
classifiers cannot be shipped or relied on, and neither can the `silero_vad.onnx` bundle. Note the ambiguity that
matters for the recommendation: the *front-end* two files are derived from Google's Apache-2.0 `speech_embedding` and a
generated mel implementation, and the README's blanket sentence may be over-broad for them — but "may be over-broad" is
not a licence review. Treat the shipped `.onnx` bytes as non-commercial until counsel or a clean-room rebuild says
otherwise (§6 risk 1). The upside is that the classifier itself is ~100 k parameters trained from synthetic speech, so
a Bottie-owned replacement is a small, well-defined job.

Custom training is genuinely cloud-free: synthetic positives from TTS plus a large negative corpus, then training only
the classifier head on frozen features. openWakeWord's own recipe is ~100,000 synthetic clips for `alexa` and
"~31,000 hours of negative data", with a Colab notebook that produces a usable model "in <1 hour" [vendor-claimed].
The extra lever that matters for a desktop app is the **custom verifier model**: a logistic-regression second stage
trained on **3 positive clips plus ~10 seconds of negative speech per speaker**, which the docs say can "significantly"
cut false activations ([docs/custom_verifier_models.md](https://github.com/dscripka/openWakeWord/blob/main/docs/custom_verifier_models.md)).
That is a 30-second onboarding step, and it is the honest answer to "always-listening will false-trigger".

Rust path: there is no official Rust binding. The practical one is [`oww-rs`](https://crates.io/crates/oww-rs)
(MIT, 94,568 downloads, last published 2026-06-12) which is "extracted source code from openWakeWord" running on
[`tract-onnx`](https://github.com/sonos/tract) — pure Rust, no ONNX Runtime. Its README bundles `alexa.onnx` "as a
sample", which is precisely the NC weight that must not ship. Two other candidates exist and one is a trap:
[`aria-wake`](https://crates.io/crates/aria-wake) (Apache-2.0, 62 downloads, too new to trust) and
[`phonix`](https://crates.io/crates/phonix) which is **AGPL-3.0-only** and therefore excluded by Bottie's policy.

Maintenance note: the reference repo's last push is 2025-12-30 and the newest tagged release is v0.6.0 from
2024-02-11 [measured: GitHub API]. Not abandoned, but not moving.

### 4.2 livekit-wakeword — the training pipeline to use, and the DET numbers to beat

[`livekit/livekit-wakeword`](https://github.com/livekit/livekit-wakeword) (Apache-2.0, created 2026-02-26, 302 stars) is
a re-implementation of the same front-end with a better classifier head: `(16, 96)` frozen embeddings →
Conv1D blocks → multi-head attention → mean pool → sigmoid, instead of openWakeWord's flatten-then-DNN. The repo also
carries the whole data pipeline (Piper TTS synthetic positives, CMU-dict adversarial negatives, MUSAN/ACAV background
noise, RIR reverb, augmentation, 3-phase training, ONNX export, DET evaluation). Its own comparison table
[vendor-claimed, with the eval code and DET plots published in-repo]:

| Metric | openWakeWord (DNN) | livekit-wakeword (DNN) | livekit-wakeword (conv-attention) |
| --- | --- | --- | --- |
| AUT (area under DET; lower better) | 0.0720 | 0.0423 | **0.0012** |
| False positives per hour | 8.50 | 3.07 | **0.08** |
| Recall | 68.6% | 85.3% | **86.1%** |
| Optimal threshold | 0.01 | 0.01 | 0.68 |

Read that honestly. It is a single-phrase ("hey livekit"), single-corpus comparison by the project that benefits from it
(15,000 positive clips, 45,084 negative clips, 25 hours of audio, per the README), and the openWakeWord column is a
model *they* trained with *their* pipeline, not the pretrained upstream artifact. But the direction of the result — a
temporal classifier head materially beats a flat DNN on false accepts — is mechanically plausible and matches why the
conv-attention head exists. It is the best public DET evidence in this category and it comes with the code to
reproduce it, which Picovoice's own benchmarks do not (see §4.3). It is still [third-party] for Bottie's purposes
until we measure our own phrase.

Two licence facts to keep separate. The **repo** is Apache-2.0 including the training code, and the pretrained
`hey_livekit.onnx` (953,357 B) ships from an Apache-2.0 repo
([hello-wakeword](https://github.com/livekit-examples/hello-wakeword)). The **front-end weights** are the same two
files the openWakeWord NC sentence covers: the LiveKit docs say so explicitly — "ONNX model: `melspectrogram.onnx` (from
openWakeWord)" and "`embedding_model.onnx` (Google speech_embedding)"
([docs/feature-extraction.md](https://github.com/livekit/livekit-wakeword/blob/main/docs/feature-extraction.md)) — and
the Rust crate "compiles the mel spectrogram and speech embedding models into the binary". So `cargo add
livekit-wakeword` ships those bytes into Bottie's binary, and the crate declares only "Apache-2.0". That is the gap
that must be closed before shipping, by either (a) a written licence opinion on the two front-end files, or (b)
pinning those two files from a clean upstream (Google's `speech_embedding` is Apache-2.0 per openWakeWord's own
README; the mel front-end is a fixed-parameter re-implementation) and recording the provenance per file in
`runtime-assets.json`. Option (b) also removes the crate's `include_bytes!` and lets Bottie own a ~200-line inference
module, which is the recommendation in §5.

Rust API status: crate `livekit-wakeword` 0.1.3 (Apache-2.0, 1,842 downloads, first published 2026-03-13, part of
`livekit/rust-sdks`). Its only public item is `WakeWordModel`; docs.rs reports **21% documented**; dependencies are
`ort ^2.0.0-rc.11` (already in Bottie's graph at `2.0.0-rc.13`) plus `ort-tract` for the pure-Rust backend, plus
`ndarray` and `resampler`. The LiveKit docs state the crate "uses a pure-Rust ONNX backend by default and falls back to
the native ONNX Runtime on aarch64 Windows"
([docs.livekit.io wakeword](https://docs.livekit.io/agents/multimodality/audio/wakeword.md)). That is a very good fit:
the default path adds no native library, and the fallback reuses the ORT Bottie already pins.

### 4.3 Picovoice Porcupine — rejected, and precisely why

Porcupine is the most accurate small engine in the category and I am not recommending it. Be precise about the reasons,
because "proprietary" alone is not one:

1. **It needs a vendor key at runtime, checked at init.** Not a download-once licence file — a key whose validation is
   also how the vendor "enforce[s] usage limits". On 2026-11-01 the Terms of Use state that access is granted as
   "limited access to Services through a **Free Trial** of Services prior to entering a commercial agreement", with
   scope and duration "as determined by Picovoice", revocable "at any time without notice"
   ([Terms of Use §6](https://picovoice.ai/docs/terms-of-use/)). A user's wakeword would stop working when their trial
   lapses or the vendor revokes it. That is a remote kill switch in a feature Bottie markets as local.
2. **The free tier is gone.** The FAQ and quick start now speak only of a "Free Trial" and require a "Picovoice
   Account & AccessKey" ([FAQ](https://picovoice.ai/docs/faq/porcupine/),
   [quick start](https://picovoice.ai/docs/quick-start/porcupine-python/)). Users report Picovoice's own written
   confirmation that free-tier AccessKeys stop working after 2026-06-30 and that "There is no non-commercial tier
   planned" — [Home Assistant forum, 2026-06-04](https://community.home-assistant.io/t/fyi-picovoice-confirmed-free-tier-accesskeys-will-stop-working-after-june-30-2026/1012744)
   [third-party: a user quoting an email; the primary Terms of Use now showing only a Free Trial is consistent with it].
   Note this already bit Bottie's ecosystem: Home Assistant's Porcupine wake-word users had to move.
3. **The licence cannot be recorded honestly in `DEPENDENCY-LICENCES.md`.** The repo ships an Apache-2.0 LICENSE file
   ([LICENSE](https://github.com/Picovoice/porcupine/blob/master/LICENSE)), but the engine binary and `.ppn` models are
   governed by the Terms of Use, not that file. Bottie would have to record a proprietary, account-bound runtime
   dependency for a feature that is advertised as local.
4. **Benchmark-disclosure clause.** The Terms require that any public comparative benchmark include "all information
   necessary for Picovoice or a third party to replicate the Benchmark". We did not benchmark Porcupine, and this
   document does not publish numbers for it beyond the vendor's own. Anyone extending this section should know the
   constraint before quoting a comparison.

What Porcupine gets right, and what to copy: ~1 MB model, "about 1 MB of memory and less than 4% of a single core on a
Raspberry Pi 3" [vendor-claimed, FAQ], 4.0.0 in December 2025 with multi-core/GPU support, and a one-click custom
keyword service. The *category benchmark* is that a competitive desktop wakeword should be single-digit MB, a few
percent of a core, and trainable — which the ONNX options are.

**Snowboy is the cautionary tale, and it is specifically about the custom-word path.** KITT.AI's notice at the top of
the [Snowboy README](https://github.com/Kitt-AI/snowboy): all KITT.AI products, including Snowboy, were shut down by
**2020-12-31**; "All official websites/APIs for our products will be taken down", and the GitHub repositories remain
open "but only community support will be available". Custom words were created by POSTing three recordings to a hosted
endpoint, `https://snowboy.kitt.ai/api/v1/train`, which returned a `.pmdl`. When the service died, every user lost the
ability to create a new wakeword, even though the detector binary was Apache-licensed and still in their tree; the repo
has been untouched since 2021-10-13 and its description now redirects to a fork. Snowboy also never supported Windows.
The lesson is not "avoid small vendors" — it is **never put the custom-word path behind someone else's server, and
pin every model file so an upstream shutdown is survivable**. That is why the recommendation here trains "hey bottie"
locally with an Apache-2.0 pipeline and pins the resulting bytes, and why Porcupine's key-gated model service is
disqualifying rather than merely inconvenient.

### 4.4 sherpa-onnx keyword spotting — the runner-up, because custom words need no training

[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) (Apache-2.0) has a real streaming keyword spotter: a Zipformer
transducer plus keyword search, driven by a `keywords.txt` file of phoneme sequences. Three pretrained models are
documented ([KWS pretrained models](https://k2-fsa.github.io/sherpa/onnx/kws/pretrained_models/index.html)): a
`zh-en` 3M model (2025-12-20), a Chinese WenetSpeech 3.3M model, and the English
`sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01` trained on GigaSpeech XL. For English the unpacked set is
≈19 MB: encoder 12 MB fp32 or 4.6 MB int8, decoder 660–1100 KB, joiner 248–628 KB, tokens 4.9 KB, plus `en.phone`
3.2 MB and `bpe.model` 240 KB for phonemisation. Streaming granularity is `chunk_size=16` → "a latency of 320 ms", or
`chunk_size=8` → "160 ms", with the docs warning lower latency means lower accuracy. Default
`keywords_threshold=0.25`, `keywords_score=1`.

The reason it is the runner-up: **custom keywords are a text edit**. "To customize your own keywords, the only thing
you need to do is replacing the `--keywords-file`", generated with `sherpa-onnx-cli text2token --tokens-type phone+ppinyin
--lexicon en.phone`. So "hey bottie" is `/HH EY1/ /B AA T IY/` in a file, with no TTS corpus, no GPU, no training run,
and no dependency on Bottie's ability to produce a licence-clean classifier. For an app whose custom-word story is the
whole point, that is a very strong property, and it is a strictly better answer than "train a model once and hope the
phrase generalises".

What it loses on: (a) **unmeasured accuracy** — there is no published FPPH/DET curve for the English KWS models that I
could find, and an open-vocabulary search over a transducer's lattice is not the same kind of classifier as a
discriminative wakeword head, so the 0.08 FPPH figure from LiveKit's `hey livekit` model does not transfer; (b) **size
and trust boundary** — ~19 MB unpacked and a large C++ library in-process, whereas the ONNX family is ~2.4 MB of
model plus a pure-Rust runtime; (c) **latency** — 160 ms chunk quantisation is worse than 80 ms frames; (d)
**phonemisation risk for a novel word** — "bottie" is out-of-vocabulary in a CMU-dict lexicon, so its phone sequence is
a judgement call (`B AA T IY` vs `B AA D IY`), and a wrong sequence silently degrades recall.

Rust status is good, which is what makes it a credible runner-up rather than a research note: the **official** crate
[`sherpa-onnx`](https://crates.io/crates/sherpa-onnx) 2.0.1 (2026-10-27) exposes `KeywordSpotter` and
`KeywordSpotterConfig` ([docs.rs](https://docs.rs/sherpa-onnx/latest/sherpa_onnx/)), and the project ships prebuilt
Tauri demo apps for macOS/Windows/Linux. The older community crate `sherpa-rs` 0.6.8 (2025-10-05) is now redundant.

### 4.5 microWakeWord — cleanest weights licence, wrong device class

[OHF-Voice/micro-wake-word](https://github.com/OHF-Voice/micro-wake-word) (Apache-2.0) targets microcontrollers:
40 spectrogram features every 10 ms from the TFLite-Micro micro_speech preprocessor (with its own noise suppression and
AGC), a MixConv streaming model, quantised TFLite. Prerolled models live in
[esphome/micro-wake-word-models](https://github.com/esphome/micro-wake-word-models) — `alexa.tflite`,
`hey_jarvis.tflite`, `okay_nabu.tflite`, **115,400 bytes each** — and, uniquely in this whole survey, that repo says
its licence covers the weights explicitly: "These terms cover the entire contents of this repository, including the
trained model weights (`.tflite`)".

So why not the recommendation? Three reasons. First, **custom words are explicitly hard**: "Training new models is
intended for advanced users. Training a model that works well is still very difficult, as it typically requires
experimentation with hyperparameters and sample generation settings", and the provided notebook "will most likely not
be usable". Bottie cannot ship a user-facing "name your wakeword" story on that. Second, **the design is
ESP32-shaped**: the front-end exists to be cheap on a 240 MHz microcontroller, including noise suppression and AGC
tuned for far-field device microphones, and it throws away spectral resolution a desktop CPU could use. Third,
**desktop tooling is thin**: the only Rust path is a hobby crate (`micro-wakeword` 0.1.3, MIT, 111 downloads) and it
needs a TFLite runtime (C/C++), i.e. a new native surface for a worse-fidelity model.

Keep it on the list for one reason: it is the only candidate whose *weights* are unambiguously Apache-2.0 with an
explicit statement, and 112 KB per model is the best size story by an order of magnitude. If the licence question in
§4.1/§4.2 cannot be resolved, microWakeWord's English models are a legally-clean but weaker fallback — with no
"hey bottie" without a hard training project.

### 4.6 Vosk — works, wrong shape

[Vosk](https://alphacephei.com/vosk/models) small English model is 40 MB and "requires about 300 MB of memory in
runtime"; small models allow dynamic vocabulary reconfiguration, which is how people approximate keyword spotting
(restrict the grammar to a handful of phrases). Apache-2.0 models, Apache-2.0 code, MIT Rust wrapper (`vosk` 0.3.1,
last updated 2024-10-27) that nevertheless needs the native `libvosk` shared library shipped or built per platform.
Rejected on footprint (300 MB resident for an armed always-listening mode is not a defensible budget next to a 2.4 MB
ONNX path), on it being a full ASR decoding continuously, and on having no published KWS false-accept evidence.

### 4.7 Other Rust options, and one licence trap

- [`rustpotter`](https://crates.io/crates/rustpotter) 3.0.2 (Apache-2.0, pure Rust, 79,670 downloads) trains a custom
  wakeword locally from a handful of recordings — the best "no cloud, no corpus" story on paper. It loses on evidence
  (no published accuracy, last release 2023-10-01) and on technique (template matching against MFCCs, which is the
  approach openWakeWord's README measures and finds "significantly below" Porcupine).
- [`aria-wake`](https://crates.io/crates/aria-wake) 0.6.0 (Apache-2.0) loads "openWakeWord and livekit-wakeword
  models" — promising, but first published 2026-10-12 with 62 downloads. Too young to pin a product on; worth
  watching.
- [`phonix`](https://crates.io/crates/phonix) 0.1.0 is **AGPL-3.0-only**. It would contaminate the graph. Excluded.

### 4.8 Operating-system options: no public always-on keyword API on any of the three

This is the part that is usually assumed rather than checked, so here is the check as of 2026-11-01.

- **macOS/iOS.** The Speech framework now has `SpeechAnalyzer`, `SpeechTranscriber`, and `SpeechDetector`. Apple's own
  one-line abstract for `SpeechDetector` is "A module that performs a voice activity detection (VAD) analysis"
  ([Apple docs](https://developer.apple.com/documentation/speech/speechdetector)); `SpeechAnalyzer` is a transcription
  pipeline ([Apple docs](https://developer.apple.com/documentation/speech/speechanalyzer)). There is **no public
  keyword-spotting or wake-word API**: nothing in the public Speech framework lets an app register "hey bottie" and get
  a low-power trigger. `SFSpeechRecognizer` with `requiresOnDeviceRecognition` can transcribe locally, but only as a
  running recognition task (power-hungry, and the audio-session and permission ergonomics are wrong for always-on).
  "Hey Siri" is OS-internal. Conclusion: macOS offers on-device **VAD** and on-device **STT**, not wakeword — Bottie
  already has its own VAD and does not need Apple's.
- **Windows.** The current first-party speech API is the Windows AI `SpeechRecognition` API, and Microsoft's own page
  labels it "currently experimental and not supported for use in production environments. Apps trying out this API
  should not be published to the Microsoft Store", offering batch and streaming **transcription**
  ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/ai/apis/speech-recognition)). The older
  `Windows.Media.SpeechRecognition` / `System.Speech` recognisers are grammar-constrained ASR, not a low-power keyword
  engine, and are not usable for hands-free activation without running full recognition continuously. Conclusion: no
  public always-on keyword detector on Windows either.
- **Linux.** Bottie's Linux TTS path is Speech Dispatcher, which is a **speech output (TTS)** interface; it has no
  recognition or keyword component. PulseAudio/PipeWire provide echo cancellation and routing, not spotting. The
  de-facto Linux options are the same in-app engines (sherpa-onnx KWS is used by real desktop projects, e.g. the
  [VocaRead and Fono entries](https://github.com/k2-fsa/sherpa-onnx#projects-using-sherpa-onnx) in sherpa-onnx's own
  project list).

Consequence: a cross-platform Bottie wakeword must be **an engine Bottie ships**, not an OS capability. That removes the
"just call the platform API" option entirely and is why §4.1–4.5 matter.

### 4.9 ONNX models on Hugging Face

I searched the Hub's model API for `wakeword` and `kws` on 2026-11-01. The results are hobby fine-tunes
(`Ahmed-ibn-Harun/wake_word`, `TheStageAI/wake-word`, `laion/bud-e_wakeword-models_livekit-wakeword`, various
`hey-*` personal projects) with download counts in the single or double digits and, in every case I inspected, no
licence metadata and no evaluation. There is **no mature, licence-clean, evaluated ONNX wakeword model** distributed
primarily on the Hub. The canonical artifacts live in GitHub releases instead: openWakeWord's
[v0.5.1 release assets](https://github.com/dscripka/openWakeWord/releases/tag/v0.5.1) (melspectrogram 1,087,958 B,
embedding 1,326,578 B, `alexa_v0.1.onnx` 854,246 B, `hey_jarvis_v0.1.onnx` 1,271,370 B, `hey_mycroft_v0.1.onnx`
857,691 B, `timer` 1,742,475 B, `weather` 1,149,158 B, `silero_vad.onnx` 1,807,522 B), LiveKit's model inside
`hello-wakeword`, and microWakeWord's TFLite files in `esphome/micro-wake-word-models`. So "pin a model from the Hub"
is not a viable plan; pin from GitHub releases with recorded SHA-256, which `runtime-assets.json` already does for
Whisper.

### 4.10 False accepts vs false rejects, and what the evidence actually covers

What exists:

- **openWakeWord's targets**: "<5% false-reject rates and <0.5/hour false-accept rates with appropriate threshold
  tuning", measured with positives mixed with noise at realistic SNR (5–10 dB) *and* reverberated with room impulse
  responses, and negatives from the ~5.5 hour Dinner Party Corpus and ~31,000 hours of training negatives
  [vendor-claimed, methodology in the README]. Its published DET plots are `alexa` and `hey mycroft`, each compared
  against Porcupine and Mycroft Precise on small samples, with the README itself warning that "sample sizes are small
  and there are issues … with the evaluation of the other libraries that suggest these results should be interpreted
  cautiously".
- **LiveKit's DET/AUT/FPPH table** in §4.2, with DET plots and eval code [vendor-claimed, reproducible].
- **Picovoice's benchmark repo** is a genuinely useful community resource for its *test data*
  ([Picovoice/wake-word-benchmark](https://github.com/Picovoice/wake-word-benchmark)) — openWakeWord's README says it
  used those positives — but Picovoice's own accuracy claims are quantified only as "impossible to quantify it …
  test the engine using an accented dataset of your choice" plus the Snowboy/PocketSphinx comparison.
- **No academic or independent head-to-head** covering openWakeWord vs livekit-wakeword vs sherpa-onnx KWS on a
  desktop-mic corpus was found. Anyone who tells you the ordering is settled is quoting a vendor.

Two honest caveats that shape Bottie's risk:

1. **Far-field vs near-field.** All of the published evaluation above is deliberately hard: far-field capture, 5–10 dB
   SNR, reverberation, hours of Dinner Party speech. Bottie's typical use is one user, 0.3–1 m from a laptop
   microphone, in a room that is quiet relative to a dinner party. Near-field should be *easier* than the published
   operating points — which is the argument for expecting better-than-published false accepts, and also the reason
   published numbers cannot be substituted for a measurement on Bottie's machines.
2. **Self-audio and media playback.** openWakeWord's own model card states that "audio playback of music/speech from
   the same device that is capturing the microphone stream may result in significantly higher false-reject rates
   unless acoustic echo cancellation (AEC) is performed"
   ([alexa.md](https://github.com/dscripka/openWakeWord/blob/main/docs/models/alexa.md)). Bottie has **no AEC** —
   it is an explicit open Milestone 7 item — and Bottie itself speaks TTS through the same machine. So while Bottie is
   speaking, either suspend wakeword scoring or accept a degraded false-accept rate. The wakeword's own trigger phrase
   can also be self-triggered if any audio Bottie plays contains "hey bottie", which a transcript or a TTS voice
   plausibly would.
3. **Microphone arrays are not available to Bottie.** `cpal` gives Bottie one channel; laptop arrays and beamforming
   live below the OS audio API in most cases, so there is no far-field beam to lean on. This pushes the design toward
   near-field, device-local activation (which is what an armed desktop mode is).

## 5. Integration sketch

### 5.1 Who owns what

Recognition belongs in Rust, in-process, in the existing microphone subsystem — **not** a sidecar. Justification: the
whole point of the sidecar precedent (`local_image_worker.rs`, `speech/nano.rs`) is to keep large untrusted C/C++
surfaces out of the credential-holding process. Here the surface is `tract` (pure Rust, MIT OR Apache-2.0) executing a
~2.4 MB ONNX graph whose bytes are SHA-256-verified, on audio Rust already owns. A sidecar would add a process, a
protocol, a handshake, and a failure mode to save nothing.

Concretely, mirroring the existing module shapes:

```
src-tauri/src/microphone/
  vad.rs        (unchanged) energy VAD, 20 ms frames, 3-frame onset / 10-frame release
  wakeword.rs   (new)     detector: 16 kHz mono f32 in, bounded pre-roll ring, score out
  capture.rs    (edit)    one cpal stream; fan each downmixed sample to VAD + wakeword
  armed.rs      (new)     armed-session lifecycle, TTL, indicator state, teardown
  latency.rs    (edit)    add the wake-to-capture interval as an observable
```

`wakeword.rs` holds: `MelFrontend` (ONNX), `SpeechEmbedding` (ONNX), `Classifier` (ONNX), a 16×96 embedding ring of
`f32` (6 KB), a pre-roll ring of 1.5 s of 16 kHz mono (96 KB as f32, 48 KB as i16), and a threshold + consecutive-frame
confirmation counter. It exposes only:

```rust
pub(super) enum VoiceActivity { Silence, Speech }          // existing, unchanged
pub(super) enum WakeState { Idle, Armed, Candidate, Detected }
```

Never a path, hash, logit vector, or model name to the WebView — same rule the README already applies to transcripts,
device identity, and detector thresholds.

### 5.2 Model pinning, in the existing `runtime-assets.json` style

Follow the `whisperTinyQ5` shape (repository + revision + `files[{path,sha256,size}]` + `licence{path,sha256,source,spdx}`)
and the `embeddingGemma` shape for multi-file assets. Three files, one classifier:

```json
"wakewordHeyBottieV1": {
  "repository": "<bottie-voice-assets repository>",
  "revision": "<40-hex commit>",
  "variant": "hey-bottie-v1-conv-attention-int8",
  "files": [
    { "path": "melspectrogram.onnx", "sha256": "<64 hex>", "size": 1087958 },
    { "path": "embedding_model.onnx", "sha256": "<64 hex>", "size": 1326578 },
    { "path": "hey_bottie.onnx",     "sha256": "<64 hex>", "size": "<measured at export>" }
  ],
  "licences": [
    { "path": "third-party/speech-embedding/LICENSE", "sha256": "<64 hex>",
      "source": "https://tfhub.dev/google/speech_embedding/1", "spdx": "Apache-2.0" },
    { "path": "third-party/torchlibrosa/LICENSE", "sha256": "<64 hex>",
      "source": "https://github.com/qiuqiangkong/torchlibrosa", "spdx": "MIT" }
  ]
}
```

Rules that follow from the existing document, not from me:

- **Acquisition only after explicit user action**, with progress and the licence shown before activation — the same
  contract the image worker uses ("show exact model/runtime IDs, license, expected disk and memory use, source revision,
  download progress, and verified file hashes before activation", `ROADMAP.md` §8.3).
- **All-files size/SHA-256 activation gate**, then re-verification at load boundary — exactly what the Whisper path does
  ("Native code verifies its exact 32,152,673-byte SHA-256 contract before loading it", README).
- **App-owned cache only**, no user-supplied model directory for wakeword; the classifier is a paired asset of the
  feature, not a user choice. That keeps the trust surface fixed and the WebView payload path-free.
- **The classifier is a Bottie build artifact**, trained offline (see §5.5), never on the user's machine at arm time.

### 5.3 Interaction with the existing energy VAD and capture loop

Today: `Record` → `capture_worker` opens the device, `build_input_stream` downmixes and calls
`VoiceActivityDetector::push` on each sample, VAD confirms onset after `SPEECH_ONSET_FRAMES = 3` × 20 ms, capture is
bounded by `MAX_CAPTURE_DURATION = 60 s` / `MAX_RETAINED_BYTES = 32 MiB`, and a 1,500 ms `TRANSCRIPTION_INTERVAL_MS`
worker consumes snapshots.

With Voice mode armed, the same `capture_worker` runs but in an `Armed` phase:

1. **One stream, two taps.** The device is opened once, at arm time. Each downmixed sample goes to both
   `VoiceActivityDetector::push` (unchanged) and the wakeword module's frame accumulator. There is no second `cpal`
   stream and no re-open on detection, so a wake event cannot create a new permission prompt, a new device-open race,
   or a second orange mic indicator.
2. **Resample once.** The wakeword needs 16 kHz mono f32; devices commonly give 44.1 or 48 kHz. Resample at the stream
   boundary (`rubato`, MIT, or a fixed polyphase filter), not per model. Do not rely on asking the OS for 16 kHz — some
   drivers refuse (§6). Record the actual device rate in the existing bounded status, never the device name.
3. **Pre-roll, so the first word is not clipped.** The pre-roll ring is the answer to "how long after the user stops
   saying 'hey bottie' until capture starts" being non-zero: at detection, the ring is handed to the capture buffer and
   the existing 3-frame onset confirmation then trims it against `SPEECH_ONSET_FRAMES`, so a user who starts talking
   immediately after the wakeword loses nothing.
4. **VAD keeps its job.** The wakeword is a *trigger*, not a VAD: the energy detector still decides speech/silence
   segments and still produces the bounded `VoiceSegment` list the composer renders. Do not let the wakeword replace
   onset detection — it is a different question with a different failure mode.
5. **Suspend scoring while Bottie speaks.** Scoring runs only in `Armed` and `Candidate`; it is paused for the whole
   `speaking` playback state (the same boundary that already drives **Interrupt & record**), because echo makes both
   false accepts and false rejects worse. Barge-in via "hey bottie" then requires an explicit product decision; the
   conservative default is: not during playback.
6. **Bounded everything.** Arm TTL (suggest 300 s, matching `MAX_CAPTURE_DURATION`'s 60 s as a *different*, shorter
   capture bound), a hard cap on pre-roll bytes, one detection per debounce window (LiveKit's listeners default
   `debounce = 2.0 s`), and the same `CaptureCommand::Stop|Discard` teardown path so an armed session can be cancelled
   from IPC with no new cancellation machinery.

### 5.4 The permission / "no mic until Record" story

Bottie's README contract is: "Bottie does not open the selected microphone or request operating-system permission until
Record is chosen." A wakeword necessarily relaxes that, so relax it explicitly and narrowly rather than silently:

- **Voice mode is an explicit action, default off, never persisted as on.** Choosing it is the analogue of choosing
  Record: it is the moment the device opens and, on macOS, the moment the mic indicator appears. Nothing about app
  startup changes.
- **Armed is a visible state.** The composer shows a distinct, non-dismissible **Voice mode armed — listening for "hey
  bottie"** state with a stop control, in the same vocabulary as **Listening for speech**. No silent-armed state exists
  in the UI, and the WebView receives only a fixed enum, never levels or scores.
- **Armed expires.** TTL as above; expiry closes the stream and returns to the idle state, so a forgotten window does
  not listen for hours. Also close on conversation change, new generation, page close, and `Discard` — the existing
  teardown triggers, extended to the armed phase.
- **One detection is one capture.** A wake event starts the ordinary capture session, which then behaves exactly as
  Record does (same 60 s / 32 MiB bounds, same correction UI, same audio-delivery and retention choices). Detection
  never auto-sends audio anywhere.
- **Onboarding verifier inside the same permission story.** The optional 3-clip verifier (§4.1) is recorded only after
  the user chooses to enrol, uses the same capture path, is stored as a few hundred floats in the app-owned cache, and
  its presence is shown as **Voice mode: tuned to your voice**. It is the one thing that makes an armed mode honest
  about false accepts.
- **Say what is stored.** Detection audio that does not trigger is dropped; the pre-roll ring is memory-only and cleared
  on every state transition; nothing about an armed session is written to the SQLite store.

### 5.5 Custom wakeword pipeline ("hey bottie"), and what it costs

This is a **build-time / release-time** activity, not something a user does, and not something Bottie does over the
network at arm time. Two options, in preference order:

1. **Train once, ship the classifier.** Use LiveKit's Apache-2.0 pipeline against a config naming the phrase:
   `target_phrases: ["hey bottie"]`, `n_samples: 10000` synthetic positives per split, `model_type: conv_attention`,
   `steps: 50000`, `target_fp_per_hour: 0.2`. Stages are `setup` (Piper checkpoint, ACAV features, RIRs, MUSAN
   backgrounds), `generate` (Piper VITS + SLERP positives, CMU-dict adversarial negatives), `augment` (noise, reverb,
   pitch), `train` (3-phase, focal loss, mixup, checkpoint averaging), `export` (ONNX, INT8), `eval` (DET/AUT/FPPH).
   System dependencies are `espeak-ng`, `ffmpeg`, `sox`, `libsndfile`, `portaudio` — all local, all free. The result is
   one `.onnx` of ~1 MB. Time and hardware are the unmeasured part: LiveKit documents running the pipeline locally and
   on SkyPilot GPU instances, and 10,000 samples + 50,000 steps on frozen features is a small training job (features
   are precomputed, so it is a CPU-scale classifier fit), but **no wall-clock number is published** → §6.
   The negative corpus is what decides the false-accept rate, and it is also the licence question: LiveKit's negatives
   are "ACAV features" plus MUSAN/background audio, and [ACAV100M](https://acav100m.github.io/) is 100 M YouTube-derived
   clips distributed **as YouTube IDs with no explicit licence grant**. Everything Bottie relies on for its *own*
   classifier should therefore come from a clean corpus (MUSAN, LibriSpeech CC-BY-4.0, Common Voice CC0, plus
   Bottie-recorded room audio), and this choice must be recorded in the asset manifest. This is the second
   licence-driven constraint, and it is cheap to satisfy because the classifier is small.
2. **No training at all: switch the engine to sherpa-onnx KWS** and put `HH EY1 B AA T IY` in a keywords file. Zero
   training, zero corpus, zero GPU, instant customisation — at ~19 MB, 160 ms chunking, an in-process C++ library, and
   no published FPPH. That is why it is the runner-up rather than the recommendation.

Whichever engine wins, the *product* rule is: the wakeword phrase is a shipped, pinned, evaluated artifact with a
recorded evaluation report, exactly like the Whisper model. A user-renamable wakeword is out of scope for v1 — it would
require either a training service (Snowboy's mistake) or a per-user model that Bottie cannot evaluate.

### 5.6 Latency: end of "hey bottie" → capture starts

Derived [derived] from the published frame sizes and Bottie's existing constants; the per-stage compute costs are
[unverified] until measured on a named host.

| Stage | Cost | Basis |
| --- | --- | --- |
| Frame quantisation: last phoneme → end of the 80 ms frame that contains it | 0–80 ms | openWakeWord/LiveKit process 80 ms frames |
| Newest embedding is at most one stride stale | 0–80 ms | embedding stride = 8 mel frames = 80 ms (LiveKit `docs/feature-extraction.md`) |
| Mel + embedding + classifier inference for that frame | ≈1–5 ms | tiny graphs (~330 k-param embedder, ~100 k-param head); [unverified] on host |
| Consecutive-frame confirmation (if 2 frames required) | 0 or 80 ms | our choice; not required by the model |
| Handoff to the capture owner | 0–40 ms | existing `CAPTURE_WORKER_POLL = 40 ms` poll |
| **Total, end of wakeword → capture running** | **≈90–250 ms, call it ~150 ms** | sum above |

Two things follow. First, ~150 ms is short enough to feel like an instant response to a spoken command, but only if
Bottie **plays a short earcon and changes state at detection** — otherwise the user has no feedback while they wait,
which is how 150 ms feels like a second. Second, transcript-level latency is not this number at all: it is
`~150 ms + user's command length + the existing 1,500 ms TRANSCRIPTION_INTERVAL_MS`, because Bottie's streaming Whisper
still re-recognises on a fixed interval. Wakeword does not make the transcript faster; it removes a mouse click.
For comparison: the Porcupine-style engines that claim "no frame quantisation" are not actually faster here, because
Bottie's own 40 ms poll and 60 ms VAD onset confirmation dominate the residual.

### 5.7 What changes in the existing modules

- `microphone.rs`: add `MicrophonePhase::Armed`, arm/disarm commands, an arm TTL, and one new latency observable
  ("native receipt of arm → first wakeword score"). Reuse `MAX_CAPTURE_DURATION`/`MAX_RETAINED_BYTES` unchanged for the
  capture that follows a detection.
- `microphone/capture.rs`: one stream, two taps, resample once, pre-roll ring, pause scoring during playback.
- `microphone/vad.rs`: no change — and that is the point. The energy detector stays the single source of truth for
  speech/silence segments in the UI.
- `microphone/latency.rs`: new measurable interval, following the existing rule that missing work is shown as
  waiting/processing/unavailable rather than zero.
- `speech.rs`: untouched. Wakeword is input, not output; it must not couple to the TTS backend selection.
- IPC: one new action (enable/disable Voice mode), one new state enum, no paths, no hashes, no scores, no model names.
- Packaging: the three ONNX files join the existing checksum-verified asset pipeline; `tract-onnx` (MIT OR
  Apache-2.0) joins `Cargo.lock`; `third-party/` gains the licensed notices; `DEPENDENCY-LICENCES.md` gains the model
  entries with the reasoning in §4.1 recorded, including the NC-sentence ambiguity.

## 6. Risks and open measurements

Ordered by what would change the recommendation.

1. **OpenWakeWord's CC-BY-NC-SA sentence covers its distributed model files, including the two front-end files that
   LiveKit's crate compiles in.** If that reading holds, `cargo add livekit-wakeword` ships non-commercial weights, and
   both it and openWakeWord require clean-up before shipping. *Resolution path:* pin the two front-end files from
   provenance we can defend (Google `speech_embedding` is Apache-2.0; the mel front-end is fixed-parameter generated
   code), record per-file provenance and licence in `runtime-assets.json`, and get a written opinion for the release
   record. *What changes if it goes the other way:* if even the front-end cannot be cleared, the recommendation falls
   back to microWakeWord's Apache-2.0 TFLite weights (weaker accuracy, no "hey bottie") or to sherpa-onnx KWS
   (Apache-2.0 code+models, unmeasured accuracy) — both worse, neither impossible.
2. **`livekit-wakeword` is a five-month-old 0.1.x crate.** Created 2026-02-26, crate first published 2026-03-13,
   1,842 downloads, 21% documented, single-vendor. A breaking 0.2 could land any month. *Mitigation:* pin the exact
   version, and keep Bottie's inference (`mel → embedding → classifier`) as a ~200-line module over `ort`/`tract` with
   three pinned ONNX files, so upgrading the crate is optional rather than load-bearing. Note the same risk applies to
   `oww-rs` (single maintainer, MIT, "extracted from openWakeWord").
3. **No FPPH/FRR number exists for "hey bottie" on Bottie's hardware, rooms, and microphones.** Every number in §4.10
   is someone else's phrase on someone else's corpus. *Measurement:* 5+ hours of real room audio (silence, music,
   speech, Bottie's own TTS playback) for FPPH; 20+ speakers × 10 attempts near-field for FRR; per platform.
   This is the measurement that decides the default threshold and whether the per-user verifier is mandatory or
   optional.
4. **CPU and RSS on Bottie's hosts are unmeasured.** Published anchors: Porcupine "~1 MB memory, <4% of a single core
   on a Raspberry Pi 3" [vendor-claimed]; openWakeWord "15–20 models in real time on one Raspberry Pi 3 core"
   [vendor-claimed]; LiveKit/OWW front-end is 2.41 MB of ONNX files [measured from release asset sizes]. *Derived
   estimate:* one wakeword (≈3.4 MB f32, four small graphs, 12.5 inferences/second at 80 ms frames) should sit well
   under 1% of one modern desktop core and add low tens of MB RSS for the ORT/tract sessions. **Measure on macOS arm64
   (M3 Max, the host `docs/audio-cpp-evaluation.md` already uses), Windows x64, and Linux x64** before promising a
   budget, and record it in the `docs/audio-cpp-evaluation.md` style with the host named.
5. **Device sample-rate refusal.** Some drivers will not open at 16 kHz. Resampling must be bounded and tested on all
   three platforms, and the resampler must not add perceived latency (a 1024-tap filter at 48 kHz is 21 ms of group
   delay — decide and document).
6. **Self-audio and media playback.** No AEC exists (Milestone 7 open item), Bottie speaks through the same machine,
   and the wakeword phrase can be present in content the user plays. Mitigation: suspend scoring during playback,
   require the per-user verifier for always-armed use, and set the default threshold from measurement 3, not from
   LiveKit's 0.68 or openWakeWord's 0.5.
7. **Latency and perceived responsiveness.** ~150 ms [derived] needs the earcon; if measurement shows >300 ms, revisit
   frame size (40 ms frames would halve quantisation at slightly more CPU — untested) or reconsider sherpa-onnx
   chunk-8's 160 ms.
8. **An OS-level KWS API could appear.** macOS 26 already added on-device VAD and a new transcription pipeline; a
   future macOS/Windows keyword API would make this whole slice obsolete for that platform. Nothing to do except keep
   the detector behind a Rust trait so a platform backend can be swapped in, exactly as `SpeechBackend` already allows
   for TTS.
9. **Training reproducibility.** The "hey bottie" model must be rebuildable from a pinned config + pinned data
   manifests by someone other than the person who trained it, or Bottie inherits Snowboy's failure mode in slow motion.
   That means committing the YAML, the dataset revisions and hashes, the exporter version, and the eval report next to
   the pinned `.onnx`.

## 7. Sources

- https://github.com/dscripka/openWakeWord — reference architecture, 80 ms frames, pretrained model licence (CC-BY-NC-SA-4.0), FPPH/FRR targets, Porcupine/Mycroft DET comparisons
- https://github.com/dscripka/openWakeWord/blob/main/docs/models/alexa.md — head architecture (102,849 params), synthetic positive and ~31,000 h negative recipe, AEC caveat
- https://github.com/dscripka/openWakeWord/blob/main/docs/custom_verifier_models.md — 3 clips + ~10 s per-speaker verifier
- https://github.com/dscripka/openWakeWord/releases/tag/v0.5.1 — exact ONNX model files and byte sizes
- https://crates.io/crates/oww-rs — MIT Rust OWW inference on `tract-onnx` (94,568 downloads)
- https://github.com/skoky/oww_rs — oww-rs README, bundled `alexa.onnx` sample (NC weight), `cpal` capture
- https://crates.io/crates/aria-wake — Apache-2.0 crate loading openWakeWord and livekit-wakeword models (too new)
- https://crates.io/crates/phonix — AGPL-3.0-only OWW port (excluded by policy)
- https://github.com/livekit/livekit-wakeword — Apache-2.0 pipeline, conv-attention head, AUT/FPPH/recall comparison, one-YAML training
- https://raw.githubusercontent.com/livekit/livekit-wakeword/main/docs/feature-extraction.md — front-end files are openWakeWord's `melspectrogram.onnx` and Google `speech_embedding`
- https://raw.githubusercontent.com/livekit/livekit-wakeword/main/docs/overview.md — ONNX+PyTorch architecture, frozen front-end
- https://raw.githubusercontent.com/livekit/livekit-wakeword/main/docs/data-generation.md — Piper VITS + SLERP positives, CMU-dict adversarial negatives, sample counts
- https://docs.livekit.io/agents/multimodality/audio/wakeword.md — client-side detection, Rust crate is inference-only, pure-Rust ONNX default, threshold/debounce
- https://livekit.com/blog/livekit-wakeword — launch post, published comparison numbers
- https://github.com/livekit-examples/hello-wakeword — pretrained `hey_livekit.onnx` (953,357 B), Apache-2.0 repo
- https://crates.io/crates/livekit-wakeword — 0.1.3, Apache-2.0, `ort` + `ort-tract` dependencies
- https://docs.rs/livekit-wakeword/latest/livekit_wakeword/ — API surface (`WakeWordModel` only), 21% documented
- https://crates.io/crates/tract-onnx — MIT OR Apache-2.0 pure-Rust ONNX
- https://crates.io/crates/ort-tract — MIT OR Apache-2.0 tract backend for `ort`
- https://picovoice.ai/docs/terms-of-use/ — Free Trial only, revocable at will, licence and benchmark-disclosure clauses
- https://picovoice.ai/docs/faq/porcupine/ — "about 1 MB of memory and less than 4% of a single core on a Raspberry Pi 3", AccessKey requirement
- https://picovoice.ai/docs/quick-start/porcupine-python/ — "Picovoice Account & AccessKey" under Requirements
- https://github.com/Picovoice/porcupine/blob/master/LICENSE — Apache-2.0 LICENSE file that governs the bindings, not the engine or models
- https://community.home-assistant.io/t/fyi-picovoice-confirmed-free-tier-accesskeys-will-stop-working-after-june-30-2026/1012744 — user quoting Picovoice: key validated at init, free tier ends 2026-06-30, no non-commercial tier planned
- https://github.com/Kitt-AI/snowboy — KITT.AI shutdown notice (products off by 2020-12-31), hosted `/api/v1/train` custom-model path, no Windows support
- https://k2-fsa.github.io/sherpa/onnx/kws/pretrained_models/index.html — KWS model inventory, sizes, 160/320 ms latency, `--keywords-file` customisation, "threshold=0.25"
- https://github.com/k2-fsa/sherpa-onnx — Apache-2.0, platform/lang matrix, Tauri support, in-process KWS users
- https://crates.io/crates/sherpa-onnx — official crate 2.0.1 (2026-10-27)
- https://docs.rs/sherpa-onnx/latest/sherpa_onnx/ — `KeywordSpotter`, `KeywordSpotterConfig`
- https://github.com/OHF-Voice/micro-wake-word — Apache-2.0, 40-feature 10 ms front-end, "training a model that works well is still very difficult"
- https://github.com/esphome/micro-wake-word-models — TFLite models 115,400 B each, licence explicitly covers weights
- https://crates.io/crates/micro-wakeword — MIT Rust detection for microWakeWord TFLite models
- https://crates.io/crates/rustpotter — Apache-2.0 pure-Rust spotter, 3.0.2 (2023-10-01)
- https://alphacephei.com/vosk/models — 40 MB small model, ~300 MB runtime, Apache-2.0
- https://crates.io/crates/vosk — MIT wrapper requiring native `libvosk`
- https://developer.apple.com/documentation/speech/speechdetector — "A module that performs a voice activity detection (VAD) analysis"
- https://developer.apple.com/documentation/speech/speechanalyzer — on-device transcription module
- https://learn.microsoft.com/en-us/windows/ai/apis/speech-recognition — experimental, not for production or Store, transcription only
- https://acav100m.github.io/ — ACAV100M is YouTube IDs, no explicit licence grant (training-data provenance)
- https://github.com/Picovoice/wake-word-benchmark — openly provided wakeword test positives used by multiple projects
