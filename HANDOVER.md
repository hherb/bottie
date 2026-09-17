# Bottie handover

Last verified: 2026-09-17

## Start here

Start from the draft PR for `codex/linux-clean-runtime-input-lock` after it merges. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-nvidia-proof.md`, `docs/local-image-linux-worker-bundle.md`, and the three checked-in clean-runtime
input evidence JSON files.

## Current state

- The exact clean Ubuntu/ARM64 proof image remains
  `sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad`, 5,780,237,265 bytes. It passed
  Bottie's full offline private-protocol proof with zero `libucc` mappings and byte-identical reviewed output.
- Its 63 Python and 112 Debian installed identities are now bound to 175 authoritative archives totaling
  3,016,341,507 bytes. `docs/local-image-linux-clean-runtime-input-lock.json` has canonical lock SHA-256
  `2628e5ec74886a91b6b1b68890666d2340c7a99c9f20b51b9b9dc483946c9831`; the adjacent verification and provenance
  records preserve the independent result and official PyPI, PyTorch, and Canonical snapshot sources.
- The verifier selects only top-level wheel metadata, so legitimate vendored `.dist-info` records do not create false
  duplicates. Embedded and filename tags must otherwise match exactly. The sole exception is bound to the official
  `nvidia-cusparselt-cu13==0.8.0` ARM64 filename, 220,791,277-byte size, and PyPI SHA-256 because that wheel embeds
  NVIDIA's `manylinux2014_sbsa` spelling. Modified, renamed, or substituted wheels still fail closed.
- Real Debian inspection uses `dpkg-deb --show --showformat`; the previous `--field` invocation did not expand format
  placeholders and was found by the first production evidence run.
- `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. Linux local image generation remains
  unavailable. No rebuild, repeated trace, new closure, licence inference/review, bundle, app wiring, signing, release,
  Store, updater, or provider action occurred.

## Validation

- The production host generator recollected the exact image inventory offline, validated all 175 archives, generated
  the lock, and revalidated it before writing. A separate verifier run returned `verified: true`, exact counts 63/112,
  and total size 3,016,341,507 bytes.
- Focused clean-runtime lock and host suites pass: 15 tests, including top-level metadata selection, the byte-bound
  NVIDIA exception, real `dpkg-deb` command construction, checked-in manifest schema/count/order/digest, byte drift,
  foreign architecture, and renamed-wheel rejection.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. The
  default Vitest glob also found an unrelated ignored `.claude/worktrees/audio-cpp-asr-tts-spike-d9e3e8` checkout with
  no generated Svelte config; the clean rerun explicitly excluded `.claude/**`.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after the expected
  loopback-bind denial: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- `git diff --check` passes.

## Next slice

Produce two independently named network-disabled rebuilds from the frozen input lock:

1. obtain explicit authorization before transferring the additional reviewed worker/build source to the DGX; do not
   enable package egress because the retained 175-archive tree is complete;
2. define a build path that starts from only
   `sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90`, installs Debian and Python inputs only
   from the verified offline tree, and copies only the reviewed worker source;
3. build twice without network access or mutable tags, then compare exact installed inventories and a documented
   normalized regular-file filesystem measurement before rerunning the private-protocol proof and traced closure.

Do not infer licence expressions, create a licence-review manifest, update the original NGC closure, accept a Linux
profile, add an executable/hardware probe/app execution/UI, publish a bundle, or perform signing, release, Store,
updater, provider, or legal-terms activity.
