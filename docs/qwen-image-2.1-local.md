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
into Bottie's cache. Choose Image, then **Local 2.1** in Bottie. The current profile produces one 512×512 PNG using
the base model's 40-step sampling schedule and an explicit seed. Local editing is not enabled.

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
server. A Bottie-managed 2.1 installer remains separate work.

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

To rerun against an already installed, license-approved local server:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked \
  live_installed_qwen_21 --lib -- --ignored --nocapture
```

The fixture uses isolated temporary storage and a synthetic prompt; it does not read or modify private conversations.
