# MOSS-Image local backend

Implementation: 2026-10-18. MOSS is an optional local image backend using the maintained Swift/MLX port on Apple
Silicon and the official Diffusers pipeline on Linux/Windows NVIDIA CUDA machines. SFT is the setup default;
`--variant Base` selects the base checkpoint. Hosted Qwen 2.0, the external Qwen 2.1 server, and the legacy 2512
worker remain distinct. A configured MOSS installation becomes the disclosed local choice; failed MOSS work never
silently switches to another model or a cloud provider.

## Setup

From the Bottie source checkout, use Python 3.12 or later:

```sh
python3 scripts/moss-image/install.py --download
```

This is an explicit, large optional installation. Each selected model snapshot contains approximately **27.2 GB**
of pinned assets, in addition to the runtime and build dependencies. Setup verifies native GPU availability before downloading, then sizes and SHA-256 hashes before
atomically publishing `moss-image/runtime.json` under Tauri's app-data directory. Existing verified files and partial
downloads are reused. The app never downloads models, installs dependencies, or builds a worker automatically.

On **Apple Silicon**, setup builds the pinned `xocialize/moss-image-swift` implementation using SwiftPM. The port
requires **macOS 26+**, Xcode/Swift 6.3 or later, and the Metal compiler component. If Xcode reports that Metal is
missing, install its official component with `xcodebuild -downloadComponent MetalToolchain`. The bf16 checkpoint
has a substantial memory footprint: the upstream port reports approximately 26.2 GB peak on its M5 Max fixture.
That is upstream evidence, not a measurement of another Mac or a promise that a lower-memory device will work.

On **Linux/Windows CUDA machines**, setup creates a dedicated Python environment using the exact direct versions in
`requirements-cuda.txt`, or `--reuse-python /absolute/path/to/venv/python` verifies an existing environment (including official PyTorch `+cu`
wheel tags). Use an appropriate CUDA-enabled wheel for the target architecture; a CPU-only ARM64 wheel is rejected. CUDA and
bfloat16 support are required; there is no CPU, MPS, or cloud fallback. The worker uses GPU residency at 40 GiB device
memory, component offload at 24 GiB, and sequential offload below that. These are placement policies, not measured
minimum-memory guarantees. Offload also needs substantial system RAM. The CUDA backend is implemented but has not
yet been exercised on a CUDA host in this change.

To reuse an existing regular-file snapshot rather than download another copy:

```sh
python3 scripts/moss-image/install.py --model-directory /absolute/path/to/verified-snapshot
```

Use the exact selected snapshot from the inventory. Symlinks that escape the selected model directory are rejected.
`--reuse-executable /absolute/path/to/BottieMossImage` reuses a built Apple worker and its adjacent Metal resources.
`--app-data /absolute/path/to/app-data` supports alternate or sandbox-container installations. The default locations
are `~/Library/Application Support/com.bottie.app` on macOS, `$XDG_DATA_HOME/com.bottie.app` (normally
`~/.local/share/com.bottie.app`) on Linux, and `%APPDATA%/com.bottie.app` on Windows.

Restart an updated Bottie build after setup. In **Image**, choose **MOSS Image SFT** or **MOSS Image Base** in Execution,
then choose the aspect ratio and size. Only one configured MOSS variant is selected at a time. Rerunning setup with
`--variant Base` changes that native selection. Rename the registration file to disable MOSS and restore the previous
local discovery order; retain it to restore the installation later. The worker/model are external dependencies and
are not bundled in the Bottie installer or the contained `run_python` executor.

## Layout prompts

Paste ordinary descriptions or complete layout JSON into the existing image prompt. MOSS accepts up to **32 KiB of
UTF-8 input**, separately from the existing Qwen limit. Each worker also checks the rendered chat template against the
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
storage. Discovery checks the closed model/runtime/revision combination, executable and resource hashes, and model
file sizes. Before inference, every locked model/config/tokenizer/local pipeline file is hashed off-thread through a
bounded buffer. That verification cooperates with cancellation and finishes before releasing the one-active-run slot.
Only an exact registered executable is started, with no shell. Prompts travel over private stdin, never command-line
arguments. The worker receives only local asset paths; Hugging Face offline flags are set. Python also blocks socket
audit events. This is defense in depth, not an operating-system network sandbox or a distribution containment claim.

Each request owns a fresh child, avoiding a permanently resident GPU model. Stdout accepts a model/revision/runtime
handshake, bounded progress events, and exactly one matching PNG completion. Completion must match the requested seed
and dimensions and be followed by successful child exit and EOF. Events are capped at 34 MiB, decoded PNGs at 25 MiB,
startup at five minutes, private writes at five seconds, process exit at ten seconds, and the worker exchange at thirty
minutes. Local checksum verification runs before that exchange and has cooperative cancellation. Failure, cancellation, and timeout kill/reap the exact child before the run completes. Worker parent-exit
watchdogs also stop inference if Bottie disappears unexpectedly. Stderr and dependency output never enter the WebView.
Image-start, retry, and editing registration share the microphone-admission mutex.

The existing PNG normalization removes metadata and validates decoded dimensions before app-private storage.
Durable provenance uses provider `moss-image`, the exact selected `repository@revision` identity, local execution, and seed; a retry
must match that family/model and reverify the current pinned installation. It cannot retry through Qwen. MOSS
readiness passes through the existing acquisition-status channel without offering an unrelated Qwen download.

## Sources and validation

The full inventory is `scripts/moss-image/models.lock.json`; it includes pinned license bytes and executable CUDA
pipeline code. The Swift bridge pins port revision `6c26a0296ba2d2a09e061d89a9b9b758786cd186` and commits its resolved
Swift dependency graph. The upstream port code is MIT; the model snapshots declare Apache-2.0. No upstream project's
own shipping approval is treated as Bottie approval.

Primary references: [Swift/MLX port](https://github.com/xocialize/moss-image-swift),
[MLX SFT snapshot](https://huggingface.co/mlx-community/MOSS-Image-SFT-bf16),
[MLX Base snapshot](https://huggingface.co/mlx-community/MOSS-Image-Base-bf16), and the
[official pipeline and layout guide](https://github.com/OpenMOSS/MOSS-Image).

Pure tests cover inventory safety and mutation, UTF-8 and image bounds, exact output identity, child EOF/exit,
framing, cancellation/reaping, durable retry family checks, initial status identity, and setup failure preservation.
Browser inspection uses the explicit development-only `?image=moss` fixture and checks real composer controls at
desktop and 620 × 800 layouts. It is presentation evidence, not native Tauri click automation.

The opt-in native proof performs real layout generation, normalization, durable local provenance, active denoise
cancellation, and fresh-worker recovery through the same adapters:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_moss_layout_persists_cancels_and_recovers \
  --lib -- --ignored --nocapture
```

The Swift bridge builds successfully on the current Mac after installing the missing official Metal toolchain. The setup-time probe exercises a real Metal
array operation from the copied runtime and its adjacent resources before model acquisition.
Full inference qualification is recorded separately after that proof runs; CUDA hardware and native UI clicks are
not implied by mock, protocol, or build tests. No production release or new model/runtime distribution is created.

Automated checks pass: 433 frontend/script tests (three existing skips), ten Python adapter/setup tests, clean Svelte
diagnostics, Prettier, Black at 120 columns, Python byte compilation, the production frontend build, Cargo formatting,
and locked Cargo checking. The serial host Rust suite passes 711 library tests (41 ignored), one updater fixture,
six image-adapter fixtures, ten transport fixtures, and doc tests. The real Metal probe also passes from the copied
app-data runtime. Full model inference remains pending acquisition at this source checkpoint; its live result is
recorded separately rather than inferred from these checks.
