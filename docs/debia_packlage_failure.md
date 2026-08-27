# Debian package distribution-signing failures

- Status: source correction implemented; protected verification pending
- Last updated: 27 August 2026
- Latest protected source: `3c01c178aa4d323300b9f7a3430e19f53e7e2b2c`
- Latest protected run: [33048169287](https://github.com/hherb/bottie/actions/runs/33048169287)

## Executive summary

The protected Linux distribution workflow has failed nine times. The first two failures were secret-format problems:
an invalid Base64 value, followed by an OpenSSL private-key PEM that was not an OpenPGP secret-key export. Those
credential-shape problems are now resolved.

The following seven runs reached the signing path. Each successive source change removed one ambiguity. The latest run
proves that the unsigned DEB build, inspection, isolated smoke test, protected OpenPGP key import, passphrase-backed
signing, public/private key match, public keyring, canonical DEB payload, `_gpgorigin` insertion, and exact embedded
signature bytes are all working. It then fails when the hardened stock-GnuPG check verifies that exact signature over
that exact payload.

The most likely cause is now **SHA-1 selection caused by the legacy `debsigs` `--openpgp` signing argument**. Bottie's
verification command deliberately treats SHA-1 as weak and rejects it. A credential-free reproduction proves that
this exact argument changes the detached-signature digest from SHA-256 to SHA-1 and recreates the observed
`gpgv`-passes/hardened-GnuPG-fails split. The failed protected signature itself was not retained, so the cause remains
a high-confidence inference rather than a packet-level fact recovered from that artifact.

The working correction no longer forwards the legacy GnuPG command. It accepts only the exact four arguments emitted
by `debsigs` 0.1.26, reconstructs a bounded signing command with SHA-256, checks the resulting packet digest identifier
is 8, and independently verifies from a dedicated clean GnuPG home. It also adds a credential-free Ubuntu integration
that exercises the real `debsigs`, GnuPG, `gpgv`, and `debsig-verify` toolchain with an ephemeral test key. A fresh
protected run is still required before any Linux distribution evidence can be claimed.

No failed run uploaded distribution evidence, and the cleanup step removed protected material and package bytes each
time. There is still no verified or distributable Linux package.

## Most likely cause

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

That result explains all otherwise contradictory evidence from the latest protected run:

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
- **Package-build failure:** every run passes locked build, inspection, and distinct-identity smoke testing before
  signing.

## Protected-run history

All nine runs passed the credential-free source contracts and unsigned build/inspection/smoke steps unless the run
failed earlier during protected-key preparation. All nine passed cleanup. Evidence upload was skipped in every run.

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
check before `debsig-verify`. The latest run passed the equality checks and failed the new GnuPG check. This disproved
payload mismatch, transfer truncation, and embedding rewrite as the primary cause.

## Working correction after `3c01c17`

The current source correction addresses both the reproduced digest failure and the remaining verifier-context
mismatch without weakening any verification policy:

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
5. A Linux-only credential-free integration generates an ephemeral passphrase-backed RSA key and tiny DEB, then runs
   the production signing function through Ubuntu's real `debsigs`, GnuPG, `gpgv`, and `debsig-verify`. It requires
   one `_gpgorigin`, SHA-256, successful hardened and policy verification, and explicit rejection of a generated SHA-1
   control signature.
6. The real-tool integration runs in the normal ungated Ubuntu package-smoke workflow as well as before protected
   preparation in the manual distribution workflow. It uses no repository signing credential and removes its entire
   temporary root and agents.

TDD first failed on the missing argument allow-list, inconsistent lowercase identity handling, the inherited
verification home, and the absent Linux integration command. The focused source suite now passes 15 tests across the
signing and release-candidate contracts; shell syntax and ShellCheck pass. The real Ubuntu integration cannot execute
on the local macOS host, so its first runner result remains pending.

The completed local matrix also passes 11 Linux package contracts, both workflow lints, policy XML validation, the
dependency and release-asset inventories, formatting, Svelte diagnostics with zero errors or warnings, 42 frontend
test files with 166 passing tests and three opt-in tests skipped, and the production build. Cargo formatting and
compilation pass; 398 Rust tests pass and 31 loopback, public-network, credential, live-provider, or performance tests
remain ignored. Doc tests pass with zero cases. The Node diagnostics emitted only the existing non-failing
`DEP0205 module.register()` deprecation warning.

## Why local tests did not catch this

The earlier source tests verified arguments, ordering, failure boundaries, path restrictions, evidence normalization,
and workflow policy using mocks and dependency-free fixtures. They did not run Ubuntu's real `debsigs` 0.1.26 together
with GnuPG's digest-policy flags. That is why every source suite could pass while the protected runner failed. The new
ephemeral Linux integration closes that credential-free toolchain gap on the normal Ubuntu package-smoke runner, while
the separate protected run remains necessary to prove the configured distribution key and environment.

## Secondary cause to keep controlled

At `3c01c17`, the final Node verification inherited `GNUPGHOME` for the private signing home while the preparation
probe used a separate clean verification home. This was a real context mismatch even though the SHA-1 reproduction
better explains the exact `gpgv`-passes/hardened-GnuPG-fails pattern. The current correction explicitly supplies the
clean verification home to both the exact GnuPG check and `debsig-verify`.

ASCII armor is unlikely to be the cause: the embedded bytes match the signer output, and both GnuPG and `gpgv` support
armored detached signatures.

## Remaining proof and release boundary

The correction deliberately keeps the SHA-1 rejection and does not add `--allow-weak-digest-algos`. The source and
portable tests can prove the fail-closed command boundary; the automatic Ubuntu integration can prove the real
credential-free toolchain. Neither proves that the repository's protected environment is correctly configured.

After the source change is reviewed and published, a new protected workflow dispatch still requires separate explicit
authorization. That run is the only proof that the configured private key, passphrase, current runner packages, and
current source work together. Until it passes, do not upload, publish, or describe a DEB as verified distribution
output.

The protected success criterion remains unchanged: exactly one `_gpgorigin`, a modern OpenPGP digest, successful
stock-GnuPG verification over the verifier-order payload, successful `debsig-verify` policy verification, normalized
identity-free evidence upload, and unconditional cleanup.
