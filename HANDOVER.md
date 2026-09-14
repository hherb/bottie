# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #174 at `46191d0`. Branch `codex/local-image-availability` completes the next bounded
Milestones 8.3-8.4 availability slices. Read `ROADMAP.md` Milestones 8.3-8.4,
`docs/local-image-model-package.md`, and `src-tauri/src/local_image_worker/`.

## Completed slice

- A pure path-free evaluator now fails closed in this order: unsupported platform, unsupported architecture,
  insufficient physical memory, unproved hardware profile, missing/mismatched worker, missing/mismatched model, then
  ready. It uses the selected package's measured 29,526,129,448-byte peak and still requires the exact accepted
  `apple-m3-max-128gb` evidence profile; it does not generalize support to other Apple-silicon machines.
- The native macOS probe reads `hw.memsize` and `machdep.cpu.brand_string` through `sysctlbyname`, combines those with
  the compile-target architecture, and maps only Apple M3 Max plus exactly 128 GiB to the accepted profile. The current
  host reported `Apple M3 Max` and 137,438,953,472 bytes.
- Availability re-hashes the exact installed executable and canonical symlink-free worker bundle against selected
  evidence. Promoted model inspection reuses the all-files activation gate through a new read-only cache path that does
  not create or repair an absent cache. Five focused tests cover every state, exact-profile rejection, byte drift,
  cache tampering, and no-mutation absence.

## Next slice

Add a Rust-owned local-image availability service that resolves one fixed app-resource worker layout and one app-data
model-cache root, evaluates the selected q4 package off the UI thread, and exposes typed path-free metadata through a
read-only Tauri command. Test missing/unsafe layouts, repeated requests, exact IDs and byte requirements, and serialized
absence of paths and hashes. Do not add Svelte presentation yet, package or auto-download the 1.1 GB proof bundle, start
the worker, load/generate images, or perform an expensive re-hash on the WebView thread.

Keep local 2512 distinct from hosted Qwen Image 2.0. Do not silently fall back to cloud or reuse Bottie's approved
Python-tool runtime. Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without
separate authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
