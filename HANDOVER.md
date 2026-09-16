# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #191 at `9d6e402`. Branch `codex/linux-image-worker-runtime-closure` adds the next
fail-closed Linux ARM64 Diffusers bundle-preparation gate. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `docs/local-image-linux-runtime-closure-review.json`, and
`local-image-worker/diffusers_runtime_closure.py`.

## Current state

- The existing exact DGX proof can opt into a Python audit and process-map trace. Trace context is written only after
  deterministic cold/warm generation, cancellation, and clean shutdown pass. The proof remains read-only, non-root,
  capability-free, and offline.
- The host closure gate verifies the exact derived image ID, injects the NVIDIA driver boundary with networking
  disabled, and emits only path-free evidence. It classifies observed Python/imported files and mapped native files,
  recursively closes active Python requirements, resolves ELF dependencies, and compares every package owner with the
  complete 248-Python/434-Debian environment record.
- Two independent GPU-injected DGX collections were byte-identical. The host-bound record is 27,889 bytes at SHA-256
  `fdb7bcf079976d2d456bf7c70b5a174609313d9eb9678d395a6c5ae3d3347820`. It contains 340 files totalling
  6,133,717,252 bytes, 295 ELF files, and 78 package components. It observes `libcuda.so.1`, `libnvidia-ml.so.1`, and
  `libnvidia-ptxjitcompiler.so.1` as host-supplied driver interfaces.
- Assembly stops with status 3 on 108 blockers: three ambiguous ELF SONAMEs, fourteen unowned file hashes, two missing
  licence-byte records, eleven undeclared licences, and 78 unreviewed expressions. `closureComplete`,
  `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false.
- No bundle was assembled, accepted, imported, or executed. Linux remains `CandidatePreparationOnly`; no product
  profile, executable, hardware probe, app execution, IPC/UI, signing, release, Store, or updater behavior changed.

## Validation and limits

The exact traced proof passed deterministic cold/warm output, cancellation, process-map capture, and clean shutdown on
the named DGX Spark. The final exact-image suites pass 15 active tests with one host-only orchestration test skipped;
all 44 local Python worker tests pass. Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped),
and the production build pass. Cargo formatting/checking pass; the identical host-local Rust suite passes 649 active
library tests (37 ignored), updater evidence, all 6 execution-adapter tests, all 10 private-worker transport tests, and
doc tests after the expected sandbox loopback denials. Two GPU-injected classifier runs produced identical path-free
records. No browser or native-app review was required because this slice changes no presentation or app execution.

The closure is runtime evidence, not redistribution approval. Its recursive Python graph intentionally includes
required components with zero directly observed files. It excludes model/output bytes, temporary and pseudo-filesystem
paths, and proof tracing code. The NVIDIA boundary is a closed SONAME allowlist, not bundled driver ownership.
Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

On the same DGX Spark and exact derived image, resolve only the seventeen closure-identity blockers: classify each of
the fourteen unowned file hashes from authoritative Python/Debian/first-party provenance, and disambiguate the three
colliding extension-module SONAMEs by exact requester and owned-file identity. Add regression tests for every general
classification rule; do not assign ownership from filenames alone or suppress a collision merely to reduce the count.
Regenerate two deterministic path-free records and require `closureComplete: true` before proceeding.

Do not review licences or assemble bytes in that slice. A later slice must obtain authoritative licence/notice bytes
and independently review the expression for every one of the 78 exact components before bundle assembly. If any
identity remains unprovable, stop with the path-free blocker summary. Keep Linux at `CandidatePreparationOnly`: do not
add an accepted profile, executable, hardware probe, worker import/download, application execution, UI/IPC field,
Docker product dependency, cloud fallback, local editing, guessed Qwen Image 2.0 weights, signing, release, Store, or
updater publication.
