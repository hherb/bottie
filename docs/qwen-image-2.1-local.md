# Local Qwen-Image 2.1

Last verified: 2026-10-13

Bottie prefers an already installed `ddalcu/Qwen-Image-2.1-MLX-Serve-8bit` pack served by MLX Serve at
`http://127.0.0.1:11234/`. It reads `/v1/models` without loading or downloading anything and requires the exact pack,
`owned_by: mlx-serve`, explicit `image` capability, and loaded/ready state. The existing managed 2512 worker remains
available when that preferred server is absent. A start is bound to the model displayed before prompt persistence;
a retry is bound to the original package and seed. Neither operation switches models or falls back to Cloud.

## Reuse an existing installation

Start a Qwen-Image-2.1-capable MLX Serve build with the existing model directory:

```sh
mlx-serve --model /absolute/path/to/Qwen-Image-2.1-MLX-Serve-8bit \
  --serve --host 127.0.0.1 --port 11234
```

This reads the existing files in place, including a pack stored in oMLX's model directory. It does not copy the model
into Bottie's cache. Choose the **Image** tab, then **Local 2.1** in Bottie. Choose an aspect ratio and image size;
the default produces one 512×512 PNG using the base model's 40-step sampling schedule and an explicit seed.
The Chat tab retains chatbot tools and voice input; each tab keeps its own prompt draft. Model details are collapsed
until requested. Local editing is not enabled.

The preferred server supports three closed size tiers across five exact ratios:

| Ratio         | Small    | Medium   | Large     |
| ------------- | -------- | -------- | --------- |
| Square 1:1    | 512×512  | 768×768  | 1024×1024 |
| Landscape 3:2 | 768×512  | 1152×768 | 1536×1024 |
| Portrait 2:3  | 512×768  | 768×1152 | 1024×1536 |
| Wide 16:9     | 1024×576 | 1536×864 | 2048×1152 |
| Tall 9:16     | 576×1024 | 864×1536 | 1152×2048 |

These presets have aligned axes and bounded area. Rust permits them only for the exact 2.1 server package; the
legacy 2512 worker remains fixed at one 512×512 output. Settings/model identity are captured before asynchronous
prompt persistence, saved with the generated request, and retained on retry. No schema change or cloud fallback is
introduced. Larger sizes are allowed presets, not new measurements of their peak memory or latency.

The inspected oMLX 0.7.1.dev1 installation lists this pack as an unloaded text model and has no image-generation API.
Listing it in `/v1/models` does not make it usable for image output. MLX Serve supplies the separate
`/v1/images/generations` endpoint; Bottie keeps the configured oMLX text service unchanged.

## Optional installation and license

The upstream [Qwen Research License](https://huggingface.co/Qwen/Qwen-Image-2.1/blob/main/LICENSE) applies to the model.
Read and accept it before optionally downloading/installing a pack through MLX Serve's model manager. A conversion's
Apache-2.0 metadata does not replace the upstream model license. Existing installations can be reused immediately;
Bottie does not automatically download model weights or install/start the server.

Bottie's managed acquisition contract also requires an explicit revision-bound `licenseAccepted` acknowledgement for
research-licensed packages. The visible action says “Accept license and download”, with the license linked before the
action; omission, false acceptance, and changed package facts fail closed. The new 2.1 server route itself has no
managed download, worker-import, or license-acceptance action because its installation is owned by the external local
server. The Bottie-managed 2.1 installer now has a native model-acquisition foundation; its application action and
managed server setup are not yet available.

## Managed installer: model-acquisition foundation

`src-tauri/src/local_image_worker/managed_install.rs` implements the first optional-installer layer. It is compiled
and tested but is not connected to a Tauri command, composer action, or automatic startup. Constructing it and reading
its disclosure perform no network or cache mutation. The existing installed-server route continues to reuse files
in place.

The download is pinned to [the installed pack's immutable revision](https://huggingface.co/ddalcu/Qwen-Image-2.1-MLX-Serve-8bit/tree/fbda4caa0b4b1e17b5a29633e8deb600386f1eaf):

- Repository: `ddalcu/Qwen-Image-2.1-MLX-Serve-8bit`.
- Revision: `fbda4caa0b4b1e17b5a29633e8deb600386f1eaf`.
- Model files: 18, totaling 17,647,912,015 bytes (about 17.65 GB / 16.44 GiB); runtime bytes are separate.
- Included terms: `LicenseRef-Qwen-Research`, SHA-256
  `8dc973f024ff95966bea25866efa443fd16776dcb1001e681e3d467ea572b28d`.
- Native source inventory: `managed_install/qwen21-source.json`, including every size, SHA-256, and strong HTTP ETag.

Upstream API metadata and small-file contents were checked at the immutable revision. All 18 installed files were
hashed locally and match that inventory; no duplicate weight download was needed. Upstream `main` at inspection time
was `dc21b8d3441eef8f50ee5915dd5ea74679bcf338` and had a larger replacement text-encoder shard. That newer package is
not silently substituted for the installed, exercised pack.

Before any download or cache write, the native installer requires affirmative license acceptance plus the exact
repository, revision, license identifier, terms digest, and byte count. Omitted acceptance defaults to false; changed
facts fail closed. The disclosure links the immutable package LICENSE. Source URLs, file contracts, and locations
remain native. Resume is source-bound, full and ranged responses retain strong validators, and every file passes
size/hash verification before atomic promotion. Cancellation retains synced partials. A completed install is
reverified and reused without another download; corrupted promoted bytes fail closed.

The result type identifies **verified model files**, not generation readiness. The generic acquisition manifest's
internal 128 GiB memory ceiling is neither a measured peak nor a hardware qualification and is excluded from the
disclosure. No worker protocol, load request, server process, or generation capability is enabled by this layer.

The next slice must qualify and install the managed macOS ARM64 runtime, then add process ownership, loopback server
startup/shutdown, readiness probing, native mutation serialization, and an optional application action with license,
size, progress, cancellation, and resume presentation. The upstream runtime candidate is
[MLX Serve v26.10.2](https://github.com/ddalcu/mlx-serve/releases/tag/v26.10.2), source commit
`ccedc6a4820f48e576633062232ed0cba1139616`; its published ARM64 archive metadata says 78,894,505 bytes and SHA-256
`7082424c2d4c3821c1b323e2822aec79502cc82475b01fd2bfcd70d6ccd7ec27`. These are candidate source facts; that archive has
not been downloaded, unpacked, or accepted as Bottie's runtime. The existing 26.9.5-dev proof is separate evidence.

Focused native acquisition checks:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib managed_install
```

They cover exact source facts, all consent bindings, no-write rejection/pre-cancellation, resumed transfer, active
cancellation with retained partials, corrupt-byte rejection, and verified-cache reuse after the source disappears.

## Test DMGs

The refreshed tabbed-composer testing package is built from source commit
`71721b339e929d07083d772c3ededb1673d25ff2` (Bottie 0.9.0):

`src-tauri/target/release/bundle/dmg/bottie_0.9.0_image-tabs_71721b3_aarch64.dmg`

SHA-256: `a924fd02cb68574e428d428be4800e0707c9ab502e1f34c868109b378e62bc0e` (42,021,931 bytes).
It includes contained Python, Chat/Image tabs, and the size/aspect controls described above. Final read-only mounted
app inspection verifies the main development signature; disk-image verification and the packaged Python
execution/sandbox/cancellation/cleanup proof pass. The package remains unnotarized, without production Developer ID
signing. Apple Silicon and macOS 14 or newer are required; image-model weights and MLX Serve are not bundled.

The 2026-10-13 Apple Silicon testing installer was built **before** the managed-installer source changes, from commit
`57d82da96ff7f9652570f2fef1ed68726a800182` (Bottie 0.9.0):

`src-tauri/target/release/bundle/dmg/bottie_0.9.0_qwen21_57d82da_development_aarch64.dmg`

SHA-256: `8bdbbd6f461d4932471666d4a9ccbd926f004bbb2a6c84d0c8958fcd86849cfe` (42,022,347 bytes).
It requires macOS 14 or newer and contains the sandboxed Python runtime. No image-model weights or MLX Serve runtime
are bundled. `hdiutil verify` passed, and the contained-Python proof exercised the exact mounted app's private-pipe
execution, sandbox denial, cancellation, and runner cleanup. Inspection/checksum sidecars accompany the DMG.
The main app was signed with the existing Apple Development identity after the standard unsigned build's incomplete
bundle signature was detected. Its strict signature verification and the nested Python transport checks pass on the
final read-only mount. This is an unnotarized development testing package, without production Developer ID signing.
Testers may need macOS **Privacy & Security → Open Anyway** after attempting to open the downloaded app.

## Native boundary and evidence

- Rust sends the prompt only to literal IPv4 loopback, bypasses environment proxies, refuses redirects, and never
  follows a returned image URL. Requests contain no credentials, model paths, or cloud fallback instruction.
- Response bytes and decoded inline PNGs have independent bounds. Exactly one output is decoded and re-encoded by
  Bottie's existing PNG normalizer before private durable storage. Malformed output, changed dimensions, and cancelled
  results leave no uncommitted temporary bytes.
- Cancellation drops the HTTP request. The installed server's denoising callback detects the disconnected client and
  stops generation. Durable provenance records the actual quantized package, Local execution, and seed; existing
  generated-image asset, export, backup, and retry behavior is shared.
- Server-managed disk and memory fields are unused, rather than fabricated measurements. The composer says that it
  reuses the installed model and requires no additional model download.

On the Apple M3 Max 128 GiB host, MLX Serve 26.9.5-dev / MLX 0.32.2 generated a decoded 512×512 botanical robot PNG in
69.1 seconds. The opt-in native fixture then passed generation, normalization, durable completion with exact package
provenance, and cancellation in 71.59 seconds. The server log confirmed “generation cancelled — client disconnected”.
This is development/runtime evidence, not a signed installer or distribution claim.

The subsequent size/aspect slice also passed a live 768×512 generation, native normalization, durable completion
with exact dimensions/model provenance, and cancellation in 138.53 seconds. The browser controls were exercised at
1280×720 and 620×800, including arrow-key tab navigation, size/aspect changes, and preserved Image drafts. Browser
preview uses fixture conversation data; it does not perform native generation. Cloud size choices follow the
[provider's bounded dimensions API](https://www.alibabacloud.com/help/en/model-studio/qwen-image-api), while local
choices are a closed Rust-owned catalog. Full native Tauri UI clicks were not automated.

To rerun against an already installed, license-approved local server:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked \
  live_installed_qwen_21 --lib -- --ignored --nocapture
```

The fixture uses isolated temporary storage and a synthetic prompt; it does not read or modify private conversations.
