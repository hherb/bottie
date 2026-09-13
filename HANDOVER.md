# Bottie handover

Last verified: 2026-09-13

## Start here

PR #164 merged into `main` at `3a08b32`. The current branch is `codex/qwen-image-2-provider`.

Read the Milestone 8 generated-images section of `ROADMAP.md`, then `src-tauri/src/image_generation/mod.rs` and
`src-tauri/src/image_generation/dashscope.rs`.

## Completed slice

- Added a provider-neutral native image-generation contract with bounded prompt, dimension, output-count, capability,
  execution, and temporary-result types.
- Added an exact synchronous DashScope adapter pinned to `qwen-image-2.0-2026-03-03`; it uses the documented
  multimodal-generation route and request shape, disables redirects, keeps the bearer key in a sensitive header, limits
  the response envelope, requires terminal assistant choices, and accepts only the exact count of HTTPS image URLs.
- Added a credential-free persisted DashScope/Model Studio workspace root and a `qwen-image` operating-system-vault
  credential identity, including the existing one-authentication session warmup path.
- Added a path-free Tauri setup-validation command and Settings card. Validation checks the normalized HTTPS root,
  credential shape, fixed model identity, and static capabilities without making a provider request, generating an
  image, or incurring a model charge.
- Documented the staged hosted path and the explicit no-local-weights boundary. Future local execution must implement
  the same contract after exact Qwen-Image-2.0 weights and compatible runtimes are actually published; do not route the
  model through oMLX or an older Qwen-Image implementation.

No billable provider request, generated-image download, image persistence, conversation-schema change, local runtime,
model download, release, signing, publication, workflow dispatch, or Store action is included. Unrelated untracked
logo-kit, screenshot, and Linux public-key files remain untouched.

## Validation

The full frontend/script suite passes 352 tests with 3 skipped across 69 passing and 1 skipped files.
`npm run format:check`, `npm run check`, and `npm run build` pass.

Application Cargo formatting and `cargo check` pass. The serial application suite passes 512 library tests with 36
ignored plus the updater-evidence binary test; doc tests pass. Cargo reports only the existing future-incompatibility
notice for `block 0.1.6`.

The browser presentation was reviewed at a desktop viewport: the exact checkpoint, credential and endpoint fields,
non-billable validation disclosure, cloud badge, and native-only disabled state render cleanly. The native app built and
launched successfully. macOS accessibility automation could not inspect the running native window, so no native
interaction is claimed. No live Model Studio test was run because this slice deliberately performs no provider I/O.

## Next boundary

Implement one explicit, cancellable text-to-image action that requires a saved configuration and clear cloud-delivery
and cost disclosure. Rust must call the existing exact adapter, immediately download each temporary result, validate
content type, decoded format, dimensions, and byte/pixel ceilings, then store content-addressed app-private PNG bytes.
Only path-free durable image metadata may cross IPC.

Add assistant-authored image persistence and exact `providerId`, `modelId`, and `execution` provenance before showing a
generated result in the conversation. Do not reuse the current user-attachment association in a way that attributes an
assistant-generated image to the user. Editing/reference-image support is a later slice.

Do not add a local Qwen-Image-2.0 adapter until exact weights are publicly available and verified. At that point, assess
MLX on Apple silicon and released CUDA/ROCm/Windows-capable runtimes behind explicit capability discovery and execution
choice, with no silent cloud fallback.

After the shared durable generation slice, add the distinct open `Qwen/Qwen-Image-2512` local text-to-image track:
MLX-Gen first on Apple silicon, pinned Diffusers on proven Linux/Windows NVIDIA hardware, then evidence-gated Linux ROCm
and a possible stable-diffusion.cpp Vulkan/GGUF fallback. Keep 2512 and exact 2.0 model identities visibly separate.

App-store shipping and other distribution work are paused while Milestone 8 is the active product priority.

Do not commit, push, open a PR, dispatch workflows, sign, release, publish, or perform Store work without separate
authorization.
