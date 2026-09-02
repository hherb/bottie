# Bottie handover

Last verified: 2026-09-02

## Start here

PR #130 merged into `main` at `c412f8a`. The current updater-publication slice is on
`codex/updater-publication`. Milestones 0–7 remain complete.

Read, in order:

1. `HANDOVER.md`
2. the Milestone 7 section of `ROADMAP.md`
3. `distribution/update/README.md`
4. the release sections of `README.md`

## Completed slice

The protected outside-Store updater pipeline now:

- lets the existing macOS, Windows, and Linux required-reviewer workflows run as reusable jobs and export only their
  exact verified updater artifact plus `.sig` as one-day workflow artifacts;
- requires an exact 0.9.0 full-release confirmation and fresh Gemma-terms acknowledgement;
- refuses a non-`main` dispatch, changed `main`, existing tag, or existing release;
- requires the current-source release-candidate gates, including Developer ID/notary/Gatekeeper, direct MSI and
  executable Authenticode, Linux OpenPGP, platform smoke, and exact updater-signature evidence;
- builds `latest.json` only from three canonical final artifacts that match protected hashes, sizes, signatures, target,
  and production public-key hash;
- creates a complete GitHub draft, verifies the exact source, tag, asset set, sizes, and GitHub SHA-256 digests, then
  publishes and verifies the same tag through GitHub's latest-full-release API; and
- retains only path-free publication evidence and removes downloaded release bytes and raw API responses.

The release is beta-labelled but intentionally a full GitHub release because Bottie's fixed
`/releases/latest/download/latest.json` endpoint cannot resolve a GitHub prerelease. No protected workflow was
dispatched, and no tag, release, updater asset, or `latest.json` was created during this source slice.

The release candidate now uses the direct Authenticode MSI for this outside-Store channel. Microsoft Store
certification and publication remain separately deferred until fresh release-owner notice. The updater workflow does
not inspect, submit, poll, certify, or publish Store state.

The locked Linux `speech-dispatcher 0.16.0` and `speech-dispatcher-sys 0.7.0` manifests declare
`LGPL-2.1 OR MIT OR Apache-2.0`. Those exact versions were reviewed onto the existing MIT/Apache notice path; later
versions still fail closed. The generated dependency inventory and third-party notices are current.

## Current limits

GitHub environment secret names were inspected without accessing values. `macos-distribution` has the updater key and
password but still needs these existing Apple credentials:

- `BOTTIE_APPLE_DEVELOPER_ID_P12_BASE64`
- `BOTTIE_APPLE_DEVELOPER_ID_P12_PASSWORD`
- `BOTTIE_APPLE_NOTARY_KEY_P8`
- `BOTTIE_APPLE_NOTARY_KEY_ID`
- `BOTTIE_APPLE_NOTARY_ISSUER_ID`
- `BOTTIE_CI_KEYCHAIN_PASSWORD`

`windows-distribution` has the updater key and password but still needs:

- `BOTTIE_WINDOWS_SIGNING_PFX_BASE64`
- `BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD`

The `updater-publication` environment is configured with the repository owner as its required reviewer and must remain
protected. Missing platform credentials are not platform evidence, and source tests or unsigned builds are not native
artifact acceptance.

## Validation

The full standard frontend and Rust suites, focused updater/release tests, dependency and notice checks, workflow lint,
and complete diff review passed for this branch. `npm run release:candidate` fails closed only on the expected absent
current macOS, Windows, and Linux protected evidence gates. No UI changed, so browser visual review was not applicable.
The existing `block 0.1.6` future-incompatibility warning is unrelated.

Bottie is not a Cargo workspace; run Cargo commands serially with `--manifest-path src-tauri/Cargo.toml`.

## Next bounded action

After this draft PR merges, configure the existing Apple and Windows credentials in their named protected environments
without exposing values. Re-read and explicitly accept the current Gemma terms, dispatch `Updater publication` from
the exact current `main`, approve each protected environment, and monitor the complete run. Only after the live
latest-release verification passes should the actual platform artifacts, GitHub release, and retained evidence be
documented as published.

Do not resume Microsoft Store certification or publication without fresh release-owner direction. Preserve the
unrelated untracked logo-kit, screenshot, and Linux signing-public-key files.
