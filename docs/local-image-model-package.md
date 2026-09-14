# Local image model package evidence

Last reviewed: 2026-09-14

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

## Acceptance boundary

The candidate is deliberately not selected or downloadable yet. `model_package.rs` requires native evidence tied to
the exact package revision, runtime commit, hardware profile, and bounded generation profile before it can produce a
`ModelPackageManifest` and `ModelSourcePlan`. Acceptance also requires:

- an exact worker executable size and SHA-256;
- a measured whole-process peak-memory value;
- a decoded PNG SHA-256 plus explicit visual review;
- active-step cooperative cancellation within the worker manager's three-second grace.

These fields are a fail-closed native contract, not evidence that a run happened. Bottie must obtain them from an
isolated real-runtime proof before the candidate can be enabled.

## Hugging Face delivery

Immutable Hugging Face `resolve` URLs currently return one `302` or `307` before file bytes. The downloader therefore
keeps automatic redirects disabled and treats the first response as a resolution envelope. It requires one exact
`X-Repo-Commit`, `X-Linked-ETag`, and, when present, `X-Linked-Size`, then accepts only:

- the exact revision/path under Hugging Face's relative resolve-cache route; or
- HTTPS destinations beneath `*.cdn.hf.co` or `*.xethub.hf.co`, without user info, fragments, or custom ports.

The second response cannot redirect again and must still match the exact full or ranged byte contract. Final package
promotion rechecks every file size and SHA-256. Resumable staging binds the delivery mode as well as repository root,
revision, paths, and validators, so a transport-policy change cannot reuse an old partial.

## Not performed

No model weights were downloaded, no MLX-Gen environment or worker was built, no runtime was executed, and no output,
memory, cancellation, offline-generation, or worker network-isolation claim was made. The next proof needs separate
approval because the exact candidate download is 17,442,350,812 bytes.
