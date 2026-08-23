# Bottie dependency and licence review

Reviewed: 2026-08-24

This is a technical inventory, not legal advice. It records the exact locked package metadata and non-package runtime
assets selected by the current Bottie source tree so release work has an explicit, reproducible starting point.

## Reproduce the inventory

`dependency-inventory.json` is generated from the two lockfiles without network access or package build scripts:

```sh
node scripts/dependency-inventory.mjs
npm run dependencies:check
```

The check runs `cargo tree --locked --offline` for the macOS arm64 and x64 normal/build graphs, reads npm's version-3
lockfile directly, hashes the reviewed manifests and application artwork, and compares the result byte-for-byte with
the committed JSON. It deliberately does not invoke `npm install`, package lifecycle scripts, Cargo builds, or a
licence web service.

The generated record contains, for every resolved entry:

- ecosystem, exact version, direct/transitive status, graph/install scope, and selected target;
- declared licence expression and Bottie's review classification;
- selected Rust feature flags or npm optional/peer state;
- the authoritative crates.io package page or exact integrity-pinned npm registry archive; and
- input hashes, reviewed application assets, and the security-sensitive direct feature choices.

The Rust report is the union of the currently buildable macOS arm64 and x64 graphs: 399 unique crates, including 29
direct packages, 376 entries in a normal runtime graph, and 23 build-only entries. `Cargo.lock` contains 649 package
records as a conservative all-platform superset. The npm lock contains 157 exact package paths: 14 direct packages,
eight production-install entries, and 149 development-install entries. npm's `dev` marker describes installation
scope, not whether a bundler copies code into the final frontend, so all 157 remain in the conservative notice review.
Windows and Linux graph/artefact verification remains a release task; this review does not fetch their currently
uncached target packages.

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

Across 399 Rust packages, 157 npm paths, and six reviewed non-package asset groups, the result is:

| Classification  | Entries | Result                                                                 |
| --------------- | ------: | ---------------------------------------------------------------------- |
| Compatible      |      10 | No further technical gap identified by this inventory                  |
| Notice-required |     543 | Must be represented in the release notice/licence bundle               |
| Review-required |       9 | Five MPL crates, one Python-2.0 npm package, and three asset groups     |
| Unknown         |       0 | No missing declaration in either resolved macOS graph or npm lockfile  |

No GPL, AGPL, LGPL, or SSPL declaration appears in the reviewed graphs. The five MPL-2.0 runtime-graph crates are
`cssparser 0.36.0`, `cssparser-macros 0.6.1`, `dtoa-short 0.3.5`, `option-ext 0.2.0`, and `selectors 0.36.1`.
`argparse 3.0.0` is the one Python-2.0 npm production-install entry. These declarations are not automatically treated as
incompatible, but release counsel/review must confirm the applicable notice and source-form obligations.

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
Bottie does not yet capture the final archive identity, extracted licence files, or packaged native artefact in a
release manifest, so this asset stays review-required.

Authoritative sources: [ONNX Runtime licence](https://github.com/microsoft/onnxruntime/blob/main/LICENSE) and
[upstream third-party notices](https://github.com/microsoft/onnxruntime/blob/main/ThirdPartyNotices.txt).

### EmbeddingGemma

Bottie does not bundle model weights in the repository or current executable. When semantic indexing first needs the
model, FastEmbed downloads `onnx-community/embeddinggemma-300m-ONNX` Q4 files into Bottie's application-owned cache.
The current API follows the repository's main revision; Bottie does not pin a commit or record the ONNX/data/tokenizer
hashes. The model card declares the Gemma licence, whose terms include use restrictions and distribution notice
requirements. Release work must therefore pin and verify the complete model snapshot, present/record the applicable
terms before first download, and decide whether Bottie or the user is the distributor.

Authoritative sources: [model repository](https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX) and
[Gemma Terms of Use](https://ai.google.dev/gemma/terms).

### Application artwork and platform frameworks

The Tauri icon set and browser favicon are repository-owned bytes and their exact hashes are in the inventory, but no
source/provenance or grant is recorded. They remain review-required. macOS WebKit, Security, LocalAuthentication, and
other frameworks are supplied by the operating system and are not copied into Bottie's current app bundle.

## Release-blocking gaps

1. The repository has no root `LICENSE`/`LICENCE` file. `package.json` says MIT while Bottie's Rust package declares no
   licence. The copyright holder and exact project licence text must be recorded before distribution.
2. Bottie has no generated distributable notice/licence bundle. Release packaging must include the exact selected
   package texts, required copyright notices, MPL handling, and ONNX Runtime's matching third-party notices.
3. The packaged ONNX Runtime platform archive and its extracted notices must be captured and verified for every
   release target; a lockfile entry for the Rust wrapper alone is insufficient.
4. The EmbeddingGemma revision/files are not pinned and the Gemma terms are not presented or accepted through a
   release-ready product flow.
5. The application icon/favicon provenance and redistribution rights are not recorded.
6. Windows and Linux resolved graphs, native assets, system dependencies, and final bundle contents remain unverified.

These are release gates, not reasons to change dependencies in this review slice. Dependency upgrades, replacement,
vendoring, model-cache behavior, user-facing licence acceptance, packaging, signing, and updates remain separate work.
