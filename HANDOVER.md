# Bottie handover

Last verified: 2026-09-17

## Start here

Start from `codex/linux-clean-runtime-wheel-tags` after its draft PR merges. PR #201 is merged at `d56b3a3`. Read
`ROADMAP.md` Milestone 8.4, `docs/local-image-linux-nvidia-proof.md`,
`docs/local-image-linux-worker-bundle.md`, and the three `diffusers_clean_runtime_{inventory,lock,lock_host}.py` files.

## Current state

- The exact clean Ubuntu/ARM64 proof image remains
  `sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad`, 5,780,237,265 bytes. It passed
  Bottie's full offline private-protocol proof with zero `libucc` mappings and byte-identical reviewed output.
- The clean image has 63 installed Python distributions and 112 installed Debian packages. Its build used mutable
  indexes; no complete authoritative artifact set, input lock, reproducible rebuild, repeated trace, or new closure
  exists yet.
- The offline lock gate requires exactly 63 wheels and 112 Debian archives matching the exact installed identities.
  It now also requires one bounded `WHEEL` member beside `METADATA` and exact agreement between embedded expanded
  compatibility tags and filename tags. A renamed foreign-platform wheel cannot satisfy the ARM64 gate.
- `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. Linux local image generation remains
  unavailable. No package download, source transfer, clean rebuild, licence inference/review, bundle, app wiring,
  signing, release, Store, updater, or provider action occurred in this slice.
- A read-only DGX connection reached the host but could not authenticate because the passphrase-protected key was not
  available to the invoking process. The user must unlock the existing key without sharing its passphrase in chat.
  Explicit authorization is still required before transferring new repository source or enabling remote package
  egress.

## Validation

- All 82 local image-worker Python tests pass, including renamed-foreign-wheel rejection and valid expanded-tag
  coverage.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after the expected
  loopback-bind denial: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- `git diff --check` passes.

## Next slice

Produce the first complete clean-runtime input lock on the named DGX:

1. obtain explicit authorization for the minimal reviewed source transfer and remote package egress, and have the user
   unlock the existing passphrase-protected SSH key locally;
2. obtain the exact authoritative ARM64 wheel for every one of the 63 installed Python identities and the exact `.deb`
   for all 112 installed Debian identities from one immutable Ubuntu source, retaining source provenance outside the
   path-free lock;
3. place only those 175 archives in the documented offline tree, run `diffusers_clean_runtime_lock_host.py` against the
   retained image, rerun `diffusers_clean_runtime_lock.py` independently, and retain only reviewed path-free lock and
   verification evidence.

Do not substitute versions, source distributions, foreign wheels, or different Debian archives. Do not rebuild yet,
infer licence expressions, create a licence-review manifest, update the original NGC closure, accept a Linux profile,
add an executable/hardware probe/app execution/UI, publish a bundle, or perform signing, release, Store, updater, or
provider activity.
