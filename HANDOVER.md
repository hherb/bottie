# Bottie handover

Last verified: 2026-09-17

## Start here

Start from the draft PR for `codex/linux-clean-runtime-offline-rebuild-gate` after it merges. Read `ROADMAP.md`
Milestone 8.4 and the clean-runtime sections of `docs/local-image-linux-worker-bundle.md` and
`docs/local-image-linux-nvidia-proof.md`.

## Current state

- The exact clean Ubuntu/ARM64 proof image remains
  `sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad`, 5,780,237,265 bytes. It passed
  the earlier full offline private-protocol proof with zero `libucc` mappings and byte-identical reviewed output.
- Its 63 Python and 112 Debian identities are frozen to 175 authoritative archives totaling 3,016,341,507 bytes. The
  canonical input-lock SHA-256 is `2628e5ec74886a91b6b1b68890666d2340c7a99c9f20b51b9b9dc483946c9831`.
- `docs/local-image-linux-clean-runtime-rebuild-plan.json` now binds the exact Ubuntu base, input lock, and byte sizes
  and hashes of the three reviewed worker files. The host gate re-verifies all inputs, then performs two independently
  named Linux/ARM64 BuildKit builds with networking, pulls, and cache disabled.
- Each result must have exactly the lock's installed inventory. Comparison binds canonical regular-file paths,
  permission modes, numeric ownership, sizes, and SHA-256 values while ignoring modification times and excluding only
  Docker's three runtime-owned hosts files. The evidence is path-free and bound to the plan and input-lock digests.
- The gate has not run. No source was transferred to the DGX, no image was rebuilt, and no proof, trace, closure,
  licence review, bundle, app wiring, product availability, signing, release, Store, updater, provider, or legal action
  occurred. `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false.

## Validation

- All 99 local image-worker Python tests pass, including 13 focused rebuild-plan, offline-command, path traversal,
  hard-link, inventory, source-drift, target, and normalized-filesystem tests.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. The
  default Vitest glob also found the unrelated ignored `.claude/worktrees/audio-cpp-asr-tts-spike-d9e3e8` checkout;
  the clean rerun explicitly excluded `.claude/**`.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after expected
  loopback-bind denials: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests. One unrelated XPC cancellation fixture timed out once, passed alone, and
  passed in the complete rerun.
- Python byte-compilation and Black formatting checks pass. `git diff --check` passes.

## Next slice

Execute and assess the two frozen-input rebuilds on the named DGX:

1. obtain explicit user authorization before transferring the minimal reviewed plan, input-lock, worker, inventory,
   lock-verifier, and rebuild scripts to the DGX; use the existing unlocked SSH authentication without requesting or
   recording its passphrase, and do not enable package egress;
2. confirm the exact base image and retained 175-archive tree are present, then run the documented host command with
   two fresh distinct build names and an evidence path outside both repository and input tree;
3. retain the path-free evidence only if both rebuilt inventories equal the lock and the normalized regular-file
   measurements agree exactly. If they agree, the following slice is to rerun the private-protocol proof and generate
   two fresh traces and a new closure bound to the rebuilt bytes.

Do not infer licence expressions, create or accept a licence-review manifest, update the original NGC closure, accept
a Linux profile, add an executable/hardware probe/app execution/UI, publish a bundle, or perform signing, release,
Store, updater, provider, or legal-terms activity.
