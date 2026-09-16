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

## Environment review gate

`local-image-worker/diffusers_bundle_environment.py` inventories every Python distribution and Debian package in the
retained proof environment before any bundle assembly. Its host-side entry point asks the Docker daemon to inspect the
derived-image reference, requires the exact immutable image ID plus Linux ARM64 metadata, and launches collection by
that verified ID with networking disabled. The in-container collector cannot add image identity to its unbound output.
It separately requires Python 3.12.3, every direct Python pin, and every checked-in proof input before returning the
package-manager inventory to the host, which then binds the exact base- and derived-image digests. The record contains
only component identities, versions, architectures, package-relative licence labels, exact licence byte counts and
hashes, and the measured NVIDIA container-terms bytes. Absolute paths and licence contents are not retained.

Run the gate from the Docker host, naming the derived image reference that must resolve to the retained immutable ID:

```sh
python3 local-image-worker/diffusers_bundle_environment.py \
  sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c \
  /absolute/path/outside-the-repository/linux-environment-review.json
```

Caller-provided environment variables are not image evidence and are ignored. A missing Docker inspection, a tag that
resolves to another image, target drift, runtime drift, or checked-in proof-input drift fails closed.

The named DGX Spark produced a record covering 248 Python and 434 Debian components. Two independent runs were
byte-identical; the exact byte count and SHA-256 are retained in the checked-in summary.
[`local-image-linux-environment-review.json`](local-image-linux-environment-review.json) retains the bounded path-free
summary and all exact blockers.

The gate deliberately exited with status 3 and `assemblyEligible: false`: 76 blockers remain, including twelve
components without measured package-owned licence bytes and additional Python distributions without a non-placeholder
licence declaration. This package-manager baseline is conservative; it does not classify unmanaged native files or
claim every installed Jupyter, development, or NVIDIA tool belongs in Bottie's runtime. A later procedure must first
derive the exact Python and native dependency closure of the worker, then require real authoritative licence bytes and
reviewed expressions for every included component. It must not discard a component merely to hide a blocker. No bundle
candidate was assembled from this review.

## Runtime closure gate

`local-image-worker/prove_diffusers_worker.py` can opt into a proof-only audit trace without changing the worker
protocol. The trace records imported Python module files and dynamically loaded libraries, then snapshots the worker
process maps after deterministic cold and warm generation. It writes the trace context only after generation,
cancellation, and clean shutdown pass. The proof container remains read-only, non-root, capability-free, and offline;
model weights, generated output, temporary files, pseudo-filesystems, and the tracing helper are excluded from closure
bytes.

`local-image-worker/diffusers_runtime_closure_host.py` resolves the requested derived image through the host Docker
daemon and runs the classifier by exact image ID read-only, non-root, capability-free, with networking disabled. It
requests NVIDIA runtime injection so the classifier can measure the proof-time host-driver files named by the
process-map evidence. Those files are accepted only when their declared ELF SONAME is on the closed driver boundary.
The classifier measures all other observed regular files, maps them to Python, Debian, or exact marker-backed native
ownership, recursively closes active `Requires-Dist` dependencies without optional extras, and resolves each ELF
`NEEDED` name. Duplicate extension-module basenames block only when an actual `NEEDED` edge requests the ambiguous
name. The output retains byte hashes, sizes, owners, component identities, and path-free marker provenance but no
filesystem paths.

Run the gate on the same Docker host as the traced proof:

```sh
python3 local-image-worker/diffusers_runtime_closure_host.py \
  sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c \
  /absolute/path/outside-the-repository/runtime-trace \
  /absolute/path/outside-the-repository/linux-runtime-closure-review.json
```

The named DGX Spark trace binds Python audit bytes at
`6543aa3fb8b87cf678c511358fab42d5f397c61c4a5b3b431ed5fabed5af1b87` and process-map bytes at
`f6bd927a91e67af3ba896c9144f93fd04e8cfef8c45a4ea97c72c2e0053d7c40`. Two independent GPU-injected collections
were byte-identical. The host-bound record is 30,671 bytes with SHA-256
`8a3d8cdf7c4d35f70740bc9391c3747a39b94a3898dab21f3d339f4fc55e6024`.
[`local-image-linux-runtime-closure-review.json`](local-image-linux-runtime-closure-review.json) retains the bounded,
path-free summary.

The closure contains 340 files totalling 6,133,717,252 bytes, including 295 ELF files and 84 components: 51 Python
distributions, 27 Debian packages, and six marker-backed native components. It observes `libcuda.so.1`,
`libnvidia-ml.so.1`, and `libnvidia-ptxjitcompiler.so.1` on the explicit host-driver boundary. All fourteen previously
unowned file hashes now have exact component identities, and the three unrequested extension-module basename
collisions no longer masquerade as linker conflicts. `closureComplete` is true.

The record remains deliberately ineligible for assembly: eight components lack authoritative licence bytes, seventeen
declare no usable licence, and all 84 expressions still need independent review. A bounded read-only search of the
exact image found package-owned NVIDIA SDK licence bytes for the cuSPARSELt Debian package, but no matching in-image
licence files for the HPC-X Open MPI/UCC/UCX or NVPL BLAS/LAPACK components. That discovery is not a licence review or
redistribution approval. The gate exits with status 3, with `licenseReviewed`, `assemblyEligible`, and
`distributionReviewed` false. It does not copy, assemble, import, or execute a product bundle.

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
