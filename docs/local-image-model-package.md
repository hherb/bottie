# Local image model package evidence

Last reviewed: 2026-09-15

## Candidate

Bottie's first reviewed Apple-silicon candidate is the mixed q4/q8 MLX-Gen package
`AbstractFramework/qwen-image-2512-4bit`, derived from the distinct open-weight
`Qwen/Qwen-Image-2512` text-to-image checkpoint. It is not Qwen-Image-2.0.

- Package revision: `423f1f5bf708c6e11eb78881ef9738422cea0814`
- Package files: 18
- Exact repository-file total: 17,442,350,812 bytes
- License: Apache-2.0, with `LICENSE.md` included in the exact manifest
- Declared minimum/generated runtime: MLX-Gen 0.18.2
- Pinned MLX-Gen commit: `fca64a283737c68b67a7bfd88d93f7aa9101a95c`
- Target evidence profile: Apple M3 Max with 128 GB unified memory, 512x512, 15 steps

The repository-file total is the sum of all 18 immutable sibling sizes. Hugging Face's separate `usedStorage` value is
not used because it differs from the bytes the downloader must actually receive and verify.

Reviewed upstream material:

- [immutable package revision](https://huggingface.co/AbstractFramework/qwen-image-2512-4bit/tree/423f1f5bf708c6e11eb78881ef9738422cea0814)
- [MLX-Gen v0.18.2 commit](https://github.com/lpalbou/mlx-gen/commit/fca64a283737c68b67a7bfd88d93f7aa9101a95c)
- [MLX-Gen package and quantization guidance](https://github.com/lpalbou/mlx-gen/blob/main/docs/quantization.md)

## Accepted Apple-silicon proof

The candidate passed its bounded proof and is now the selected first package for the exact
`apple-m3-max-128gb` evidence profile. Selection does not make the product route available on unprobed hardware.

- Decoded RGB pixel SHA-256: `4cd2921c3cf0a43f791cd725cf72da1ff0be04fe97883a9a4b32332cc9cfc0a5`
- Worker executable: 56,008,496 bytes, SHA-256
  `187bfc58b30328278e52500c2b28999f2ff56cf510dd0790a3e90100aa81464b`
- Canonical worker bundle: 1,107,880,778 bytes, SHA-256
  `7a5db3e6c1c59c5d9264d8fed1f40f9a160bc8c4d64c642464588504856f0bf1`
- Whole-process lifetime peak physical footprint: 29,526,129,448 bytes
- Active denoising-boundary cooperative cancellation: 110 ms
- Operating-system network-denial probe: passed
- Decoded 512x512 RGB PNG visual review: passed

The worker was built from the pinned MLX-Gen checkout and frozen `uv.lock` with CPython 3.13.14, PyInstaller 6.16.0,
and `pyinstaller-hooks-contrib` 2026.7. PyInstaller relocates `libmlx.dylib` to its onedir root, so the build explicitly
places the exact `mlx.metallib` beside that relocated library; omitting that data placement fails closed at model load.

`model_package.rs` requires evidence tied to the exact package revision, runtime commit, hardware profile, bounded
generation profile, and network sandbox before it can produce a `ModelPackageManifest` and `ModelSourcePlan`.
Acceptance also requires:

- an exact worker executable size and SHA-256;
- a measured whole-process peak-memory value;
- a decoded RGB pixel SHA-256 plus explicit visual review;
- active-step cooperative cancellation within the worker manager's three-second grace.

These fields remain a fail-closed native contract. The selected evidence is frozen in code so later runtime integration
can re-hash the installed worker bundle before use.

## Hugging Face delivery

Immutable Hugging Face `resolve` URLs currently return one `302` or `307` before file bytes. The downloader therefore
keeps automatic redirects disabled and treats the first response as a resolution envelope. It requires one exact
`X-Repo-Commit`, `X-Linked-ETag`, and, when present, `X-Linked-Size`, then accepts only:

- the exact revision/path under Hugging Face's relative resolve-cache route; or
- HTTPS destinations beneath `*.cdn.hf.co` or `*.xethub.hf.co`, without user info, fragments, or custom ports.

Nested Hugging Face cache paths are accepted only in the resolver's exact percent-encoded single-segment form. The
second response cannot redirect again and must still match the exact full or ranged byte contract. Final package
promotion rechecks every file size and SHA-256. Resumable staging binds the delivery mode as well as repository root,
revision, paths, and validators, so a transport-policy change cannot reuse an old partial. A response-envelope or range
failure cannot append bytes and therefore retains an already verified prefix; source drift, unsafe paths, and integrity
failures still discard it. The live proof resumed after interruption and crossed the previously failing nested path.

## Remaining boundary

The proof cache, worker build, and PNG remain ignored local evidence and are not application payloads. No automatic
worker download, general hardware availability, cloud fallback, signing, release, or distribution work was performed.
The native availability contract reads macOS physical memory, checks the compile-target architecture, accepts only the
exact Apple M3 Max 128 GiB evidence profile, re-hashes the installed executable and complete worker bundle, and
re-verifies promoted model bytes without mutating an absent cache. The application now resolves fixed app-owned worker
and model-cache locations, serializes that work off the WebView task, and presents only exact package disclosure plus
one
coarse path-free state. The composer now keeps Cloud as its default and enables an explicit Local 2512 choice only for
that exact ready state. Each local start and retry re-verifies the installation, launches the selected executable under
the accepted macOS network-denied sandbox profile, retains one warm model, and maps one 512x512 output through shared
PNG validation, durable storage, provenance, cancellation, retry, export, and deletion. There is no Cloud fallback.
When the exact worker and hardware gates pass but the model is absent or mismatched, a Rust-owned single-slot
coordinator presents every selected package fact before one explicit install or resume action. Startup performs only
read-only source-bound staging inspection. Approved acquisition uses the existing strict Hugging Face source plan,
retains only exact synced partials on cancellation or interruption, verifies all 18 files, promotes atomically, and then
refreshes readiness; paths, hashes, source URLs, and response details stay native. The proof worker remains ignored
local
evidence rather than a default application payload. Bottie now lets the user select an already-built exact worker folder
through a native picker, validates it before creating cache state, copies only regular symlink-free content into
app-owned staging, re-hashes the staged executable and canonical bundle, and atomically promotes the result. Readiness
and execution resolve only that promoted cache identity. The selected path, bundle hashes, and filesystem failures
remain native; the WebView receives exact runtime/size disclosure and fixed path-free outcomes. An automatic worker
download remains unavailable because no archive source and digest are accepted.
