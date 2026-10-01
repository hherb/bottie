# Bottie handover

Last verified: 2026-09-22

## Start here

Start from the draft PR for `codex/linux-clean-runtime-license-sources` after it merges. Read `ROADMAP.md` Milestone
8.4 and the clean-runtime sections of `docs/local-image-linux-nvidia-proof.md` and
`docs/local-image-linux-worker-bundle.md`.

## Current state

- Two exact offline Linux/ARM64 rebuilds still agree across 63 Python identities, 112 Debian identities, and 27,035
  normalized regular files. The accepted rebuilt image is
  `sha256:740816cb8f348aa26e3d32f73b15b86b7f5a7228cff7b24d6910ee7aa0ee12a4`.
- The clean-runtime profile now accepts only the exact SentencePiece 0.2.2 and tokenizers 0.23.2 authoritative source
  archives. It requires both archives, exact component identities, archive/member sizes and hashes, and rejects partial
  input, licence-review manifests, the legacy NGC native-source set, image drift, lock drift, trace drift, and source
  drift.
- Collection is read-only, non-root, capability-free, and offline. A bounded in-process little-endian AArch64 ELF
  reader closes `NEEDED` and `SONAME` edges without adding `readelf` or changing the frozen runtime.
- The two independent collections agree on 161 files totalling 4,039,257,278 bytes, including 114 ELF files and 62
  components. `runtimeFilesSha256` is `f0f3e2dabe8777609f479ef81b3542e966f312a528183afda3f07368d43424d4`.
- `docs/local-image-linux-clean-runtime-closure-review.json` is the 23,223-byte path-free record with SHA-256
  `ebbca8034178339e6ba6007ad244ace868157a67f715dcd0628926444bd806fb`. `closureComplete` is true.
- No missing licence-byte blocker remains. Exactly 66 licence-only blockers remain: Jinja2, safetensors, tokenizers,
  and Triton have undeclared expressions, and all 62 component expressions are unreviewed. `licenseReviewed`,
  `assemblyEligible`, and `distributionReviewed` remain false.
- No licence-review manifest, bundle assembly, accepted Linux profile, hardware gate, app execution/UI, signing,
  release, Store, updater, provider, or legal action exists.

## Validation

- All 118 local image-worker Python tests pass, including exact source selection/completeness, profile/lock/trace
  rejection, canonical package identities, in-process AArch64 ELF parsing, legacy NGC behavior, and agreement.
- Python byte-compilation and Black checks pass; every changed implementation is below 500 lines.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. The
  default Vitest glob also found the unrelated `.claude` worktree; the clean rerun explicitly excluded `.claude/**`.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after expected
  loopback-bind denials: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- Both final DGX collections agree and reproduce the checked-in closure record SHA-256. JSON parsing and
  `git diff --check` pass.

## Recent maintenance

Branding maintenance completed on 2026-09-30: `assets/logo_v2/bottie_icon_512.png` is the approved app/general logo
master; the locked icon pipeline now derives both product favicons and the website header/footer logo from it.
`npm run package:macos:dmg` builds the
local unsigned macOS installer with contained Python by default; the explicit `:no-python` variant opts out. The
inspected arm64 DMG is neither distribution-signed nor notarized.

Credential and Localmail maintenance on 2026-10-01: macOS now stores credentials together in one keychain item.
Status queries request public attributes only, session authentication accepts Touch ID or a login password, and
accessible legacy entries migrate with keychain dialogs disabled. Older entries requiring separate approval remain
intact; re-save their API keys in Settings. Public retirement metadata prevents replaced/deleted legacy entries from
returning. Localmail setup now accepts administrator-issued `lmk_` API keys, still sent with the server's Bearer
scheme, and rejects expiring login-token drafts. No live prompt-count or Localmail-server acceptance is claimed.
The Settings follow-up includes Localmail key changes in Save and reconnect, keeps drafts on failed writes, and
uses draft credentials without eagerly reading locked legacy keys for search/cloud/image tests. Removal commits
legacy retirement before best-effort cleanup, so a legacy ACL cannot stop removal or resurrect its key. Regression
checks cover replacement/removal across restart and actual Settings clicks with fake vault commands.

Generation maintenance on 2026-10-02: recent oMLX failures hit the old four-round/eight-call tool budgets while the
server returned valid tool requests with roughly 7k context tokens. Generation limits are now saved in Settings,
defaulting to 12 rounds, 24 calls, and 8,192 output tokens per model request. Rust validates the saved values and
snapshots them before run provenance; all mapped provider loops use those snapshots. Older settings gain defaults.
The independent five-minute tool-work deadline, 120-second stream-idle timeout, and output-byte limits remain.
Validation: 413 frontend tests and 671 active native library tests pass; the explicit loopback fixture also completes
12 and 13 configured tool rounds. Settings clicks verify save/reopen, cancellation, and invalid-value rejection;
native persistence checks verify restart and legacy defaults. No live private conversation was replayed.

Localmail PDF maintenance on 2026-10-02: the attachment reader rejected the server's valid `offset`, `limit`, `total`,
and `next_offset` fields because it expected a closed text-only response. It now accepts paged responses and older
text-only responses, requests the first 12,288 characters, validates page metadata, and marks unread text truncated.
Malformed server responses now have a specific fixed decode-failure message without exposing native details.
Synthetic regression tests reproduce the rejection using the actual Localmail response schema. No private PDF was
retrieved; server-side text extraction must already have completed.
All 38 focused Localmail tests pass. The full host suite passes 673 tests but the existing shell-based Python
cancellation fixture times out waiting for its start marker, also when run alone. The actual development-signed DMG
passes packaged Python execution, App Sandbox containment, cancellation, and client-exit termination checks.

## Next product slice

Complete independent expression review for the exact clean closure without assembling a bundle:

1. obtain a complete independent review record for all 62 sorted component identities, including reviewed non-placeholder
   expressions for Jinja2, safetensors, tokenizers, and Triton; do not infer expressions from package metadata;
2. TDD a clean-profile-only two-trace review intake bound to the exact image, lock, trace contexts, source archives,
   licence/notice bytes, and per-component review evidence without weakening the retained NGC review route;
3. rerun both closures and retain output only if every expression and review byte agrees, no licence blocker remains,
   `licenseReviewed` and `assemblyEligible` are true, and `distributionReviewed` remains false.

Stop if a complete independent review record is unavailable. Do not author a review conclusion, infer an expression,
assemble or publish bytes, accept a Linux profile, add hardware probing/app execution/UI, or perform signing, release,
Store, updater, provider, or legal-terms activity.
