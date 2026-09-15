# Operations and troubleshooting

[Back to the manual index](index.md)

## Common failures

### Cargo cannot find a manifest

The root is not a workspace:

```sh
cargo check --manifest-path src-tauri/Cargo.toml
```

Use `python-runner/Cargo.toml` for the standalone runner.

### Native link/WebKit/audio compilation fails

Install Tauri 2 platform prerequisites and native libraries implied by `src-tauri/Cargo.toml`. Diagnose the earliest
compiler/linker error. Do not remove a cross-platform dependency to mask a host setup problem.

### Tauri API unavailable in browser

Expected: use `npm run tauri dev`. For UI review, use an existing deterministic development-preview pattern, never a
shipping fake-success path.

### Frontend test hangs

Check unresolved promises, channel/listener/timer cleanup, `onMount` disposal, and shared browser state. The project
intentionally uses one Vitest worker for deterministic shared-resource tests.

### Rust test flakes

Remove wall-clock, public-network, global-environment, port, and home-directory dependence. Use fake clocks/transports,
temporary directories, and local servers. Do not hide races with sleeps.

### SQLite busy/corrupt/migration error

Reproduce with a temporary store; do not alter a user's real data. For incidents, preserve the original/evidence and
follow `MIGRATION-ROLLBACK.md` before recovery.

### Provider stream fails

Separate discovery, mapping, transport, parsing, tool loop, and durable finalization. Reproduce with fixtures/local
servers and sanitized diagnostics. Never print auth headers, request bodies, attachment bytes, or raw errors.

## Diagnostics

Use stable event names, bounded detail, permitted provider identifiers, and sanitization helpers. Logs must not become
storage for prompts, mail, transcripts, Python, paths, or credentials. Even temporary debug output must avoid secrets;
a committed diagnostic needs redaction/bound tests.

## Dependencies

JavaScript dependencies/lock are in root package files; native ones are in `src-tauri/Cargo.toml`/`Cargo.lock`.
After intentional changes run:

```sh
npm run dependencies:check
npm run notices:check
```

Regenerate inventory/notices with scripts, not manual editing. Review licenses, native prerequisites, default features,
network behavior, and downloads. `runtime-assets.json` pins separately downloaded assets; changing it requires the
specialist integrity/proof tests.

## Packaging and releases

`scripts/` contains macOS package/distribution/XPC/signing, Windows MSI/MSIX/AppContainer/signing, Linux
DEB/containment/signing, updater/publication, candidate, asset, and notice tooling. Start with a matching `:test`,
`:inspect`, or `:check`. Build/sign/distribute commands may modify staging, install packages, require platform tools, or
need credentials—read the script and workflow first.

Build success, signature verification, install smoke, containment proof, candidate binding, and publication are
separate evidence. A development launch is not shipping proof.

Versions appear in `package.json` and `src-tauri/Cargo.toml`, with other contracts possibly binding them. Use release
scripts and inspect metadata. Credentialed/manual workflows in `.github/workflows/` are boundaries; never weaken branch,
digest, signature, candidate, or latest-release checks for local convenience.

Seek experienced paired review for Tauri capabilities/CSP/protocols, credentials/crypto/updater, SSRF/TLS/Localmail,
migrations/recovery/backups/permanent deletion, audio retention, protected Python/workers/containment/signing, and
cross-platform installer/permission/handle behavior. Junior developers should contribute there, but risk warrants
adversarial review.
