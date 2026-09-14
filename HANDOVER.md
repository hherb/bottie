# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #175 at `593e5bd`. Branch `codex/local-image-availability-service` continues Milestones
8.3-8.4. Read `ROADMAP.md` Milestones 8.3-8.4, `docs/local-image-model-package.md`, and
`src-tauri/src/local_image_worker/`.

## Completed slices

- `LocalImageAvailabilityService` resolves only `local-image-worker/bottie-local-image-mlx-worker` beneath Tauri's
  app-resource directory and `local-image-models` beneath app data. It rejects relative or lexically traversing roots,
  performs no construction-time filesystem mutation, and treats a symlinked worker root as an integrity mismatch.
- Each request is serialized and moved to a blocking native task. Hardware precedence avoids expensive worker/model
  hashing on unsupported hosts, worker failure avoids model hashing, and the selected q4 manifest is re-verified only
  after the exact accepted worker passes.
- `get_local_image_availability` returns exact model, package, runtime, license, revision, disk, and measured-memory
  metadata with one coarse readiness state. It serializes no path, hash, hardware marketing string, or correlation ID.
- The image composer loads this status without blocking other startup work and shows the exact local 2512 package,
  measured 16.2 GiB model/27.5 GiB peak-memory disclosure, and a fixed failure reason. The only executable action
  remains visibly Cloud `qwen-image-2.0-2026-03-03`; there is no automatic download or fallback.

## Validation

Formatting, Svelte diagnostics (0 errors/warnings), production build, 373 frontend/script tests (3 skipped), 607 Rust
library tests (36 ignored), the updater test, all 10 local-image transport tests, and Rust doc tests pass. The sandboxed
Rust suite could not bind loopback fixtures; the identical host-local suite passed. Focused service tests cover exact
layout, unsafe roots/symlinks, repeated read-only requests, and byte-for-byte path/hash-free JSON. The development-signed
native app launched and remained running without a provider request. Browser presentation was reviewed at the desktop
viewport and 760px with the Context panel closed; the Cloud/local distinction and local disclosure remained readable.

## Next slice

Add a Rust-owned local image execution adapter that accepts only a freshly `ready` service result, obtains the verified
native worker/model locations without crossing IPC, and maps one text-to-image request through the existing private
worker manager into the shared PNG validation, durable generated-asset storage, provenance, progress, and cancellation
contracts. Use the fixture bundle/cache for tests; do not bundle or auto-download the 1.1 GB proof worker or 16.2 GiB
model, enable a local composer selection before the complete native route exists, add editing, generalize hardware,
silently fall back to cloud, or reuse Bottie's approved Python-tool runtime.

Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate
authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
