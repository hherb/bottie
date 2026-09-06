# Bottie handover

Last verified: 2026-09-06

## Start here

PR #154 merged into `main` at `9a9160e`. The current branch is
`codex/python-protected-package-comparison`. Microsoft Store certification and publication remain deferred until fresh
release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- A credential-free comparison now independently revalidates the accepted three-platform development manifest before
  accepting any future protected package. It checks the exact source revision, platform target, closed package shape,
  canonical input hashes, shared runtime core, and platform-specific runtime layout rather than trusting the manifest's
  accepted label.
- The protected package must retain the exact accepted runtime identity for its platform. Its signed runner and native
  transport bytes may differ, but their exact sizes and digests are preserved in the normalized result.
- Runtime equality cannot substitute for containment. A separate closed platform-native shipping record must contain
  all target-specific denial, private-pipe, cancellation, resource, and owned-process cleanup outcomes and must bind to
  the exact protected inspection digest. Windows and Linux additionally require installed-protected-package evidence;
  macOS requires inspection of the protected app.
- The command consumes and emits only bounded, path-free evidence. Default and protected package configurations are
  unchanged, and no credential, protected workflow, signing, notarization, release, publication, or Microsoft Store
  action is used.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The comparison is an
acceptance contract for future protected evidence, not that evidence itself. No current protected package contains the
Python resources, and no platform has produced a shipping containment record. The contract does not establish signing,
notarization, installed production behavior, release identity, publication, or Microsoft Store certification.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The initial focused test failed because the protected-package comparison module did not exist. Five focused tests cover
accepted runtime equality with changed signed native bytes; exact target and source matching; runtime drift; tampered
candidate evidence; missing, false, stale, unknown, and path-bearing containment fields; and the local command plus
dependency-inventory wiring.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` pass. Svelte reports zero errors or warnings;
307 frontend/script tests pass with 3 skipped. The explicit application Cargo format and check commands pass. Its test
suite passes with 501 library tests and 36 opt-in tests ignored, plus the updater-evidence test and doc tests. One
unchanged timing-sensitive macOS XPC cancellation fixture initially timed out waiting for its child fixture to start;
the exact isolated retry and the complete suite rerun both pass.

The Python runner's format, strict offline Clippy, offline tests, and locked offline release build pass. Seven unit tests
and the explicit missing-runtime guard pass, while three runtime-dependent tests remain intentionally ignored.
Dependency inventory regeneration/check, third-party notices, release assets, Prettier, workflow lint, and
`git diff --check` pass.

No browser or native-app review is required for this pure script contract. This macOS host cannot produce Windows or
Linux shipping containment, and this slice deliberately does not produce macOS shipping containment. No protected
distribution workflow was dispatched. Draft-PR hosted evidence is pending.

## Next bounded action

Add the credential-free macOS protected-package staging and inspection producer consumed by the comparison contract,
without changing the default package or claiming native shipping containment. Do not use credentials, dispatch
protected workflows, sign, notarize, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
