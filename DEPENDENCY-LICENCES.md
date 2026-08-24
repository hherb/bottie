# Bottie dependency and licence review

Reviewed: 2026-08-24

This is a technical inventory, not legal advice. It records the exact locked package metadata and non-package runtime
assets selected by the current Bottie source tree so release work has an explicit, reproducible starting point.

## Reproduce the inventory

`dependency-inventory.json` is generated from the two lockfiles without network access or package build scripts:

```sh
node scripts/dependency-inventory.mjs
npm run dependencies:check
npm run notices:check
npm run release:assets:check
```

The check runs `cargo tree --locked --offline` for macOS arm64/x64, Windows x64, and Linux x64 normal/build graphs,
reads npm's version-3 lockfile directly, hashes the reviewed manifests, package-notice sources, runtime-asset contract,
and application artwork, and compares the result byte-for-byte with the committed JSON. It deliberately does not
invoke `npm install`, package lifecycle scripts, Cargo builds, or a licence web service.

The generated record contains, for every resolved entry:

- ecosystem, exact version, direct/transitive status, graph/install scope, and selected target;
- declared licence expression and Bottie's review classification;
- selected Rust feature flags or npm optional/peer state;
- the authoritative crates.io package page or exact integrity-pinned npm registry archive; and
- input hashes, reviewed application assets, and the security-sensitive direct feature choices.

The Rust report is the union of the four reviewed graphs: 531 unique crates, including 30 direct packages, 503 entries
in a normal runtime graph, and 28 build-only entries. The macOS arm64 and x64 graphs each contain 399 crates, Windows
x64 contains 402, and Linux x64 contains 480. `Cargo.lock` contains 649 package records as a conservative superset for
architectures outside those four release targets. The npm lock contains 157 exact package paths: 14 direct packages,
eight production-install entries, and 149 development-install entries. npm's `dev` marker describes installation
scope, not whether a bundler copies code into the final frontend, so all 157 remain in the conservative notice review.

The separately deployed `website/` project has its own manifest, lockfile, build, and Node test command. It is not
linked or bundled into the Tauri desktop application and is outside this inventory. The root Vitest command now
excludes `website/` and the unrelated repository-root `assets/` workspace so each project retains its own test runner.

## Classification policy and result

- **Compatible** means a package offers a reviewed public-domain-equivalent/permissive choice that does not require a
  distributable notice in Bottie's technical policy.
- **Notice-required** means the declared permissive expression requires its copyright/licence terms, attribution, or
  upstream notices to accompany a distribution.
- **Review-required** means a human must confirm a file-level, nonstandard, platform, artwork, or model obligation
  before release.
- **Unknown** means the locked metadata contains no declared licence. Unknown entries fail the release gate.

Across 531 Rust packages, 157 npm paths, and six reviewed non-package asset groups, the result is:

| Classification  | Entries | Result                                                                 |
| --------------- | ------: | ---------------------------------------------------------------------- |
| Compatible      |      12 | No further technical gap identified by this inventory                  |
| Notice-required |     682 | Represented in the generated release notice/licence bundle             |
| Review-required |       0 | Every exact declaration and non-package runtime asset has a disposition |
| Unknown         |       0 | No missing declaration in any reviewed resolved graph or npm lock path |

No GPL, AGPL, LGPL, or SSPL declaration appears in the reviewed graphs. The five MPL-2.0 runtime-graph crates are
`cssparser 0.36.0`, `cssparser-macros 0.6.1`, `dtoa-short 0.3.5`, `option-ext 0.2.0`, and `selectors 0.36.1`.
`argparse 3.0.0` is the one Python-2.0 npm production-install entry. The exact package sources and licence texts are
included in `THIRD-PARTY-NOTICES.txt`; MPL source-form availability remains the repository's public source tree rather
than a separately modified or vendored copy.

`third-party/package-licence-texts.json` retains 679 exact locked package identities and 403 deduplicated text bodies.
The generator prefers each published package's own top-level licence, copying, copyright, and notice files. When a
workspace or platform-binary package intentionally omits the shared file, it records the authoritative package source
and uses the matching canonical text from immutable SPDX License List 3.28.0. ONNX Runtime's exact versioned licence
and complete upstream third-party notice file are appended without localization or truncation.

## Security-relevant selections

- `reqwest`: defaults off; `json`, `rustls`, and `stream` keep native HTTP on Rustls instead of the native-TLS
  default.
- `rustls`: defaults off; `ring` and `std` make the cryptographic provider explicit.
- `rusqlite`: `backup` and `bundled` compile SQLite into Bottie and enable its online-backup API.
- `fastembed`: defaults off; the Hugging Face and ONNX Runtime download paths use Rustls, with ORT selected as a
  checksum-pinned build input.
- `image`: defaults off; only the reviewed JPEG and PNG decoders are compiled.
- `zip`: defaults off; `deflate-flate2` limits DOCX and portable export archive support to DEFLATE.
- `lopdf`: defaults off, excluding its optional encryption and time features.
- `keyring`: `v1` plus crate defaults use target-native credential stores through the compatibility API.
- `objc2-local-authentication`: `LAContext` and `block2`, gated to macOS, retain the native biometric boundary.

`tauri` and `tauri-plugin-dialog` are runtime dependencies; `tauri-build` and `@tauri-apps/cli` are build tooling.
The Tauri configuration bundles only local frontend output and the named icon set. No sidecar executable, general
resource directory, downloaded font, or repository-owned dynamic library is configured.

## Native and runtime assets

### SQLite and sqlite-vec

`rusqlite 0.38.0` selects `libsqlite3-sys 0.36.0` with SQLite 3.51.1's amalgamation compiled into Bottie. SQLite's
upstream copyright page places the implementation in the public domain. `sqlite-vec 0.1.7-alpha.10` compiles its C
extension into the same Rust binary under its declared MIT/Apache-2.0 choice. Both versions and the selected Cargo
features are fixed in the generated inventory.

### ONNX Runtime

`fastembed 6.0.0` selects `ort`/`ort-sys 2.0.0-rc.13` with the Rustls binary-download feature. That build helper selects
a target-specific ONNX Runtime 1.28.0 archive from its embedded distribution table, verifies the table's SHA-256, and
links the native runtime. ONNX Runtime is MIT-licensed and publishes a separate upstream third-party notice file.
Bottie's `runtime-assets.json` records the exact ort-sys-selected archive URL and SHA-256 for macOS arm64, Windows x64,
and Linux x64. The version-matched upstream licence and complete third-party notice are committed under
`third-party/onnxruntime-1.28.0/`, hash-bound to the contract, included in `THIRD-PARTY-NOTICES.txt`, and required in
every inspected package payload.

Authoritative sources: [ONNX Runtime licence](https://github.com/microsoft/onnxruntime/blob/main/LICENSE) and
[upstream third-party notices](https://github.com/microsoft/onnxruntime/blob/main/ThirdPartyNotices.txt).

### EmbeddingGemma

Bottie does not bundle model weights in the repository or application package. Before FastEmbed loads the model,
native code fetches only revision `75a84c732f1884df76bec365346230e32f582c82`, streams SHA-256 verification over the
six exact ONNX/data/tokenizer/config files in `runtime-assets.json`, and then aliases FastEmbed's cache reference to
that verified snapshot. `MODEL-NOTICE.txt` records the reviewed 1 April 2026 Gemma terms and is bundled beside the
project licence and third-party notices.

The release operator must personally read and accept those terms. Bottie creates no acceptance evidence by default;
the exact acknowledgement command documented in `README.md` writes an ignored, identity-free, timestamp-free evidence
record bound to the model revision and model-notice hash. The release gate fails closed when that record is missing or
stale. This repository work does not accept the Gemma terms on another person's behalf.

Authoritative sources: [model repository](https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX) and
[Gemma Terms of Use](https://ai.google.dev/gemma/terms).

### Application artwork and platform frameworks

The Tauri icon set and browser favicon are repository-owned MIT-licensed bytes. Their editable sources, generation
inputs, packaged outputs, exact hashes, and redistribution statement are recorded in the inventory. macOS WebKit,
Security, LocalAuthentication, and other frameworks are supplied by the operating system and are not copied into
Bottie's current app bundle.

## Remaining release gates

The repository licence, generated notice bundle, four-target resolved-graph review, ONNX Runtime evidence, immutable
EmbeddingGemma snapshot, and terms-evidence mechanism are complete. Publication remains blocked until the release
operator explicitly accepts the reviewed Gemma terms and fresh macOS, Windows, and Linux packages prove they contain
the exact current documents. macOS must then pass the existing Developer ID/notarization/stapling/Gatekeeper contract;
Windows and Linux still require their verified distribution signatures. No signing key, acceptance record, package,
tag, upload, or release is created by the documentation and generation commands in this slice.
