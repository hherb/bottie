# Streaming local text-to-speech

Last checked: 2026-11-01. Versions examined are named inline; speech tooling moves fast and the
candidate half of this document should be re-read within a release cycle.

Research input for the voice-mode investigation. Nothing in Bottie reads this file, no dependency
was added, and no weight was downloaded. Claims produced by this repository carry the same markers
as the rest of the investigation: `[measured]` (a number this repository produced, on a named host),
`[vendor-claimed]`, `[third-party]`, `[unverified]`. The host that produced the `[measured]`
numbers is the Apple M3 Max, 16 cores, recorded in
[`docs/audio-cpp-evaluation.md`](../audio-cpp-evaluation.md) and
[`docs/moss-nano-measurements.json`](../moss-nano-measurements.json).

Section order follows [`CONVENTIONS.md`](CONVENTIONS.md) with one added section: §5 is the
chunking/streaming design this slice was asked to decide, and it sits before the integration sketch
because the sketch implements it. Sections 1–4 and 6–8 map directly onto the shared shape.

## 1. Answer first

**Recommendation: Kokoro-82M (Apache-2.0 weights, int8 ONNX ≈ 88 MB) behind a new native streaming
speech path, with Bottie owning phonemisation so no GPL espeak-ng artefact is ever distributed,
and with the reply chunked at clause then sentence boundaries.** It is the only shortlisted engine
that is simultaneously licence-clean, small enough to ship as one pinned download, comfortably
faster than realtime on a four-core CPU, and able to emit audio per text chunk.

**Runner-up: the MOSS-TTS-Nano worker Bottie already ships.** It needs no new licence and no new
download, and its measured first PCM arrives 0.115–0.159 s after a request with a realtime factor
of 0.249–0.372 `[measured]`; the only change is orchestration — feed it sentence-sized requests
from the existing token stream instead of the finished reply. It costs 763 MB of assets, a Python
3.12 environment, and 1.6–1.8 GB resident.

**The finding that reframes the question: "streaming TTS" is mostly an orchestration property, not
a model property.** Every candidate here is chunk-level: it synthesises a whole sentence or clause
and *then* yields audio, so it cannot emit a partial waveform mid-sentence (KittenTTS's own
documentation is unusually explicit that "streaming is chunk-level, not token-level"
`[vendor-claimed]`). Bottie's win therefore comes from (a) consuming `StreamEvent::TextDelta`
inside Rust instead of waiting for the finished reply, and (b) choosing a short first chunk. The
engine decision is then decided by licence, size, and realtime factor — not by a "streaming:
yes/no" column.

**Rejected headline: Supertonic 3**, despite being the best-sounding option this repository has
measured (`[measured]`, "the voice is superb"), because its weights are **OpenRAIL-M** — a
use-restricted licence outside Bottie's accepted list — and the upstream repository was archived in
July 2026. That resolves a gap the audio.cpp evaluation explicitly left open ("`model_specs/*.json`
carries no licence field").

## 2. Constraints that decide it

From [`CONVENTIONS.md`](CONVENTIONS.md) and the product requirements, in the order they actually
eliminate candidates:

1. **Licence.** Code and weights must be MIT-compatible: Apache-2.0, BSD, MIT, ISC, MPL-2.0.
   GPL/AGPL/LGPL/SSPL and non-commercial or research-only weights are out. This is the single
   most discriminating filter in the whole investigation, because the practical packaging of the
   two most popular engines — Piper and Kokoro — drags GPL-3.0 espeak-ng in.
2. **The chat model stays pluggable.** TTS must consume a text token stream from a hosted
   OpenAI-compatible provider, oMLX, or a local GGUF. Any speech-to-speech model is out as the
   primary path (see §4.7) because it replaces the text turn rather than consuming it.
3. **Streaming.** First audio must start after a clause or sentence, not the whole reply;
   synthesis must keep up with generation; cancellation must silence audio in a bounded time.
   Concretely, on the slowest supported host (a four-core x64 laptop) the engine must sustain
   roughly RTF ≤ 0.5 — twice realtime — so that a burst of generation cannot starve the speaker.
4. **Trust boundary and bounded resources.** Rust owns PCM, devices, engine processes, and
   cancellation; heavy native code has two established shapes, the supervised `local-image-worker`
   sidecar and the MOSS Python worker. A second in-process native runtime is a decision, not a
   default. Download size, resident memory, and audio buffers are budgeted.
5. **Platforms.** macOS arm64 and x64, Windows x64, Linux x64, all built in CI. An integration
   that only works on one host is a partial answer.
6. **Existing contracts.** `speak_text` accepts at most 32 KiB of visible text derived from the
   response's safe Markdown tokens, link destinations are never spoken
   (`src/lib/speech.ts`), the WebView never receives PCM, and voice identities are closed, opaque
   tokens with at most 128 voices. A streaming path must preserve all four.

Two constraints that look decisive but are not:

- *Streaming support.* All shortlisted engines stream at chunk level (§1).
- *Voice cloning.* Not required. Bottie needs a fixed, pleasant, consistent voice per platform,
  which is exactly what Kokoro, Piper, Supertonic, MOSS-Nano, and OS voices offer.

## 3. Candidate table

TTFB here means "time from handing a short first chunk of text to the engine until the first audio
sample is playable". Where the only evidence is a realtime factor measured on another class of
machine, TTFB is derived from it and labelled. "Streaming" distinguishes *chunk-level synthesis*
(audio per sentence, which every row has) from *sub-chunk streaming* (partial audio within one
sentence, which no chunk-level engine has).

### 3.1 Plausible on a CPU-only laptop

| Candidate | What it is | Licence (code / weights) | Weights on disk | Resident RAM | First-chunk latency | Realtime factor | Streaming | Rust integration path | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Kokoro-82M v1.0** | 82M StyleTTS2/ISTFTNet, ONNX, 54 voices / 8 languages | engine code MIT–Apache (several wrappers); **weights Apache-2.0** | 88 MB int8, 169 MB f16, 310 MB f32, + `voices-v1.0.bin` | ~350–900 MB with ONNX Runtime | 0.15–0.35 s for a ~12-word clause on a laptop `[derived]`; ~90 ms reported on M5 Max `[third-party]` | 0.36× realtime on Pi 4 / 4 threads → ~0.08–0.15 on a laptop `[third-party]`+`[derived]`; 0.08 on M5 Max `[third-party]` | chunk-level (`synthesize` yields per chunk) | `ort` (ONNX Runtime) in-process, or supervised sidecar; `tts-rs` exists but see §4.1 | good |
| **MOSS-TTS-Nano** (shipping, opt-in) | ~763 MB pinned assets, Python 3.12 + ONNX CPU, 2 presets | Apache-2.0 declared in the locked assets | 763 MB assets + Python env | 1.6–1.8 GB `[measured]` | **0.115–0.159 s** to first PCM `[measured]`; ~0.3 s to first audible after buffering `[derived]` | **0.249–0.372** `[measured]` | streams PCM per internal 75-token chunk, 48 kHz stereo, ~80 ms audio events | already integrated (`src-tauri/src/speech/nano/`); no new work | good |
| **Piper 1 (piper1-gpl)** | VITS ONNX, many voices/languages, C++ + Python | **code GPL-3.0** (embeds espeak-ng); voices MIT-ish (per-voice cards) | 20–110 MB per voice (medium ≈ 61–75 MB) | ~300 MB `[third-party]` | ~40 ms `[third-party]` | 0.357 on Pi 4 / 4 threads → ~0.05–0.1 on a laptop `[third-party]`+`[derived]`; 0.03 on M5 Max `[third-party]` | chunk-level; `--output-raw`, `--sentence-silence`, Python `synthesize` generator | **blocked** as an engine; `piper-rs` links vendored espeak-ng (§4.2) | good, licence disqualifying |
| **Piper voices + own G2P** | the ONNX voices without the GPL engine | voices MIT per dataset card (varies per voice) | as above | ~300 MB | as above | as above | chunk-level | write VITS inference + permissive G2P ourselves; phoonnx (Apache-2.0, Python) is the known permissive phonemiser | partial |
| **KittenTTS v0.8** (nano/micro/mini) | 15M/40M/80M StyleTTS2 ONNX, 8 voices, 24 kHz | **code Apache-2.0; v0.8 ONNX weights Apache-2.0** | 25 MB int8 / 56 MB nano fp32; 41 MB micro; 80 MB mini | ~200–400 MB | unknown; small enough that TTFB should be < 100 ms `[unverified]` | vendor: "runs realtime on CPU" `[vendor-claimed]`; TTFB unmeasured | chunk-level, sentence split, raw chunk joins | `ort` in-process; sherpa-onnx also ships a KittenTTS path | partial |
| **Chatterbox-Nano** | 110M English, Turbo architecture, single-step decoder | **code and weights MIT** | not yet measured here | unknown | unknown | "3× faster than realtime on 8 CPU cores" `[vendor-claimed]` | chunk-level; PyTorch reference, no ONNX path documented | Python worker only (torch); Perth watermarker runs on every output | partial |
| **Supertonic 3** | ~99M flow-matching TTS, ONNX/GGUF, 31 languages, 10 preset voices | **weights OpenRAIL-M (use-restricted)**; sample code MIT | 454,072,836 bytes GGUF `[measured]`; ONNX assets similar | **1.06 GB** `[measured]` | **clause-sized chunk at RTF 0.22 ≈ 0.5 s** `[derived from measured RTF]`; audio.cpp marks it streaming-capable | **4.5× realtime CPU / 12.7× Metal** `[measured]` | streaming, per audio.cpp's model spec | audio.cpp C ABI; or Python ONNX SDK; or the repo's own Rust example | good, licence disqualifying |
| **VibeVoice-Realtime-0.5B** | 1.02B BF16 streaming-text-input TTS, English, MIT | **code MIT; weights MIT** | ≈ 2 GB BF16 | ≈ 2.5–3 GB `[derived]` | unknown | unknown; >1B parameters on CPU `[unverified]` | designed for streaming text input | PyTorch/transformers only; no ONNX or Rust path | partial |

### 3.2 Rejected

| Candidate | Licence | Size / RAM | Why rejected |
| --- | --- | --- | --- |
| Orpheus-3B (`orpheus-3b-0.1-ft`) | HF card `apache-2.0`, built on `meta-llama/Llama-3.2-3B-Instruct`, gated | 3.78B params, ~15 GB checkpoint | licence tag is Apache but the Llama community licence rides along; far outside a CPU laptop budget |
| Chatterbox Multilingual V3 / Turbo / original | MIT | 500M / 350M / 500M, PyTorch, Perth watermark always on, needs a reference clip | no ONNX/CPU path; watermarking of every utterance is a product decision; cloning reference asset needed |
| Sesame CSM-1B | HF card `apache-2.0`, gated, Sesame acceptable-use terms | 1.55B params, ~6.2 GB | conversational/autoregressive; no CPU realtime case; 1B+ model |
| VibeVoice-1.5B | MIT | 2.70B params BF16 (5.4 GB) | podcast-oriented, long-form; not a low-latency streaming path |
| Higgs Audio 2 (Boson, 3B) | **Boson Community License based on Llama 3** (attribution, 100k-AAU threshold, use policy) | 5.77B params | licence is not MIT-compatible |
| MeloTTS | MIT (code) | 163 MB | last commit 2024-12; RTF 2.5 on Pi 4 / 4 threads (≈ 0.4× realtime) — dated and slow |
| XTTS-v2 | **Coqui Public Model License** (non-commercial) | ~1.8 GB, ~4.5 GB RAM `[third-party]` | non-commercial weights; project discontinued |
| F5-TTS | code MIT, **weights CC-BY-NC-4.0** | ~1.3 GB | non-commercial weights |
| Dia-1.6B | Apache-2.0 | 1.61B params | dialogue model, ~1.6B, no CPU realtime evidence |
| KittenTTS 2 (1.7B) | **Stellon Labs Community License** | 492–954 MB | weights licence outside the accepted list (the v0.8 ONNX line stays Apache-2.0) |
| OS voices (AVFoundation, WinRT/SAPI, Speech Dispatcher) | platform; Speech Dispatcher client crate is `LGPL-2.1 OR MIT OR Apache-2.0` per `dependency-inventory.json` | 0 | whole-utterance API in Bottie's path; cannot stream (§4.6) |
| Speech-to-speech (Moshi, Qwen-Omni, GPT-realtime) | mixed | large | replaces the pluggable text turn; out of scope by product requirement |

## 4. Candidate notes

### 4.1 Kokoro-82M — the recommendation, provided Bottie owns phonemisation

Kokoro is an 82-million-parameter StyleTTS2/ISTFTNet model with **Apache-2.0 weights**
([model card](https://huggingface.co/hexgrad/Kokoro-82M)), released v1.0 on 2025-01-27 after about
1,000 A100-hours of training on permissive audio, 54 voices across 8 languages, 24 kHz output.
Size and speed are both in the right envelope: **88 MB int8 / 169 MB f16 / 310 MB f32** plus the
`voices-v1.0.bin` style table
([tts-rs model notes](https://github.com/rishiskhare/tts-rs), also packaged by sherpa-onnx as a
311 MB multi-language bundle).

**The speed evidence is strong but borrowed.** The cleanest measurement is sherpa-onnx's RTF table
on a Raspberry Pi 4 Model B (a weak, passively cooled ARM CPU), which reports
`kokoro-en-v0_19` at **2.774 / 3.870 / 2.999 / 2.774** for 1/2/3/4 threads — i.e. 0.36× realtime
at four threads — and `kokoro-multi-lang-v1_1` at 3.191 `[third-party]`
([RTF table](https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/rtf.html)). Scaled to a
current laptop and int8, RTF lands somewhere around 0.05–0.15 `[derived]`. A third-party
comparison measured **RTF 0.08, ~90 ms to first audio, ~900 MB peak** on an M5 Max
`[third-party]` ([Contra Collective, July 2026](https://contracollective.com/blog/kokoro-vs-piper-vs-xtts-local-text-to-speech-m5-max-2026)) —
treat those round numbers as marketing-shaped, but the ordering agrees with the sherpa table.
`kokoro-onnx` claims "fast performance near real-time on macOS M1" and "~300 MB, quantized ~80 MB"
`[vendor-claimed]` ([kokoro-onnx](https://github.com/thewh1teagle/kokoro-onnx)).

**The licence trap is espeak-ng, and it is worse than it looks.** Kokoro's weights are Apache-2.0,
but every practical *pipeline* wraps them in GPL-3.0 phonemisation:

- `pip install kokoro` pulls `misaki[en]`, which pulls `phonemizer-fork` (GPL-3.0-or-later) and
  `espeakng-loader`, whose wheels bundle a prebuilt `libespeak-ng` 1.52.0. `kokoro/pipeline.py`
  unconditionally imports `misaki.espeak`, so English passes `EspeakFallback` for
  out-of-dictionary words and German/Spanish/French/Hindi/Italian/Portuguese use espeak directly.
  This was traced in detail in [hexgrad/kokoro#247](https://github.com/hexgrad/kokoro/issues/247)
  (comment 2026-10-05) `[third-party]`.
- `misaki` itself is **Apache-2.0** ([pyproject classifiers](https://github.com/hexgrad/misaki)),
  and its transformer-based English fallback does not require espeak — but the `[en]` extra still
  installs espeak, and Kokoro's pipeline still prefers it.
- The obvious Rust shortcut is worse: `tts-rs` is MIT and looks dependency-clean, but its Kokoro
  engine **shells out to the `espeak-ng` binary** (`Command::new("espeak-ng").args(["--ipa",
  "--stdin", "-q", "-v", lang])`) in
  [`phonemizer.rs`](https://github.com/rishiskhare/tts-rs/blob/main/src/engines/kokoro/phonemizer.rs).
  Shipping that installs or links GPL code.
- sherpa-onnx's Kokoro path requires `--kokoro-data-dir=espeak-ng-data`, and the model tarball
  contains that directory — the same GPL data. sherpa-onnx is Apache-2.0 and has Rust bindings, so
  it is a legitimate *runtime host* for the ONNX graph, but not a licence-clean front end.
- Other community Kokoro crates exist; only `tts-rs` and `sherpa-onnx` were audited here. Any crate
  that reproduces the reference pipeline inherits the espeak-ng dependency unless it ships its own
  G2P — which is exactly the work proposed below.

So the licence-clean Kokoro integration is **Bottie's own G2P feeding the ONNX model**: an English
lexicon plus rule-based G2P, either ported from misaki's Apache-2.0 dictionary/rules or written
against its published phoneme inventory, with no espeak fallback. That is real work and the main
technical risk in this recommendation (§6), but it is bounded to English, it is Bottie's own code,
and it is the only route that keeps the weights' Apache-2.0 grant meaningful. Multilingual Kokoro
is a later problem: misaki's ja/ko/zh modules depend on language-specific libraries
(pyopenjtalk, jieba, UniDic) whose licences need their own review.

**Voices and quality.** Kokoro is the "sounds good for 82M" option; the third-party comparison
quotes its naturalness as "close enough to human that most listeners stop noticing the seams" while
calling Piper "robotic" and XTTS near-human at 8× the cost `[third-party]`. Kokoro supports no
cloning, which Bottie does not need. `misaki` g2p is designed for it, and the model tolerates a
fixed voice style, so a single good voice (`af_heart` is the community default) is a legitimate
choice for a first release.

**Streaming capability.** Kokoro is non-autoregressive and fixed-length: it synthesises a chunk and
then vocodes it (StyleTTS2 + ISTFTNet). Every wrapper exposes it as a per-chunk generator
(`KPipeline(...)` yields `(graphemes, phonemes, audio)` per segment; `tts-rs` advertises "streaming
synthesis — audio playback begins before the full text is synthesized"). What it cannot do is emit
audio partway through a sentence — which is why chunk length, not the engine, sets first-audio
latency.

### 4.2 Piper — an excellent engine that Bottie cannot ship

Piper is fast, small, and everywhere, and the Rust story looks attractive until the licence is
followed.

- **The maintained engine is GPL-3.0.** [OHF-Voice/piper1-gpl](https://github.com/OHF-Voice/piper1-gpl)
  is described by GitHub's licence API as `GPL-3.0`, and its README opens with "a fast and local
  neural text-to-speech engine that **embeds [espeak-ng] for phonemization**"
  `[vendor-claimed]`. The Debian RFP for the package puts it the same way: "embeds espeak-ng for
  phonemization"
  ([debian-wnpp 2025-10](https://lists.debian.org/debian-wnpp/2025/10/msg00601.html)).
  [espeak-ng itself is GPL-3.0](https://github.com/espeak-ng/espeak-ng) `[third-party: GitHub
  licence API]`.
- **The MIT ancestor is dead.** [rhasspy/piper](https://github.com/rhasspy/piper) is MIT but
  **archived**; its README is a single line: "Development has moved: https://github.com/OHF-Voice/piper1-gpl".
  The relicensing is not a change of heart about the same code — it is what happens when you keep
  the espeak-ng dependency and make it explicit in the licence.
- **`piper-rs` inherits GPL even though its metadata says MIT.** The crate is `license = "MIT"` and
  depends on `espeak-rs = { path = "crates/espeak-rs" }`
  ([Cargo.toml](https://github.com/thewh1teagle/piper-rs/blob/main/Cargo.toml)). Its
  `espeak-rs-sys` build script copies a vendored `espeak-ng` source tree into `OUT_DIR`, builds it
  with CMake, and links `speechPlayer`, `espeak-ng`, and `ucd` **statically**
  ([build.rs](https://github.com/thewh1teagle/piper-rs/blob/main/crates/espeak-rs-sys/build.rs)).
  A Bottie binary built on `piper-rs` therefore contains GPL-3.0 code and must be distributed
  under GPL-3.0. The crate's MIT metadata covers only its own glue.
- **The voices are fine.** [rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices)
  carries a dataset card with `license: mit`, and each voice's own `MODEL_CARD` names its dataset
  licence, which varies. Piper models are VITS ONNX files trained with an espeak-ng-derived
  phoneme inventory, so the *weights* are not a derivative work of espeak-ng's code — they are
  trained on IPA labels — but they expect espeak-style IPA as input. Use them without espeak and
  you must supply that IPA yourself, which is exactly the phoonnx-style reimplementation, and the
  voice is still the weakest of the shortlist.
- **Streaming exists and is unremarkable.** `--output-raw` streams raw PCM from the CLI (the
  piper1-gpl CLI docs recommend `--output-raw` when you "need to stream audio"), and the Python
  API exposes `PiperVoice.synthesize()` as a generator of chunks with an optional
  `--sentence-silence` between sentences
  ([CLI](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/CLI.md),
  [Python API](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/API_PYTHON.md)). Same
  chunk-level model as everyone else.

**Verdict:** rejected as an engine on licence, not on quality or speed. "Piper voices plus our own
G2P" is technically possible and licence-clean, but it is the same G2P work Kokoro needs with a
worse voice, so it loses to the recommendation on every axis except pure CPU cost.

### 4.3 Supertonic 3 — best measured voice, wrong licence, dead upstream

This is the uncomfortable finding, and it needs to be stated plainly because
[`audio-cpp-evaluation.md`](../audio-cpp-evaluation.md) recommended this voice on the strength of
listening tests.

The measurements stand: `supertonic_3_orig`, 454,072,836 bytes, ~99M parameters, 44.1 kHz mono,
31 languages, voice M1, 224 characters → 14.7 s of audio, **CPU 4 threads 3.26 s (4.5× realtime),
Metal 1.15 s (12.7×), load 12.8 s cold / 4.8 s warm, 1.06 GB resident**, and the listen test called
the voice superb `[measured]`. audio.cpp's model registry lists it as `supported` with offline and
streaming inference.

Two facts discovered while researching this document change the conclusion:

1. **The weights are OpenRAIL-M.** The model card's front matter is `license: openrail` and the
   text says: "The accompanying model is released under the OpenRAIL-M License"
   ([Supertone/supertonic-3](https://huggingface.co/Supertone/supertonic-3)). OpenRAIL-M permits
   commercial use but attaches use restrictions and flow-through obligations; it is not one of
   Apache-2.0/BSD/MIT/ISC/MPL-2.0 and therefore falls outside the list
   [`CONVENTIONS.md`](CONVENTIONS.md) records. The GGUF conversion's card states the same:
   "weights are **OpenRAIL-M** … the license permits commercial use but carries USE RESTRICTIONS"
   ([cstr/supertonic-3-GGUF](https://huggingface.co/cstr/supertonic-3-GGUF)).
2. **Upstream is archived.** The repository now opens with "This repository is archived.
   Development and support have ended … No updates, bug fixes, security patches, or support will be
   provided", with weights relocated to the `supertone-oss-archive` namespace and the Voice Builder
   service shut down
   ([supertone-inc/supertonic](https://github.com/supertone-inc/supertonic), notice dated
   2026-07-23). An archived model is not automatically unusable — the weights are fixed and
   pinnable — but it means no future fixes, and it removes the "wait for a licence clarification"
   option.

**A lighter integration than audio.cpp does exist**, which is worth recording even though the
licence decides the outcome. Options, cheapest first: the official Python SDK (`pip install
supertonic`, ONNX Runtime, `TTS(auto_download=True)`) run as a supervised worker exactly like the
Nano worker; the repo's own per-language examples, including a Rust ONNX example that follows the
same four-network pipeline; the CrispASR GGUF conversion (one file, one digest, which suits
`runtime-assets.json`); or the audio.cpp C ABI already measured. None of that fixes OpenRAIL-M.

**Verdict:** rejected on licence and upstream health, with the quality caveat recorded. If the
owner is willing to accept an OpenRAIL-M weights licence and a frozen upstream, Supertonic 3 is
the best-sounding engine in this document and the audio.cpp measurements already cover it. That is
an owner decision, not an engineering one, and it should be made explicitly rather than drifting
into a vendor directory.

### 4.4 MOSS-TTS-Nano — the cheapest path to streaming that Bottie can ship this month

Nano is already integrated, already licence-reviewed (Apache-2.0 declarations retained with the
locked assets), already streaming *audio*, and its protocol is closer to a token-streaming design
than it first appears.

**What it already does.** `NanoService` owns one persistent Python child and a CPAL output stream
on a dedicated thread; `speak(text, voice)` sends a request; the worker splits text at
sentence/word boundaries into ≤75-token chunks, synthesises each, and emits *ordered PCM events*
(`sequence`, 48 kHz stereo float32, ~375-frame chunks) followed by a `complete` event with counts
and duration. Rust validates ordering, alignment, finiteness and sample counts before pushing into
a two-second queue, and it waits for the last predicted device deadline before reporting idle
([`docs/moss-nano-integration.md`](../moss-nano-integration.md),
`src-tauri/src/speech/nano/{protocol,service,playback}.rs`).

**Measured:** first PCM at **0.115–0.159 s** across six cases, RTF **0.249–0.372**, peak RSS
**1.60–1.79 GB**, 763 MB of pinned assets plus a dedicated Python 3.12 environment
`[measured]` (`docs/moss-nano-measurements.json`). Audio is emitted in roughly 80 ms events
(77 events for 6.16 s in the English-short case), so playback begins long before a sentence
finishes — this is genuine sub-sentence audio streaming even though the *text* unit is a request.

**What changing to token-streaming would need.** Only orchestration:

1. `SpeechController` grows a *turn* concept: `begin_speech_turn(voice)`, `push_speakable_text(&str)`,
   `end_speech_turn()`, plus the existing `stop()`. A tap on the native token stream
   (`ChannelSink::text_delta`, which already runs in Rust before the WebView sees the delta) feeds
   it.
2. A chunker inside the controller emits the first clause (short) and then sentence-sized requests.
   The worker's 2,000-byte request cap and 75-token internal split already accommodate sentences;
   no protocol change is needed.
3. The playback loop currently constructs `NativePlayback` once per utterance and iterates
   `request_chunks`; change it to iterate a channel of incoming sentences and keep the same
   playback object alive, so the ring buffer never drains mid-reply.
4. Warm-up: the first reply pays model load. Today the service spawns on first `speak`; for
   streaming, spawn it when a generation starts (or on app start if the Nano voice is selected) so
   the load overlaps the user's own turn.
5. Cancellation: reuse the existing `watch` cancel channel and the "drop the output stream and
   queue before kill/reap" ordering. Audio must stop early — flush the ring and drop the stream at
   the moment of interruption, then let the 5 s teardown finish in the background.

**Verdict:** the correct interim engine. It costs nothing in licence and little in code, and it is
the only candidate whose first-audio number this repository has measured on the target class of
hardware. Its cost is the 763 MB download, the Python dependency, and 1.6–1.8 GB resident against
an ~88 MB single-file alternative.

### 4.5 Larger and expressive models

The 2025–2026 wave splits into two groups: models that are genuinely good and models that are
genuinely fast, with little overlap on a CPU laptop. The dividing line is roughly one billion
parameters. Every 1B+ candidate here is autoregressive over a neural audio codec, which means
hundreds of sequential decode steps per second of audio; none publishes a CPU realtime factor, none
is a single-graph ONNX design, and none has a Rust path that does not go through PyTorch. On a
CPU-only laptop that class cannot keep up with a streaming reply, so it is rejected on that axis
before licence or size is even considered `[derived]`.

**Licence-blocked:** XTTS-v2 (`license: other` = Coqui Public Model License, non-commercial;
[card](https://huggingface.co/coqui/XTTS-v2)), F5-TTS (code MIT, **weights CC-BY-NC-4.0**;
[card](https://huggingface.co/SWivid/F5-TTS)), Higgs Audio 2 (Boson Community License derived from
the Meta Llama 3 agreement, with a 100,000-annual-active-user threshold and attribution duties;
[LICENSE](https://huggingface.co/bosonai/higgs-tts-2-3b-base/blob/main/LICENSE)), KittenTTS 2
(1.7B under the Stellon Labs Community License; the Apache-2.0 grant covers only the v0.8 ONNX
line — see the licence section of [KittenML/KittenTTS](https://github.com/KittenML/KittenTTS)).

**Licence-acceptable but not realtime on CPU:**

- **Orpheus-3B** carries an `apache-2.0` HF tag but is a fine-tune of
  `meta-llama/Llama-3.2-3B-Instruct`, is gated (you must accept terms before the card is even
  readable), and is 3.78B parameters (~15 GB of checkpoint files)
  ([HF API](https://huggingface.co/api/models/canopylabs/orpheus-3b-0.1-ft)). The Llama community
  licence obligations do not disappear because a derived repo re-declares Apache-2.0. Streaming and
  latency are unverified here because the card is gated.
- **Chatterbox** is MIT all the way down and has an unusually good size ladder: 500M multilingual
  V3 (23+ languages), 350M Turbo, and **110M Nano**, which Resemble claims runs "3× faster than
  realtime on 8 CPU cores" `[vendor-claimed]`
  ([README](https://github.com/resemble-ai/chatterbox)). Two product-level costs: every generated
  file is watermarked with Resemble's Perth watermarker, and the API expects a reference clip for
  voice cloning. There is no ONNX path, so a Chatterbox integration means a torch worker.
- **Sesame CSM-1B** is `apache-2.0` on the card but gated, 1.55B parameters, conversational, and
  built around audio prompts ([card](https://huggingface.co/sesame/csm-1b)).
- **VibeVoice-Realtime-0.5B** is the most interesting of the big-model rows: MIT, 1.02B BF16
  parameters (~2 GB), English, and explicitly architectured for **streaming text input**
  (`VibeVoiceStreamingForConditionalGenerationInference`)
  ([card](https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B)). No CPU realtime evidence, no
  ONNX path, and a 2 GB resident model next to Whisper — plausible on an M-series laptop with
  enough RAM, not plausible as a default on a four-core x64 machine.
  `VibeVoice-1.5B` is MIT too but is a 2.70B-parameter long-form/podcast model.
- **Dia-1.6B** is Apache-2.0 and 1.61B parameters; it is a dialogue model, not a low-latency
  single-voice reader.
- **MeloTTS** is MIT but stale (last commit 2024-12) and its measured RTF on a Pi 4 is 2.5 at four
  threads — around 0.4× realtime, too close to the line for comfort on the slowest x64 hosts.
- **KittenTTS v0.8** deserves a second look as the *ultra-light* option: 15M parameters, 25 MB int8,
  Apache-2.0, ONNX, 8 voices, 24 kHz, and sherpa-onnx already ships a path for it. Its own docs are
  the best public description of chunk-level streaming's limits: "a chunk is generated and vocoded
  in full before it is yielded, so the first chunk still costs its own generation time"; "input that
  fits in one chunk (under roughly 380 characters …) yields exactly one chunk, so `generate_stream`
  behaves like `generate`"; and "concatenating streamed chunks directly gives slightly rougher joins
  than `generate`" `[vendor-claimed]`. That is the same shape as every candidate here, stated
  honestly. It is a credible fallback if Kokoro's int8 model proves too big for the download budget,
  but its voice quality is unmeasured.

### 4.6 OS voices — why they cannot stream, and why they stay as a fallback

Bottie's `SystemSpeechBackend` drives the `tts` crate 0.26.3 (MIT). On macOS 10.14+ that crate uses
**AVFoundation**, on Windows it uses WinRT/SAPI (with optional Tolk), and on Linux it uses **Speech
Dispatcher** via the `speech-dispatcher 0.16` crate
([docs.rs](https://docs.rs/tts/0.26.3/tts/)). The crate's high-level surface is exactly
`Tts::speak(text, interrupt)`, `stop()`, `is_speaking()`, `voices()`, `set_voice()` — there is no
audio callback and no partial-output API in it, which is why `system.rs` is 69 lines.

Underneath, each platform is a whole-utterance API in Bottie's direction:

- **Linux / Speech Dispatcher.** The client library speaks to a daemon that owns the synthesis and
  the audio device; the client gets indexing and begin/end notifications, not samples. There is no
  PCM path back into the process, so streaming would mean replacing the backend, not extending it.
- **Windows / SAPI and WinRT.** `SpeechSynthesizer.SpeakAsync` with `SpeakProgress` events; audio
  can be redirected to a stream, but progress events are not audio, and the crate does not expose
  either.
- **macOS / AVFoundation.** This is the one platform where the underlying API *is* capable:
  `AVSpeechSynthesizer.write(_:toBufferCallback:)` "generates speech for the utterance and invokes
  the callback with the audio buffer" and is available on **macOS 10.15+, iOS 13+**
  ([Apple docs](https://developer.apple.com/documentation/avfaudio/avspeechsynthesizer/write(_:tobuffercallback:))),
  with an additional marker callback for word ranges. The older `NSSpeechSynthesizer` path — which
  the `tts` crate still uses for macOS 10.13 and below — only offers delegate progress callbacks
  (`speechSynthesizer(_:willSpeakWord:of:)`), i.e. notification, not audio, so it cannot stream at
  all. Even the AVFoundation route is per-utterance, is a different system voice per machine, and
  is exactly the quality Bottie is trying to replace. Recorded as available, not recommended.

**Verdict:** OS voices cannot stream through today's path and should remain the zero-download
fallback with whole-utterance behaviour. They are also the fallback that keeps
**Play response aloud** meaningful when a neural engine is absent or fails.

### 4.7 Rejected alternative: speech-to-speech

Moshi, Qwen-Omni, GPT-realtime and friends replace the text turn with an audio-native model. That
fails the pluggability requirement outright: Bottie's turn is a transcript plus a text reply, and
the chat model must remain a hosted OpenAI-compatible endpoint, oMLX, or a local GGUF. Named here
only so the decision is recorded rather than rediscovered.

## 5. Streaming design: chunking, buffering, backpressure, cancellation

This section is engine-agnostic on purpose. It is where most of the perceived quality and latency
of "streaming TTS" actually comes from, and it is the same work whichever shortlisted engine wins.

### 5.1 Where to cut text

Because chunk-level engines cannot emit audio mid-sentence, **the first chunk's text length is the
first-audio latency**. Two rules follow:

1. **Short first chunk.** Emit the first chunk at the first clause boundary that yields at least
   ~5 words, or at the first sentence boundary if it arrives first, capped at ~18 words / ~90
   characters. Thereafter use sentence-sized chunks of ~120–220 characters. This is the standard
   shape: it keeps the initial synthesis under ~2.5 s of audio, which is what the latency budget in
   §5.6 assumes.
2. **Never cut inside something that changes pronunciation.** Hard rules: no split inside a decimal
   or thousands separator (`3.26`, `1,000`), inside an abbreviation (`Dr.`, `e.g.`, `vs.`), inside a
   URL or e-mail, inside an inline code span, between a number and its unit (`30 kph`, `$42.50`),
   or immediately after a colon that introduces a list. Mirrors of this logic already exist in
   `scripts/moss-nano-spike/contract.py` (sentence-then-word fallback) and
   `tts-rs`'s numeric-connector guard.

Clause boundaries (`, ` `; ` ` — ` `: `) give a shorter first chunk but risk choppier prosody;
sentence boundaries sound more natural but add the tail of the sentence to the wait. The
recommended compromise is **clause for the first chunk only, sentences after that**, and to make it
a config knob so it can be A/B'd by ear.

### 5.2 Speech-safe Markdown must be derived before chunking, and it must be streaming-safe

Bottie already derives speakable text in the WebView from markdown-it tokens: `text`, `code_inline`,
`code_block`, `fence` and `image` children contribute content, and link destinations and markup are
dropped (`src/lib/speech.ts`, with the exact expectation "Local answer Read this guide and run
cargo test." for a heading, bold, link, and inline code). Two consequences for streaming:

1. **Do not run that derivation per delta.** A delta can split `[guide](htt` or `**bold` across
   messages, so a per-delta token pass would speak fragments and leak URL text. The reducer must
   hold back: only flush up to the last complete line/block, and keep an unterminated `[`…`](`…`,
   `**`, `_`, backtick, or fence opener in the buffer.
2. **Decide where the reducer lives.** Two options:
   - *(a) Native reducer fed by `ChannelSink::text_delta`.* The token stream already passes through
     Rust (`src-tauri/src/stream_channel.rs`, `StreamEvent::TextDelta` in
     `src-tauri/src/inference/types.rs`), so TTS can subscribe with no new IPC, no extra WebView
     work, and no chance of the user's UI thread stalling speech. Cost: a second Markdown
     derivation, narrower than the WebView's, which must not drift.
   - *(b) WebView emits speakable deltas to Rust.* Reuses the existing derivation and its tests, but
     adds a high-frequency IPC command, makes speech depend on WebView liveness, and inherits the
     partial-token problem anyway.

  **Recommendation: (a), with a deliberately *narrower* native reducer (strip fences, tables,
  images and link destinations; keep inline text) plus a shared golden-fixture list** — the exact
  inputs from `src/lib/speech.test.ts` plus split-token cases — so the two derivations can be
  checked against each other. Link destinations are a hard requirement in both directions: the
  reducer must never emit `https://…`.

### 5.3 Buffering and joins

- **Start threshold.** Do not open the output stream on the first PCM chunk. Prefill to ~120 ms of
  audio (or one 80 ms worker chunk plus the next), then start. This hides scheduling jitter and is
  well under the perceptual threshold for "started immediately".
- **Buffer depth.** Keep 1–2 chunks (~0.4–1.5 s of audio) synthesised ahead. With RTF ≤ 0.3 the
  producer runs far ahead of playback, so the queue is normally full and the steady state is
  trivial; the depth matters only at start and after an interruption.
- **Joins.** Chunk joins are the main quality risk. Two mechanisms: (i) insert 40–80 ms of silence
  at sentence joins (Piper exposes exactly this as `--sentence-silence`); (ii) for clause joins
  inside a sentence — where silence would sound like a stumble — use a 5–10 ms equal-power crossfade
  over the tail/head silence the model already produced. Prefer (i) for a first release: it is
  measurably safer and audibly acceptable, and the engines already emit trailing silence (the
  Supertonic output measured 28 % near-silent samples, which is inter-word structure, not padding).
- **Do not resample twice.** Kokoro is 24 kHz mono, Nano is 48 kHz stereo, Supertonic 44.1 kHz mono.
  Keep one conversion at the cpal edge, as `nano/playback.rs` already does, and keep the ring buffer
  in the engine's native format.

### 5.4 Backpressure

Generation is usually faster than speech, so the risk is a burst, not a steady mismatch.

- **Bound the pending text**, not the audio: reuse the existing 32 KiB utterance ceiling as the
  outer bound and set an inner queue cap (suggest 8 KiB). If the cap is reached, keep accepting
  deltas into the queue but stop dispatching new synthesis until it drains — do not drop or reorder
  text, because every spoken word must correspond to the reply.
- **Bound the audio** with the existing two-second queue and the existing five-second stall
  timeout. A stalled output device must fail the utterance, not keep capture blocked
  (`wait_for_output` already encodes this).
- **Truncation is a failure, not a feature.** If a reply exceeds 32 KiB, stop at the last complete
  sentence, report the limit, and never claim completion for audio that was not produced. The Nano
  worker already refuses oversized generation for the same reason.
- **Never speak out of order.** The OpenAI-compatible streams are ordered per run; key the chunker
  by `run_id` and refuse deltas from a superseded run.

### 5.5 Cancellation and interruption semantics

Three independent levels, each with its own bound:

| Level | Trigger | Mechanism | Bound |
| --- | --- | --- | --- |
| Local audio | **Stop**, **Interrupt & record**, conversation/branch switch, new generation, page close | flush the ring buffer and drop/flush the cpal stream | one output callback, ~10–20 ms |
| Engine | same | `watch` cancel channel to the worker, drop the output stream and queue **before** kill/reap | Nano already: kill+reap confirmed within 5 s; audio is silent long before |
| Generation | **Interrupt & record** | existing provider cancellation plus capture admission serialisation | existing contract |

Semantics to write down, because they are product decisions as much as engineering ones:

- An interruption **discards the unspoken remainder**. There is no resume-in-place: the next reply
  starts a new speech turn. (Resuming mid-sentence would require re-synthesising the tail anyway.)
- **Stop playback before starting capture**, and fail closed if the stop fails — this is exactly
  what `stop_before_microphone_capture()` does today, including the rule that "a failed
  local-playback stop keeps capture fail-closed" (README).
- **Never silently switch engines on failure.** The existing controller returns a fixed error state
  rather than substituting a system voice mid-utterance, and the streaming path must do the same.
- Report the *spoken* prefix on interruption (sentence index) as path-free status, so the UI can
  show what was heard without retaining text in the WebView.

### 5.6 Latency budget: first LLM token → first audible sample

Stages: **A** first token to the first chunk boundary, **B** speech-safe Markdown reduce, **C** G2P,
**D** first-chunk synthesis, **E** ring-buffer prefill plus device latency. Numbers marked
`[derived]` are computed from the measured realtime factors in §4; the token-throughput ranges
reflect hosted providers (fast) versus local GGUF (slower).

```
Kokoro int8, clause-first chunk, 4-core x64 laptop — typical
t=0    first LLM token
|####                            |   400 ms   A  tokens until first clause boundary (12-20 words)
|    #                           |    +5 ms   B  speakable-Markdown reduce + safety holdback
|     #                          |    +8 ms   C  G2P, English lexicon, ~12 words
|      ########                  |  +300 ms   D  synth ~2.8 s of audio at RTF ~0.11
|              ##                |   +60 ms   E  120 ms ring prefill + device
t≈0.77 s  first audible sample

Kokoro int8, sentence-first chunk, slowest supported host — worst
|#######                          |   800 ms   A  20-30 word sentence needs more tokens
|     #                           |    +5 ms   B
|      #                          |   +10 ms   C
|           ###############        | +1000 ms   D  RTF 0.35, ~2.9 s of audio
|                        ###       |  +100 ms   E
t≈1.9 s   first audible sample

MOSS-TTS-Nano, clause-first chunk, M3 Max — measured engine, first reply excluded
|####                            |   400 ms   A
|    #                           |   +10 ms   B (native reducer)
|     ##                         |   +20 ms   C (worker tokeniser)
|        ####                    |  +150 ms   D  measured first PCM 0.115-0.159 s
|            ##                  |   +60 ms   E
t≈0.64 s  first audible sample

Piper (if it were licence-clean), same text
|####                            |   400 ms   A
|    #                           |    +5 ms   B
|     #                          |    +5 ms   C
|       ##                       |   +80 ms   D  RTF ~0.05
|          ##                    |   +60 ms   E
t≈0.55 s  first audible sample
```

| Stage | Best | Typical | Worst | Who owns it |
| --- | --- | --- | --- | --- |
| A token → chunk boundary | 150 ms | 400 ms | 900 ms | chunker policy + provider throughput |
| B Markdown reduce | 2 ms | 5 ms | 15 ms | native reducer |
| C G2P (per first chunk) | 3 ms | 8 ms | 50 ms (Python/transformer G2P) | phonemiser choice |
| D first-chunk synthesis | 80 ms (Piper-class) | 250–300 ms (Kokoro int8) | 1000 ms (Kokoro on a weak host; 700 ms for MOSS at RTF 0.25) | engine + chunk length |
| E playback start | 40 ms | 60 ms | 100 ms | ring prefill + device |
| **Total** | **~0.3 s** | **~0.7 s** | **~2 s** | |

Two budget facts that matter more than the table:

- **Model load is not in this budget and must not be.** Supertonic's cold load was 12.8 s
  `[measured]`; MOSS spawns a Python child with a 60 s startup deadline. Solutions: warm the engine
  when a generation starts (before the first token arrives), keep it resident between replies, and
  show the existing "waiting" state rather than counting a load as speech latency.
- **Steady state runs far ahead.** At RTF 0.11, ten seconds of speech costs ~1.1 s of CPU; the
  producer finishes a 30 s reply roughly 27 s before the speaker does. The only places latency is
  perceptible are the first chunk and any join that drains the buffer.

## 6. Integration sketch for the recommended engine

### 6.1 What owns what

**Recommended shape:** a new native streaming speech module inside the Rust process —
`src-tauri/src/speech/streaming/` — using the `ort` crate (ONNX Runtime bindings, MIT/Apache-2.0
licensed crate; ONNX Runtime itself MIT) with the Kokoro int8 model, plus a small Rust G2P, and
**no Python**. Kokoro is a single small ONNX graph plus a style table; that is a much smaller
in-process native surface than a C++ ggml framework, and Bottie already links `whisper-rs`'s own
`ggml` copy, so a second runtime is not unprecedented — but it is still a trust decision, and the
fallback shape is documented in the same module: a supervised sidecar that speaks a bounded
JSON-lines protocol on a private pipe, exactly the Nano worker's pattern, with the same
`begin/push/end` interface above it. **If the trust review rejects a second native runtime,
nothing above the trait changes.**

```
ChannelSink::text_delta (native, already in Rust)
   └─ SpeechTap (bounded, non-blocking, per run_id)
        └─ SpeakableReducer   (streaming-safe Markdown; never emits a link destination)
             └─ Chunker       (short first clause, then sentences; never splits tokens)
                  └─ SynthesisActor (its own thread)
                       ├─ G2P: text → Kokoro phoneme ids (English lexicon + rules)
                       ├─ ort session: ids + voice style → f32 PCM @ 24 kHz mono
                       └─ bounded PCM channel (<= 2 s, mirroring nano/buffer.rs)
                            └─ cpal output stream + SPSC ring buffer (400 ms cap,
                               120 ms prefill, engine-rate PCM, device convert at edge)
```

`SpeechController` remains the only owner of policy: voice selection, preference keys, the
`voice_interaction` mutex, capture admission, and stop ordering. The actor owns PCM and the device.
Nothing new crosses IPC.

### 6.2 Trait and controller changes

```rust
/// Chunk-level streaming backend: text in, audio out, cancellable at any point.
trait StreamingSpeechBackend {
    fn begin(&mut self, voice: &str) -> Result<(), SpeechErrorCode>;
    fn push_speakable(&mut self, text: &str) -> Result<(), SpeechErrorCode>;
    fn end(&mut self) -> Result<(), SpeechErrorCode>;
    fn interrupt(&mut self) -> Result<(), SpeechErrorCode>;
    fn progress(&mut self) -> StreamProgress; // Idle | Buffering | Speaking | Draining | Failed
}
```

- `SystemSpeechBackend` implements it by buffering to the next sentence and calling the existing
  whole-utterance `speak` — so the OS-voice fallback keeps working with the same call sites, just
  without streaming.
- `NanoBackend` implements it by feeding `NanoService` one request per sentence and keeping one
  `NativePlayback` alive (runner-up path, §4.4).
- Kokoro implements it natively.
- New Tauri commands (all path-free, bounded): `begin_speech_turn`, `push_speech_delta` (only if
  option (b) in §5.2 is chosen), `end_speech_turn`; `stop_speech` gains early flush. Existing
  `speak_text` stays for **Play response aloud**.
- `SpeechStatus` gains a `streaming` capability flag per voice and a `spokenThrough` sentence index
  for the interrupted case. Keep `idle | speaking | error` as the phase set so the UI contract does
  not churn.

### 6.3 Voice picker and settings

- Extend the closed identity space, not the shape: `bottie-kokoro-v1:<voice>` alongside
  `bottie-moss-nano-v1:<voice>` and the opaque system tokens. `NativeSpeechVoice` already carries
  `id`, `name`, `language`; add a capability enum (`streaming: bool`) so Settings can say
  "streams while replying" versus "whole reply only". Keep the 128-voice cap, the deterministic
  ordering, and the reserved slots for neural presets (`combined.rs` already reserves for Nano).
- Preference: reuse the existing stable opaque key and the "unavailable after restart falls back
  and remembers the fallback" rule. No new persistence format.
- Settings: one new opt-in toggle, **Speak replies automatically**, next to the existing voice
  selector. Off by default (local-only, bounded CPU). When on and the selected voice streams, the
  tap is active; when the selected voice is an OS voice, the toggle is disabled with the reason
  "this voice can only read the finished reply".

### 6.4 `runtime-assets.json` pinning

Follow the `whisperTinyQ5` entry's shape — repository, revision, per-file SHA-256 and size, plus a
licence record with the notice file's own digest:

```json
"kokoroV1Int8": {
  "repository": "hexgrad/Kokoro-82M",
  "revision": "<40-hex commit on the model repo at cut time>",
  "variant": "v1.0-int8",
  "files": [
    { "path": "onnx/model_int8.onnx", "sha256": "<64 hex>", "size": 0 },
    { "path": "voices-v1.0.bin", "sha256": "<64 hex>", "size": 0 },
    { "path": "LICENSE", "sha256": "<64 hex>", "size": 0 }
  ],
  "licence": {
    "spdx": "Apache-2.0",
    "source": "https://huggingface.co/hexgrad/Kokoro-82M",
    "notice": "MODEL-NOTICE.txt"
  }
}
```

Two pinning details that are easy to get wrong: (i) the ONNX conversion commonly used
(`taylorchu/kokoro-onnx` release assets, referenced by both `tts-rs` and sherpa-onnx) is a **GitHub
release artefact, not an HF revision**, so pin it by exact URL + SHA-256 + size and record the
conversion provenance separately from the weight provenance; (ii) `voices-v1.0.bin` is required for
every precision variant, so it is its own digest and its own failure mode. Both files must be
verified before load, in the app-owned cache, exactly as the Whisper contract is.

### 6.5 What the user experiences

1. User sends a message; the reply starts streaming as today.
2. With auto-speak on and a streaming voice selected, Rust begins a speech turn on the first
   delta, synthesises the first clause, and audio starts in the budget of §5.6.
3. The composer shows **Interrupt & record** while speaking (already implemented), which stops
   audio within one output callback, cancels the provider run, and then admits capture.
4. **Play response aloud** remains for OS voices and for re-reading a finished reply.
5. Generated speech is session-only, never written to disk, never sent to a provider, and the
   four existing monotonic timing intervals gain a fifth: **first delta to first audible sample** —
   the measurement the README currently says Bottie cannot make.

## 7. Risks and open measurements

| # | Risk or unknown | Why it matters | What would settle it |
| --- | --- | --- | --- |
| 1 | **English G2P quality without espeak-ng** — out-of-vocabulary words, numbers, homographs, proper nouns, German/French names inside English text | this is the price of licence cleanliness, and it is the single biggest quality risk of the recommendation | build the lexicon G2P, then compare against `misaki` + espeak on 200–500 sentences of Bottie's own reply style; count pronunciation errors by ear and by ASR round-trip |
| 2 | **Realtime factor on the actual weakest hosts** | every RTF in this document is Pi 4, M3 Max, or M5 Max; a four-core x64 laptop is unmeasured | measure Kokoro int8 and MOSS-Nano on macOS arm64, macOS x64, Windows x64, Linux x64 with the real Rust stack |
| 3 | **Chunk-join quality** | one candidate's own docs admit streamed joins are rougher than batch output; clause-level first chunks may sound stuttering | listening test: 20 replies, sentence-first vs clause-first vs 40 ms inserted silence, blind |
| 4 | **In-process ONNX Runtime trust surface** | second native runtime in the process that holds credentials and the SQLite store | trust review; if rejected, the same trait runs over a supervised sidecar (§6.1) |
| 5 | **Memory budget with Whisper resident** | Kokoro ~350–900 MB plus the existing Whisper tiny (~32 MB model, small RSS) is fine; adding a 1 GB engine is not | measure peak RSS of the app with the streaming engine loaded and a recording in progress |
| 6 | **First-reply model load** | Nano pays a Python spawn with a 60 s deadline; Supertonic's cold load was 12.8 s `[measured]` | warm-on-generation-start, resident idle worker, and measure the first reply honestly in the existing timing surface |
| 7 | **Kokoro v1.0 is a Jan-2025 model** | the 2026 field (KittenTTS 2, Chatterbox-Nano, VibeVoice-Realtime) may overtake it on quality per byte | re-run this comparison at implementation time; the streaming architecture outlives the engine |
| 8 | **OpenRAIL-M owner decision on Supertonic 3** | if the owner accepts it, the best measured voice becomes available at 454 MB, with a frozen upstream | explicit written decision; do not let a vendor directory imply approval |
| 9 | **Multi-language reach** | Kokoro's permissive path is English-only until misaki's ja/ko/zh dependencies are licence-reviewed (UniDic and friends) | licence review per language module; ship English first if it is not settled |
| 10 | **Truncation and 32 KiB ceiling** | a long reply cannot be spoken as one utterance today | decide: last-complete-sentence stop with a visible limit, or raise the ceiling with a measured memory/queue case |

Claims I could **not** verify, and did not use as if I had:

- Orpheus's streaming latency and realtime factor — the HF model card is gated (`[unverified]`).
- Chatterbox-Nano's "3× faster than realtime on 8 CPU cores" — vendor claim, CPU model unspecified,
  and no ONNX path exists (`[unverified]`).
- VibeVoice-Realtime-0.5B's realtime factor on any CPU (`[unverified]`).
- KittenTTS v0.8's first-chunk latency and voice quality (`[unverified]`; licence and size are
  verified).
- The third-party M5 Max RTF/TTFB figures are internally consistent but suspiciously round; they
  are used only as a cross-check, never as the basis of a decision.
- MOSS-Nano's realtime factor on non-Apple hosts — the only measured host is the M3 Max.

## 8. Sources

Project and model sources, one per line. Local paths refer to this repository.

- [`docs/audio-cpp-evaluation.md`](../audio-cpp-evaluation.md) — Supertonic measurements, GPL/eSpeak absence, whisper coexistence.
- [`docs/moss-nano-integration.md`](../moss-nano-integration.md) and [`docs/moss-nano-measurements.json`](../moss-nano-measurements.json) — Nano worker protocol, bounds, first-PCM and RTF measurements.
- [`runtime-assets.json`](../../runtime-assets.json) — pinning shape reused in §6.4.
- `src-tauri/src/speech/{system,nano,combined,voices}.rs`, `src-tauri/src/speech/nano/{protocol,service,playback,buffer}.rs` — current controller and playback contract.
- `src-tauri/src/stream_channel.rs`, `src-tauri/src/inference/types.rs` — `StreamEvent::TextDelta`, the native token tap.
- `src/lib/speech.ts`, `src/lib/speech.test.ts` — speakable-Markdown derivation and its expectations.
- `dependency-inventory.json` — recorded licence for `speech-dispatcher` 0.16.0 (`LGPL-2.1 OR MIT OR Apache-2.0`).
- <https://github.com/OHF-Voice/piper1-gpl> — GPL-3.0 engine, "embeds espeak-ng for phonemization".
- <https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/CLI.md> — `--output-raw`, `--sentence-silence`.
- <https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/API_PYTHON.md> — `PiperVoice.synthesize` streaming generator.
- <https://github.com/rhasspy/piper> — MIT, archived, development moved.
- <https://github.com/espeak-ng/espeak-ng> — GPL-3.0.
- <https://github.com/thewh1teagle/piper-rs> and <https://github.com/thewh1teagle/piper-rs/blob/main/crates/espeak-rs-sys/build.rs> — MIT crate metadata over a vendored, statically linked espeak-ng.
- <https://huggingface.co/rhasspy/piper-voices> — voices dataset card, MIT.
- <https://huggingface.co/hexgrad/Kokoro-82M> — Apache-2.0 weights, v1.0 release facts, training data.
- <https://github.com/hexgrad/kokoro/issues/247> — the GPL espeak-ng dependency trace (2025-08 → 2026-10 comments).
- <https://github.com/hexgrad/misaki> — Apache-2.0 G2P, optional espeak fallback.
- <https://github.com/thewh1teagle/kokoro-onnx> — MIT wrapper, model sizes, "near real-time on macOS M1".
- <https://github.com/rishiskhare/tts-rs> and <https://github.com/rishiskhare/tts-rs/blob/main/src/engines/kokoro/phonemizer.rs> — MIT Rust Kokoro path that shells out to `espeak-ng`.
- <https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html> — Kokoro model packaging and required `espeak-ng-data`.
- <https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/rtf.html> — per-model RTF on Raspberry Pi 4 (Piper, Kokoro, MeloTTS, Matcha).
- <https://github.com/k2-fsa/sherpa-onnx> — Apache-2.0 engine, Rust bindings, KittenTTS/piper/Kokoro paths.
- <https://huggingface.co/Supertone/supertonic-3> and <https://github.com/supertone-inc/supertonic> — OpenRAIL-M weights, archived upstream, SDK and Rust example.
- <https://huggingface.co/cstr/supertonic-3-GGUF> — OpenRAIL-M restatement, one-file GGUF conversion.
- <https://huggingface.co/api/models/canopylabs/orpheus-3b-0.1-ft> — Apache-2.0 tag, Llama-3.2 base, 3.78B, gated.
- <https://github.com/resemble-ai/chatterbox> and <https://huggingface.co/ResembleAI/chatterbox> — MIT, model ladder, Perth watermark, reference-clip cloning.
- <https://huggingface.co/sesame/csm-1b> — Apache-2.0 card, gated, 1.55B.
- <https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B> and <https://huggingface.co/microsoft/VibeVoice-1.5B> — MIT, streaming text input, 1.02B BF16 / 2.70B BF16.
- <https://huggingface.co/bosonai/higgs-tts-2-3b-base> — Boson Community License derived from Llama 3.
- <https://github.com/myshell-ai/MeloTTS> — MIT, last commit 2024-12.
- <https://huggingface.co/coqui/XTTS-v2> — Coqui Public Model License.
- <https://huggingface.co/SWivid/F5-TTS> — CC-BY-NC-4.0 weights.
- <https://huggingface.co/nari-labs/Dia-1.6B> — Apache-2.0, 1.61B.
- <https://huggingface.co/KittenML/kitten-tts-nano-0.8-fp32> and <https://github.com/KittenML/KittenTTS> — Apache-2.0 v0.8 ONNX line, Stellon Labs licence for KittenTTS 2, chunk-level streaming caveats.
- <https://developer.apple.com/documentation/avfaudio/avspeechsynthesizer/write(_:tobuffercallback:)> — AVSpeechSynthesizer incremental buffer API, macOS 10.15+.
- <https://docs.rs/tts/0.26.3/tts/> — `tts` crate backends (AVFoundation, WinRT/SAPI, Speech Dispatcher) and whole-utterance surface.
- <https://contracollective.com/blog/kokoro-vs-piper-vs-xtts-local-text-to-speech-m5-max-2026> — third-party M5 Max RTF/TTFB comparison.
