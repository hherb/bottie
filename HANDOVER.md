# Bottie handover

Last verified: 2026-09-17

## Start here

Start from `codex/linux-image-clean-runtime-lock`. PR #199 is merged at `d156fa1`. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-nvidia-proof.md`, `docs/local-image-linux-worker-bundle.md`, and
`local-image-worker/diffusers_clean_runtime_lock_host.py`.

## Current state

- The clean Ubuntu/ARM64 proof image remains
  `sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad`, 5,780,237,265 bytes. It passed
  the full offline private-protocol proof with zero `libucc` mappings and byte-identical reviewed output. It has no
  HPC-X installation or UCC runtime library; PyTorch still contains UCC-named C++ headers.
- Its base is Ubuntu 24.04 ARM64 at
  `sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90`. The proved image contains 63
  Python distributions and 112 installed Debian packages, but the build used mutable indexes and no complete input
  artifact set or lock exists yet.
- `diffusers_clean_runtime_lock_host.py` now inspects the exact clean image ID, collects its complete installed
  identities offline by immutable ID, and generates a canonical path-free lock only from exactly 63 wheels and 112
  Debian archives. `diffusers_clean_runtime_lock.py` independently reverifies that lock and artifact tree.
- Both gates reject missing/extra/symlinked files, path-shaped or duplicate identities, source distributions, non-ARM
  platform wheels, foreign Debian architectures, wheel filename/metadata drift, Debian control-metadata drift,
  byte/hash drift, count drift, and lock-digest drift. Collection is read-only, capability-free, offline, and uses
  `no-new-privileges`; generation and verification never download, install, extract, or license packages.
- No DGX/source transfer, package download, clean rebuild, repeat trace, new closure, licence inference/review, bundle,
  accepted profile, executable, hardware probe, app wiring, or distribution action occurred in this branch.
- `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. Linux local image generation remains
  unavailable. Unrelated logo-kit, screenshot, and Linux public-key files remain untouched.

## Validation

- All 80 local image-worker Python tests pass.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after the expected
  loopback-bind denial: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- `git diff --check` passes.

## Next slice

Produce the first complete clean-runtime input lock on the named DGX without broadening product scope:

1. obtain the exact authoritative wheel for each of the 63 installed Python identities and reject every source build,
   missing ARM64 wheel, dependency/version substitution, or extra artifact;
2. obtain the exact `.deb` for each of the 112 installed Debian identities from one immutable Ubuntu source and reject
   version, architecture, control-metadata, or byte drift;
3. place only those 175 artifacts in the documented offline tree, run `diffusers_clean_runtime_lock_host.py` against
   the retained clean image, independently rerun `diffusers_clean_runtime_lock.py`, and retain only the path-free lock
   and verification evidence after reviewing source provenance.

Obtain explicit authorization before transferring new repository source to the remote target or enabling remote package
egress. Do not rebuild yet, infer licence expressions, create a licence-review manifest, update the original NGC
closure, accept a Linux profile, add an executable/hardware probe/app execution/UI, publish a bundle, or perform
signing, release, Store, updater, or provider activity.
