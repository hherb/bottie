# MOSS-Image local backend

Implementation: 2026-10-18. MOSS is an optional local image backend using the maintained Swift/MLX port on Apple Silicon
and the official Diffusers pipeline on Linux/Windows NVIDIA CUDA machines. SFT is the setup default; `--variant Base`
selects the base checkpoint. Hosted Qwen 2.0, the external Qwen 2.1 server, and the legacy 2512 worker remain distinct.
A configured MOSS installation becomes the disclosed local choice; failed MOSS work never silently switches to another
model or a cloud provider.

## Setup

From the Bottie source checkout, use Python 3.12 or later:

```sh
python3 scripts/moss-image/install.py --download
```

This is an explicit, large optional installation. Each selected model snapshot contains approximately **27.2 GB** of
pinned assets, in addition to the runtime and build dependencies. Setup verifies native GPU availability before
downloading, then sizes and SHA-256 hashes before atomically publishing `moss-image/runtime.json` under Tauri's app-data
directory. Existing verified files and partial downloads are reused. The app never downloads models, installs
dependencies, or builds a worker automatically.

On **Apple Silicon**, setup builds the pinned `xocialize/moss-image-swift` implementation using SwiftPM. The port
requires **macOS 26+**, Xcode/Swift 6.3 or later, and the Metal compiler component. If Xcode reports that Metal is
missing, install its official component with `xcodebuild -downloadComponent MetalToolchain`. The bf16 checkpoint has a
substantial memory footprint: the upstream port reports approximately 26.2 GB peak on its M5 Max fixture. That is
upstream evidence, not a measurement of another Mac or a promise that a lower-memory device will work.

On **Linux/Windows CUDA machines**, setup creates a dedicated Python environment using the exact direct versions in
`requirements-cuda.txt`, or `--reuse-python /absolute/path/to/venv/python` verifies an existing environment (including
official PyTorch `+cu` wheel tags). Use an appropriate CUDA-enabled wheel for the target architecture; a CPU-only ARM64
wheel is rejected. New environments explicitly acquire the official PyTorch CUDA-13 wheel, including ARM64, rather than
PyPI's CPU-only ARM64 build. An NVIDIA driver compatible with CUDA 13 and bfloat16 support are required; there is no
CPU, MPS, or cloud fallback. The worker uses GPU residency at 40 GiB device memory, component offload at 24 GiB, and
sequential offload below that. These are placement policies, not measured minimum-memory guarantees. Offload also needs
substantial system RAM. Live SFT qualification covers the Linux ARM64 NVIDIA GB10 worker; Windows and the lower-memory
offload policies still need hardware qualification.

To reuse an existing regular-file snapshot rather than download another copy:

```sh
python3 scripts/moss-image/install.py --model-directory /absolute/path/to/verified-snapshot
```

Use the exact selected snapshot from the inventory. Symlinks that escape the selected model directory are rejected.
`--reuse-executable /absolute/path/to/BottieMossImage` reuses a built Apple worker and its adjacent Metal resources.
`--app-data /absolute/path/to/app-data` supports alternate or sandbox-container installations. The default locations are
`~/Library/Application Support/com.bottie.app` on macOS, `$XDG_DATA_HOME/com.bottie.app` (normally
`~/.local/share/com.bottie.app`) on Linux, and `%APPDATA%/com.bottie.app` on Windows.

Restart an updated Bottie build after setup. In **Image**, choose **MOSS Image SFT** or **MOSS Image Base** in
Execution, then choose the aspect ratio and size. Only one configured MOSS variant is selected at a time. Rerunning
setup with `--variant Base` changes that native selection. Rename the registration file to disable MOSS and restore the
previous local discovery order; retain it to restore the installation later. The worker/model are external dependencies
and are not bundled in the Bottie installer or the contained `run_python` executor.

## Layout prompts

Paste ordinary descriptions or complete layout JSON into the existing image prompt. MOSS accepts up to **32 KiB of UTF-8
input**, separately from the existing Qwen limit. Each worker also checks the rendered chat template against the
encoder's **4,096-token** limit and fails rather than silently truncating the layout. No prompt enhancement or JSON
rewriting occurs. Match the selected output dimensions to the layout canvas.

For a 1024 × 1024 canvas, for example:

```json
{
  "summary": {"caption": "A cream poster with a navy BOTTIE title and a blue circle."},
  "items": [
    {"label": "text", "caption": "Large navy title at the top", "text": "BOTTIE", "bbox": [-448, -384, -320, 384]},
    {"label": "object", "caption": "A blue circle in the center", "bbox": [-160, -160, 160, 160]}
  ],
  "bbox_coord_space": "abs_centralized",
  "bbox_format": "yxyx"
}
```

Boxes are `[y_min, x_min, y_max, x_max]` in centered absolute pixels. Subtract half the canvas width/height from
ordinary x/y coordinates. The UI offers 15 exact ratio/size combinations from 512² through a 1792 × 1008 wide preset,
all within the port's 1456² area envelope. Every request generates one image using 50 Euler steps, CFG 3, shift 5,
negative-branch layer 22 skipping, and a native seed. Generated layout placement and spelling still require visual
review; this is not a deterministic typography or pixel-exact text-compositing API. Reference-image editing remains
available only through the existing hosted route.

## Native contracts

Rust owns registration paths, model bytes, runtime artifacts, private stdin/stdout, cancellation, PNG bytes, and
storage. Discovery checks the closed model/runtime/revision combination, executable and resource hashes, and model file
sizes. Before inference, every locked model/config/tokenizer/local pipeline file is hashed off-thread through a bounded
buffer. That verification cooperates with cancellation and finishes before releasing the one-active-run slot. Only an
exact registered executable is started, with no shell. Prompts travel over private stdin, never command-line arguments.
The worker receives only local asset paths; Hugging Face offline flags are set. Python also blocks socket audit events.
This is defense in depth, not an operating-system network sandbox or a distribution containment claim.

Each request owns a fresh child, avoiding a permanently resident GPU model. Stdout accepts a model/revision/runtime
handshake, bounded progress events, and exactly one matching PNG completion. Completion must match the requested seed
and dimensions and be followed by successful child exit and EOF. Events are capped at 34 MiB, decoded PNGs at 25 MiB,
startup at five minutes, private writes at five seconds, process exit at ten seconds, and the worker exchange at thirty
minutes. Local checksum verification runs before that exchange and has cooperative cancellation. Failure, cancellation,
and timeout kill/reap the exact child before the run completes. Worker parent-exit watchdogs also stop inference if
Bottie disappears unexpectedly. Stderr and dependency output never enter the WebView. Image-start, retry, and editing
registration share the microphone-admission mutex.

The existing PNG normalization removes metadata and validates decoded dimensions before app-private storage. Durable
provenance uses provider `moss-image`, the exact selected `repository@revision` identity, local execution, and seed; a
retry must match that family/model and reverify the current pinned installation. It cannot retry through Qwen. MOSS
readiness passes through the existing acquisition-status channel without offering an unrelated Qwen download.

## Sources and validation

The full inventory is `scripts/moss-image/models.lock.json`; it includes pinned license bytes and executable CUDA
pipeline code. The Swift bridge pins port revision `6c26a0296ba2d2a09e061d89a9b9b758786cd186` and commits its resolved
Swift dependency graph. The upstream port code is MIT; the model snapshots declare Apache-2.0. No upstream project's own
shipping approval is treated as Bottie approval.

Primary references: [Swift/MLX port](https://github.com/xocialize/moss-image-swift), [MLX SFT
snapshot](https://huggingface.co/mlx-community/MOSS-Image-SFT-bf16), [MLX Base
snapshot](https://huggingface.co/mlx-community/MOSS-Image-Base-bf16), and the [official pipeline and layout
guide](https://github.com/OpenMOSS/MOSS-Image).

Pure tests cover inventory safety and mutation, UTF-8 and image bounds, exact output identity, child EOF/exit, framing,
cancellation/reaping, durable retry family checks, initial status identity, and setup failure preservation. Browser
inspection uses the explicit development-only `?image=moss` fixture and checks real composer controls at desktop and 620
× 800 layouts. It is presentation evidence, not native Tauri click automation.

The opt-in native proof performs real layout generation, normalization, durable local provenance, active denoise
cancellation, and fresh-worker recovery through the same adapters:

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml live_moss_layout_persists_cancels_and_recovers \
  --lib -- --ignored --nocapture
```

Use release mode for this full-snapshot proof: the current ARM SHA-256 dependency uses a software path, and debug
optimization makes verification of 27.2 GB substantially slower. No checksum verification is skipped.

The Swift bridge and its real Metal array probe pass from the copied app-data runtime. Automated checks pass: 433
frontend/script tests (three existing skips), 14 Python adapter/setup tests, clean Svelte diagnostics, Prettier, Black
at 120 columns, Python byte compilation, the production frontend build, Cargo formatting, and locked Cargo checking. The
final serial host Rust suite passes 712 library tests (41 ignored), one updater fixture, six image-adapter fixtures, ten
transport fixtures, and doc tests. One existing XPC startup fixture timed out during a concurrent run; its unchanged
isolated retry and the complete serial host rerun pass. Download regression tests cover ordinary short EOF, HTTP framing
interruption, exact-range resume, and rejecting a complete wrong digest.

## Live hardware qualification

Both SFT workers completed real offline inference on 2026-10-18. Exact models, revisions, package versions, image
hashes, and scope are recorded in [`moss-image-host-proof.json`](moss-image-host-proof.json).

| Host and proof | Output | First complete image |
| --- | --- | --- |
| Apple M3 Max, 128 GiB, Swift/MLX through Bottie's native adapter | 1024 × 1024, seed 42 | 881.09 s |
| NVIDIA GB10, Linux ARM64, CUDA worker | 512 × 512, seed 42 | 195.28 s |

The Apple proof includes full snapshot re-verification, a fresh model process, PNG normalization, and durable
`moss-image` / exact-revision local provenance. It also cancels after real denoising begins and completes a subsequent
512 × 512 generation in a fresh worker, leaving no temporary image bytes. The complete three-request proof takes
1,377.60 seconds. The runtime and verified SFT snapshot are installed in stable Mac app data.

The CUDA proof uses a separate temporary environment and verified official SFT snapshot, without changing the DGX's
existing services or frozen image runtime. It uses PyTorch 2.14.1+cu130, CUDA 13.0, driver 580.173.02, and the exact
committed direct dependency versions. Both completed images report all 50 denoising steps. Cancellation after step two
kills/reaps the owned child in 0.78 seconds; fresh-worker recovery completes in 197.63 seconds. This proves the actual
CUDA worker, not the Linux native app/storage/UI flow. It does not register a new default backend on that host.

These are individual cold-path observations, with different image sizes and concurrent host work, not comparative speed
benchmarks. Peak inference memory was not measured. Base variants, Windows, lower-memory offload paths, native Tauri UI
clicks, and broader output-quality/performance qualification remain open. No production release or new model/runtime
distribution is created.

Visual review confirms correctly spelled requested words and the intended title/circle/subtitle arrangement in both
samples. The CUDA sample also adds decorative rules absent from the layout. Layout conditioning is useful, but generated
typography and placement remain approximate and require review.

[Apple layout sample](assets/moss-image/layout-proof.png) · [CUDA layout
sample](assets/moss-image/cuda-layout-proof.png) · [Composer browser fixture](assets/moss-image/composer-preview.png)

## Development installer

The development-signed Apple arm64 DMG includes the contained `run_python` executor and the MOSS adapter. Optional MOSS
runtime/model assets remain in app data. The installer is not notarized and is not a production release.

- Source: `88344d83e6177b0bf6811929ea3f7debb446f9a8`.
- Artifact: `src-tauri/target/release/bundle/dmg/bottie_0.9.0_moss_88344d8_development_aarch64.dmg`.
- SHA-256: `0e6dabc99d7f39d76bae21bb0b0a9e8d2fdf857b8af9839087b94ea2676d1a20`.
- Final DMG verification, strict recursive signatures, staged/mounted app-byte agreement, and contained-Python
  execution/cancellation/client-exit containment proofs all pass against the final mounted bytes. Local machine
  records are beside the DMG as `moss-88344d8-package-proof.json`, `moss-88344d8-mounted-inspection.json`, and
  `moss-88344d8-python-proof.json`.
