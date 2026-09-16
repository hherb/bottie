# Linux NVIDIA worker-bundle candidate evidence

Last reviewed: 2026-09-16

This document defines how to prepare review material for one Linux ARM64 Diffusers worker bundle. It does not accept
any produced bytes for Bottie, approve NVIDIA or other third-party redistribution, or make Linux image generation
available. The native catalog remains fail-closed: the Linux candidate has no accepted product profile and no
importable executable name.

## Bound identity

`local-image-worker/diffusers_bundle_candidate.py` refuses to run if any of these checked-in proof inputs drift:

- the digest-pinned `nvcr.io/nvidia/pytorch:25.11-py3` proof Dockerfile;
- the exact direct Python requirements used by the proof;
- the Diffusers worker implementation; and
- the shared private-protocol worker implementation.

Every candidate record binds those input hashes plus worker
`diffusers-0.40.0-ngc-25.11-proof-1`, runtime `diffusers@0.40.0+ngc-25.11-arm64`, model
`Qwen/Qwen-Image-2512` at revision `25468b98e3276ca6700de15c6628e51b7de54a26`, the immutable NGC base-image
digest, and the Linux ARM64 target. This binding does not prove that an inspected directory was produced correctly;
the independent distribution review must establish its provenance.

## Closed bundle measurement

The inspector accepts one already-produced bundle directory, an executable path relative to that directory, a
relative third-party metadata path inside the directory, and an output path outside the directory. It rejects an empty
tree, absolute or traversing paths, symlinks, sockets, devices, and other non-regular content.

The output contains a sorted regular-file inventory with relative path, exact byte size, SHA-256, and one ownership
identity per file. It also contains:

- the executable's relative path, byte size, and SHA-256;
- the sum of all regular-file bytes;
- a domain-separated bundle SHA-256 over each UTF-8 relative path, file length, and file bytes; and
- the included licence manifest's size and SHA-256 plus normalized component and licence-file evidence.

The bundle digest is byte-for-byte compatible with `src-tauri/src/local_image_worker/worker_bundle.rs`, so a later
accepted record can be reverified by Bottie's existing native import boundary without defining a second hash format.
The candidate record deliberately emits `distributionReviewed: false`.

Run the inspector only after producing an exact candidate on the named Linux ARM64 environment:

```sh
python3 local-image-worker/diffusers_bundle_candidate.py \
  /absolute/path/to/worker-bundle \
  bottie/bottie-local-image-diffusers-worker \
  metadata/third-party-licenses.json \
  /absolute/path/outside-the-bundle/linux-worker-candidate.json
```

The command does not build, copy, download, sign, import, or execute the bundle.

## Licence metadata contract

The included JSON metadata is closed at schema version 1:

```json
{
  "schemaVersion": 1,
  "firstPartyPathPrefixes": ["bottie", "metadata/third-party-licenses.json"],
  "thirdPartyComponents": [
    {
      "name": "exact component name",
      "version": "exact version or immutable revision",
      "source": "https://authoritative.example/component",
      "licenseExpression": "SPDX expression or reviewed LicenseRef",
      "pathPrefixes": ["runtime/component", "licenses/component.txt"],
      "licenseFiles": ["licenses/component.txt"]
    }
  ]
}
```

Only the fixed `bottie` subtree and the licence-manifest file itself may be first-party. Every other bundle file must
match exactly one third-party owner. Each third-party component must cover at least one file and include at least one
real licence/notice file that it owns. Empty versions or sources, placeholder licence expressions, missing licence
bytes, unclassified files, relabelled runtime files, and ambiguous ownership fail closed.

Structural completeness is not a legal conclusion. The reviewed record still needs an independent comparison against
the produced environment's complete installed-package and native-library inventory, upstream licence and notice
requirements, NVIDIA container/product terms, source-offer obligations, patent/trademark restrictions, and Bottie's
intended distribution form. A valid record must not be edited to set `distributionReviewed` to true; acceptance belongs
in a later reviewed native contract tied to the exact produced executable and bundle digests.

## Product boundary

Until exact produced bytes and their complete licence record pass independent review, do not add a Linux executable
name or accepted profile to the native catalog. Do not import the candidate into Bottie's cache, add a download source,
probe Linux hardware, launch it from the application, expose Linux availability through IPC or UI, or claim support.
Docker remains proof infrastructure rather than an application dependency.
