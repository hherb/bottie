# Debian package distribution-signing failures

- Status: resolved; protected signing and independent verification passed
- Last updated: 28 August 2026
- Validated source: `c5c26d2b0234d01ebe030b1827f587ad7effdfa3`
- Successful workflow run: [33150663200](https://github.com/hherb/bottie/actions/runs/33150663200)

## Executive summary

The Linux distribution workflow failed ten times before run 11 passed. The first two failures were secret-format
problems: an invalid Base64 value, followed by an OpenSSL private-key PEM that was not an OpenPGP secret-key export.
Those credential-shape problems are resolved.

The following seven runs reached the signing path. Each successive source change removed one ambiguity. The ninth run
proved that the unsigned DEB build, inspection, isolated smoke test, protected OpenPGP key import, passphrase-backed
signing, public/private key match, public keyring, canonical DEB payload, `_gpgorigin` insertion, and exact embedded
signature bytes were all working. It then failed when the hardened stock-GnuPG check verified that exact signature
over that exact payload.

The most likely cause of the protected signing failures through run 9 is **SHA-1 selection caused by the legacy
`debsigs` `--openpgp` signing argument**. Bottie's verification command deliberately treats SHA-1 as weak and rejects
it. A credential-free reproduction proves that this exact argument changes the detached-signature digest from SHA-256
to SHA-1 and recreates the observed `gpgv`-passes/hardened-GnuPG-fails split. The failed protected signature itself was
not retained, so the cause remains a high-confidence inference rather than a packet-level fact recovered from that
artifact.

Source `2e8586f` stopped forwarding the legacy GnuPG command. It accepts only the exact four arguments emitted by
`debsigs` 0.1.26, reconstructs a bounded signing command with SHA-256, checks that the packet digest identifier is 8,
and independently verifies from a dedicated clean GnuPG home. It also added a credential-free Ubuntu integration that
exercises the real `debsigs`, GnuPG, `gpgv`, and `debsig-verify` toolchain with an ephemeral test key.

Run 10 did **not** test that protected signing correction. Eleven package contracts and fifteen focused source tests
passed, but the new credential-free integration failed before the product build, protected-key preparation, signing,
verification, or evidence upload. Ubuntu 24.04's `dpkg-deb` defaults to zstd, while Bottie's intentionally bounded
archive selector and legacy `debsigs` 0.1.26 support the fixture's XZ form, not `control.tar.zst`/`data.tar.zst`. The
runner fixture had not selected compression explicitly. Source `c5c26d2` forced XZ, asserted the exact unsigned
archive members before signing, and added allowlisted failure-stage labels.

Run 11 passed every gate from that exact source: the real credential-free Ubuntu integration, locked product build,
inspection and isolated smoke, protected key preparation, fixed SHA-256 origin signing, exact embedded-signature
verification, `debsig-verify` policy verification, identity-free evidence upload, and cleanup. This successful result
strongly corroborates the zstd diagnosis for run 10 and proves that the combined signing correction works with the
configured protected key and passphrase.

The workflow retained only normalized evidence. Its signed DEB was removed during cleanup and was not published, so
there is current verified Linux distribution-signature evidence but no retained distributable package.

## Most likely cause of the protected signing failures through run 9

Ubuntu 24.04 installs legacy `debsigs` 0.1.26 for this workflow. Its signer invokes GnuPG with:

```text
gpg --openpgp --detach-sign --default-key <key-id>
```

The invocation is visible in the
[Debian `debsigs` 0.1.26 source](https://sources.debian.org/src/debsigs/0.1.26/debsigs/). At protected source
`3c01c17`, Bottie's wrapper forwarded those arguments unchanged. The wrapper's immediate `gpgv` check accepted the
resulting signature, but the later stock-GnuPG check used `--weak-digest RIPEMD160 --weak-digest SHA1`. GnuPG documents
that `--weak-digest` makes signatures using the named digest fail verification; it is a rejection control, not an
allow-list. See the
[GnuPG weak-digest documentation](https://www.gnupg.org/documentation/manuals/gnupg/GPG-Esoteric-Options.html).

A credential-free local reproduction with a new temporary RSA signing key produced this exact pattern:

- default detached signing used digest algorithm 8, SHA-256;
- adding `--openpgp` used digest algorithm 2, SHA-1;
- `gpgv` accepted the SHA-1 signature;
- ordinary `gpg --verify` accepted it;
- the same GnuPG verification with `--weak-digest SHA1` returned status 2;
- the hardened verification continued to accept the default SHA-256 signature.

That result explains all otherwise contradictory evidence from the latest failed protected run:

1. The simple preparation probe passes because it is signed without `--openpgp` and therefore uses a modern digest.
2. The wrapper's `gpgv` check passes because it does not mark SHA-1 as weak.
3. The exact embedded signature byte comparison passes because the signature is not being truncated or rewritten.
4. The hardened stock-GnuPG check fails because it explicitly rejects SHA-1.
5. Earlier `debsig-verify` calls return status 13 because cryptographic verification fails after policy selection.

The protected signature's packet digest was not retained, so the failed artifact cannot retrospectively prove that it
used algorithm 2. The correction instead fails closed unless every newly produced origin signature uses algorithm 8.

## What the failures have ruled out

The current evidence rules out or materially weakens these earlier hypotheses:

- **Missing public key:** the repository contains the public-only Bottie OpenPGP certificate and policy.
- **Wrong public key:** protected preparation confirms that the imported private key's fingerprint matches the
  published certificate.
- **Wrong passphrase or unusable private key:** the protected signing probe succeeds.
- **Broken public keyring:** a clean-home stock-GnuPG probe verifies with the dearmored `.gpg` keyring.
- **Wrong DEB payload order:** the wrapper receives the same `debian-binary`, control archive, and data archive bytes
  as Bottie's independent canonical reconstruction.
- **Binary or line-oriented signature corruption:** the latest attempt uses ASCII armor and proves that the embedded
  `_gpgorigin` bytes equal the signer's output byte-for-byte.
- **Missing signature member:** the latest attempt requires exactly one `_gpgorigin` member before verification.
- **The PATH wrapper intercepting `debsig-verify`:** the verifier is forced to use `/usr/bin/gpg`.
- **Package-build failure in the protected attempts:** every run that reached signing passed locked build, inspection,
  and distinct-identity smoke testing first. Run 10 stopped in the credential-free fixture before the product build.

## Workflow-run history

The first nine runs passed the credential-free source contracts and unsigned build/inspection/smoke steps; runs 1 and
2 then failed during protected-key preparation. Run 10 failed in the newly added credential-free integration before
the unsigned product build. All ten passed cleanup. Evidence upload was skipped in all ten failed runs.

### 1. Invalid Base64 secret

- Run: [32902092228](https://github.com/hherb/bottie/actions/runs/32902092228)
- Source: `195eab4206bb3febe15b63b83441e6e86b820e29` on `main`
- Failure: `base64: invalid input`
- Meaning: protected preparation could not decode the configured value. Signing did not run.

### 2. OpenSSL PEM supplied instead of an OpenPGP secret key

- Run: [32904535478](https://github.com/hherb/bottie/actions/runs/32904535478)
- Source: `195eab4206bb3febe15b63b83441e6e86b820e29` on `main`
- Failure: `Protected Linux signing key import failed.`
- Meaning: the decoded value was an encrypted OpenSSL PKCS#8/RSA PEM, not an export produced by
  `gpg --export-secret-keys`. Signing did not run.

### 3. Correct OpenPGP secret reached the combined signing path

- Run: [32911010457](https://github.com/hherb/bottie/actions/runs/32911010457)
- Source: `195eab4206bb3febe15b63b83441e6e86b820e29` on `main`
- Failure: `Linux distribution signing or verification failed.`
- Meaning: protected preparation passed, but the original redacted error did not distinguish signing from
  verification.

### 4. Full-fingerprint policy roots were insufficient

- Run: [32912902586](https://github.com/hherb/bottie/actions/runs/32912902586)
- Source: `fd5cb4cad69db8856a3a31badb1a91a6dc35e7cf`
- Failure: the same combined signing-or-verification error.
- Meaning: changing policy and keyring directories from a 16-character key ID to the full fingerprint did not resolve
  the failure.

### 5. Failure isolated to independent verification

- Run: [32927773942](https://github.com/hherb/bottie/actions/runs/32927773942)
- Source: `41e5b80fc3add18bbfb72a9aef7cf9e5a8659efc`
- Failure: `Linux distribution verification failed.`
- Meaning: the new preparation signing probe passed, `debsigs` returned successfully, and `_gpgorigin` existed. The
  failure was after signing, in independent verification.

### 6. `debsig-verify` returned status 13

- Run: [32938271200](https://github.com/hherb/bottie/actions/runs/32938271200)
- Source: `8f929172dda1838108798b996e115db76229c9ba`
- Failure: `Linux distribution signature verification failed.`
- Meaning: mapped `debsig-verify` status 13. The origin signature, policy root, and policy selection were present;
  cryptographic verification failed.

### 7. Canonical payload and checked-in public trust root were insufficient

- Run: [32953099601](https://github.com/hherb/bottie/actions/runs/32953099601)
- Source: `b5209d7de71ac8a1029ffbe7c27a9d0e2ee3f0a9`
- Failure: the same mapped status-13 verification error.
- Meaning: legacy `debsigs` input matched Bottie's verifier-order payload, the protected key matched the checked-in
  public certificate, and `gpgv` accepted the detached signature before embedding. The policy verifier still failed.

### 8. Conventional keyring and stock-GnuPG probe were insufficient

- Run: [32964969246](https://github.com/hherb/bottie/actions/runs/32964969246)
- Source: `6022dd9cc3e32588cc248d708f123b333e131a5a`
- Failure: the same mapped status-13 verification error.
- Meaning: a conventional binary `.gpg` keyring and clean verification home passed the exact stock-GnuPG probe. The
  public key, keyring format, and generic verifier invocation were therefore not the primary problem.

### 9. Exact embedded signature fails the hardened GnuPG check

- Run: [33048169287](https://github.com/hherb/bottie/actions/runs/33048169287)
- Source: `3c01c178aa4d323300b9f7a3430e19f53e7e2b2c`
- Failure: `Embedded Linux origin signature verification failed.`
- Meaning: payload equality, signer success, wrapper-side `gpgv`, exactly one signature member, and byte-for-byte
  embedded-signature equality all passed. The newly added hardened `/usr/bin/gpg --verify` check rejected those exact
  files before `debsig-verify` ran.

### 10. Credential-free integration fixture likely used Ubuntu's unsupported default compression

- Run: [33066322605](https://github.com/hherb/bottie/actions/runs/33066322605)
- Source: `2e8586fb0b6d0301360d5d25ae3050084548f5af`
- Retained failure: `[bottie] credential-free Linux distribution integration failed.`
- Confirmed boundary: eleven package contracts and fifteen focused source tests passed. The failure occurred in
  `package:linux:distribution:test:integration`; the unsigned product build, protected-key preparation, protected
  signing, independent verification, and evidence upload were all skipped. Cleanup passed.
- Likely cause: Ubuntu 24.04's `dpkg-deb` defaulted the tiny test package to zstd. The fixture did not pass a compression
  option, while the bounded production archive selector and legacy `debsigs` 0.1.26 do not accept that archive form.
  The retained log cannot prove the inner gate because the integration intentionally collapsed every error to one
  identity- and path-free line.
- Meaning: this run provides no new evidence about the protected key or the SHA-256 signing correction.

### 11. XZ integration fixture and fixed SHA-256 signing path pass

- Run: [33150663200](https://github.com/hherb/bottie/actions/runs/33150663200)
- Source: `c5c26d2b0234d01ebe030b1827f587ad7effdfa3`
- Result: every workflow step passed in 14 minutes 37 seconds.
- Credential-free proof: the XZ fixture passed the real Ubuntu `debsigs`, GnuPG, `gpgv`, and `debsig-verify`
  integration before any protected material was prepared.
- Protected proof: key import, passphrase-backed signing probe, published-certificate match, SHA-256 origin signing,
  exact embedded-signature verification, and `debsig-verify` policy verification all passed.
- Evidence: the 1,524-byte artifact archive `bottie-linux-distribution-evidence` (`9677902664`) was uploaded with
  artifact digest
  `sha256:297fc666800a91393b56e56219f07e5152446aff9b61544f5b3f6646c2580bc4`. Its normalized installer record is
  version `0.9.0`, architecture `amd64`, 22,879,288 bytes, SHA-256
  `e9ba241d23fbbe2c6a54b279ed2746a986f18657fd4dbc317ab9c3b25a48d960`, and
  `signature={classification: identified, verifies: true}`.
- Cleanup: protected material and signed package bytes were removed; only the bounded JSON evidence was retained.

## Source attempts made so far

### `fd5cb4c` — Fix Linux signature verification roots

Changed policy and keyring subdirectories from the short key ID to the full OpenPGP fingerprint and masked protected
values. This targeted policy-root lookup. It was not sufficient.

### `41e5b80` — Diagnose protected Linux signing failures

Added a passphrase-backed signing probe, stage-specific error messages, and an origin-member check before independent
verification. This established that the protected key could sign and narrowed the failure to verification.

### `8f92917` — Isolate Linux signature verification

Mapped `debsig-verify` exit statuses and set `DEBSIG_GNUPG_PROGRAM=/usr/bin/gpg` so the verifier could not resolve the
passphrase wrapper from `PATH`. This narrowed the result to status 13 but did not fix it.

### `b5209d7` — Fix canonical Linux distribution signing

Added the public-only certificate, checked-in policy and verification guide, a private/public fingerprint match,
independent canonical payload reconstruction, and a wrapper that captures the legacy signer's input and verifies its
signature before embedding. This targeted missing trust roots and payload-order disagreement. Status 13 remained.

### `6022dd9` — Use a stock GnuPG debsig keyring

Changed the dearmored keyring suffix from `.pgp` to conventional `.gpg`, created a clean verification home, and added a
stock-GnuPG verification probe. The probe passed, proving that the public keyring itself was usable. Status 13 remained.

### `3c01c17` — Verify exact embedded Linux signature bytes

Made a `debsigs`-versus-canonical payload mismatch fatal, emitted an ASCII-armored signature for the legacy
line-oriented transfer, compared the embedded member to signer output byte-for-byte, and added a hardened stock-GnuPG
check before `debsig-verify`. Run 9 passed the equality checks and failed the new GnuPG check. This disproved
payload mismatch, transfer truncation, and embedding rewrite as the primary cause.

### `2e8586f` — Force SHA-256 Linux distribution signing

Validated the exact legacy signer arguments, rebuilt a fixed SHA-256 GnuPG invocation, required packet digest
algorithm 8, supplied a dedicated clean verification home, and added the real Ubuntu toolchain integration. The
portable contracts passed in run 10, but the integration fixture failed before the protected path because its DEB
compression was not pinned. Consequently this run neither confirmed nor disproved the protected signing correction.

### `c5c26d2` — Fix Linux signing integration fixture

Forced XZ compression for the real-tool fixture, asserted its exact unsigned archive members, added bounded stage
diagnostics and fail-closed cleanup reporting, and moved both Linux workflows to Node-24 action majors. Run 11 passed
the real integration and the protected signing ceremony from this source.

## Resolved correction proven by run 11

The combined correction addresses the reproduced digest failure, the verifier-context mismatch, and the run-10
fixture defect without weakening any verification policy:

1. The wrapper validates the exact `--openpgp --detach-sign --default-key <key-id>` argument vector expected from
   `debsigs` 0.1.26 before it reads protected paths or standard input. Missing, reordered, mismatched, or injected
   arguments fail with one fixed identity- and path-free error; a lowercase hexadecimal environment identity is
   normalized consistently with the Node configuration boundary.
2. The wrapper discards that legacy argument vector and reconstructs `/usr/bin/gpg` with `--no-options`, the expected
   key identity, and `--digest-algo SHA256`. It does not forward `--openpgp` or arbitrary caller options.
3. Before emitting the signature to `debsigs`, the wrapper inspects the packet and requires digest algorithm 8,
   retains the canonical-payload comparison, and verifies the detached signature with the published public keyring.
4. The exact embedded-signature GnuPG check and `debsig-verify` now receive a dedicated clean verification
   `GNUPGHOME`; they no longer inherit the private signing home.
5. A Linux-only credential-free integration generates an ephemeral passphrase-backed RSA key and tiny XZ-compressed
   DEB, asserts the exact `debian-binary`, `control.tar.xz`, and `data.tar.xz` members, then runs the production signing
   function through Ubuntu's real `debsigs`, GnuPG, `gpgv`, and `debsig-verify`. It requires one `_gpgorigin`, SHA-256,
   successful hardened and policy verification, and explicit rejection of a generated SHA-1 control signature.
6. The real-tool integration runs in the normal ungated Ubuntu package-smoke workflow as well as before protected
   preparation in the manual distribution workflow. It uses no repository signing credential and reports the bounded
   `cleanup` stage if it cannot stop its temporary GnuPG agents or remove its entire temporary root.
7. Integration failures retain only one allowlisted stage (`host-preflight`, `fixture-setup`, `ephemeral-key`,
   `verification-policy`, `fixture-package`, `positive-signature`, `weak-digest-control`, or `cleanup`). Raw command
   output, identities, and paths remain suppressed.
8. Both Linux workflows use the current Node-24 action runtimes: `actions/checkout@v6`, `actions/setup-node@v6`, and
   `actions/upload-artifact@v7`. Bottie's project commands still run on Node 22; the action runtime and project runtime
   are separate.

TDD for the run-10 follow-up first failed because the XZ fixture contract and bounded integration-stage contract did
not exist and the Linux workflows still referenced Node-20 action majors. The focused signing suite now passes twelve
tests locally. Run 11 supplied the previously missing Ubuntu and protected-run proof.

The completed local matrix also passes 11 Linux package contracts, both workflow lints, policy XML validation, the
dependency and release-asset inventories, formatting, Svelte diagnostics with zero errors or warnings, 42 frontend
test files with 168 passing tests and three opt-in tests skipped, and the production build. Cargo formatting and
compilation pass; 398 Rust tests pass and 31 loopback, public-network, credential, live-provider, or performance tests
remain ignored. Doc tests pass with zero cases. The Node diagnostics emitted only the existing non-failing
`DEP0205 module.register()` deprecation warning.

## Why the portable tests and run 10 failed differently

The earlier source tests verified arguments, ordering, failure boundaries, path restrictions, evidence normalization,
and workflow policy using mocks and dependency-free fixtures. They did not run Ubuntu's real `debsigs` 0.1.26 together
with GnuPG's digest-policy flags. That is why every source suite could pass while the protected runner failed. The new
ephemeral Linux integration closes that credential-free toolchain gap on the normal Ubuntu package-smoke runner; run
11 separately proves the configured distribution key and protected environment.

Run 10 exposed a defect in that new integration fixture rather than the protected signing path. Ubuntu Noble documents
zstd as `dpkg-deb`'s default compression, but legacy `debsigs` accepts the XZ archive form used by Bottie's supported
canonical payload. See the
[Ubuntu 24.04 `dpkg-deb` manual](https://manpages.ubuntu.com/manpages/noble/man1/dpkg-deb.1.html). Portable source tests
could assert that XZ is required, but only a real Ubuntu invocation revealed that the fixture had omitted `-Zxz`.

## Node action-runtime warning

Run 10 also displayed GitHub's Node-20 action-runtime deprecation warning for `actions/checkout@v4` and
`actions/setup-node@v4`. It was not the failure: both actions completed, setup selected Node 22.23.2, and all Node
commands before the integration ran successfully. The workflow exited 1 only when the integration command failed.

The current workflow source updates the Linux jobs to
[`actions/checkout@v6`](https://github.com/actions/checkout),
[`actions/setup-node@v6`](https://github.com/actions/setup-node), and
[`actions/upload-artifact@v7`](https://github.com/actions/upload-artifact). These action majors run on GitHub's Node-24
action runtime; `node-version: 22` intentionally remains Bottie's project runtime. The workflow does not use
`ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION` or otherwise suppress the warning. Run 11 emitted no Node-20 action-runtime
deprecation warning.

## Secondary cause to keep controlled

At `3c01c17`, the final Node verification inherited `GNUPGHOME` for the private signing home while the preparation
probe used a separate clean verification home. This was a real context mismatch even though the SHA-1 reproduction
better explains the exact `gpgv`-passes/hardened-GnuPG-fails pattern. The current correction explicitly supplies the
clean verification home to both the exact GnuPG check and `debsig-verify`.

ASCII armor is unlikely to be the cause: the embedded bytes match the signer output, and both GnuPG and `gpgv` support
armored detached signatures.

## Resolved proof and remaining release boundary

The correction deliberately keeps the SHA-1 rejection and does not add `--allow-weak-digest-algos`. The source and
portable tests prove the fail-closed command boundary; run 11 proves the real Ubuntu toolchain and configured protected
environment work together at source `c5c26d2`.

The Linux distribution-signature evidence gate is satisfied by the reviewed JSON installed under ignored `/package`.
That evidence does not retain the signed DEB and does not authorize package publication, a tag, a release, or update
delivery. Any final release package must be rebuilt and revalidated from the exact current release source rather than
reusing deleted runner bytes or treating this evidence-only workflow as publication.

The evidence schema does not embed its Git commit or workflow-run provenance. Retain run `33150663200` and source
`c5c26d2b0234d01ebe030b1827f587ad7effdfa3` alongside the JSON because the normalized release manifest cannot by itself
distinguish stale same-version evidence.
