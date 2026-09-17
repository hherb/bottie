# Linux NVIDIA worker-bundle candidate evidence

Last reviewed: 2026-09-17

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
were byte-identical. After binding three exact in-image licence sources, two fresh collections were also byte-identical.
The current host-bound record is 31,685 bytes with SHA-256
`fc8ed1d465640bd1c5c13dda5c6b5850207d0baa638f070e31c19203c924f7c9`.
[`local-image-linux-runtime-closure-review.json`](local-image-linux-runtime-closure-review.json) retains the bounded,
path-free summary.

The closure contains 340 files totalling 6,133,717,252 bytes, including 295 ELF files and 84 components: 51 Python
distributions, 27 Debian packages, and six marker-backed native components. It observes `libcuda.so.1`,
`libnvidia-ml.so.1`, and `libnvidia-ptxjitcompiler.so.1` on the explicit host-driver boundary. All fourteen previously
unowned file hashes now have exact component identities, and the three unrequested extension-module basename
collisions no longer masquerade as linker conflicts. `closureComplete` is true.

The record remains deliberately ineligible for assembly: five components lack authoritative licence bytes, seventeen
declare no usable licence, and all 84 expressions still need independent review. The exact image binds cuSPARSELt to
the measured `libcusparselt0-cuda-13@0.8.1.1-1` Debian copyright record. It also binds Open MPI and UCX to exact licence
members in the source archives shipped inside the same HPC-X installation. These measurements are not a licence review
or redistribution approval. The gate exits with status 3, with `licenseReviewed`, `assemblyEligible`, and
`distributionReviewed` false. It does not copy, assemble, import, or execute a product bundle.

The host gate also accepts an optional `--license-sources` directory. The directory is mounted read-only into the exact
image while Docker networking remains disabled. Only four fixed filenames are recognized, and every archive must match
its exact authoritative byte count and SHA-256 before one exact regular licence member is measured. The NVIDIA sources
add a second binding: both installed NVPL runtime files must be byte-identical to their corresponding regular archive
members. This prevents the archive's build suffix from being associated with the marker-only component by version-name
inference.

| Closure identity                  | Authoritative archive                                  | Archive SHA-256                                                    | Licence member evidence                                                          |
| --------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| `native:nvidia-nvpl-blas@0.2.0`   | NVIDIA `nvpl_blas-linux-sbsa-0.2.0.1-archive.tar.xz`   | `ba29f6a9d3831b6ae5c9265b4d124c13b9b9e0faea025359b02b41ad230975c2` | 19,072 bytes; `d81174652f0c448a5736afc5d50663606863bfd6ee8c8416fbd9a628c6f8802f` |
| `native:nvidia-nvpl-lapack@0.2.2` | NVIDIA `nvpl_lapack-linux-sbsa-0.2.2.1-archive.tar.xz` | `cdfbf69517a044e99e3e6231c8b2f4e845fd0de57775ccad6b4b0b4fe7e91e84` | 19,072 bytes; `d81174652f0c448a5736afc5d50663606863bfd6ee8c8416fbd9a628c6f8802f` |
| `python:sentencepiece@0.2.2`      | PyPI `sentencepiece-0.2.2.tar.gz`                      | `3d2b5e824b5622038dc7b490897efe05ebbbb9e7350fc142f3ecc8789ef9bdf6` | 11,358 bytes; `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| `python:tokenizers@0.23.2`        | PyPI `tokenizers-0.23.2.tar.gz`                        | `7f0f085686b9de0d0079e6f874ae053600db64c5d13049e0bbc0119926d25aac` | 11,357 bytes; `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4` |

Place only already-obtained exact artifacts in an absolute directory outside the repository, then run the same retained
image and trace with that directory:

```sh
python3 local-image-worker/diffusers_runtime_closure_host.py \
  sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c \
  /absolute/path/outside-the-repository/runtime-trace \
  /absolute/path/outside-the-repository/linux-runtime-closure-review.json \
  --license-sources /absolute/path/outside-the-repository/license-sources
```

The source gate does not download artifacts, retain their contents, infer a licence expression, or mark a licence as
reviewed. Supplying only a recognized subset remains partial; supplying a directory with no recognized archive fails
closed. The exact UCC revision
`native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e` deliberately has no catalog entry because
neither the retained image nor the authoritative upstream commit endpoint supplied source bytes for that exact
revision. A nearby UCC 1.5.0 release must not substitute for it.

The closure gate now accepts an optional `--license-review` manifest. The manifest is bound to the immutable derived
image, both exact proof-trace digests, and the complete sorted set of closure components. Every component must provide a
non-placeholder reviewed expression, at least one licence or notice file, and a separate review record. Both the source
files and review record are embedded as Base64 bytes with independently checked byte counts and SHA-256 hashes. The
collector emits only their path-free measurements; malformed Base64, byte drift, missing/extra identities, duplicate or
unsorted records, absolute/traversing names, partial coverage, and image or trace drift fail closed.

Run the review only after authoritative source bytes and an independent record exist for all 84 exact components:

```sh
python3 local-image-worker/diffusers_runtime_closure_host.py \
  sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c \
  /absolute/path/outside-the-repository/runtime-trace \
  /absolute/path/outside-the-repository/linux-runtime-closure-review.json \
  --license-review /absolute/path/outside-the-repository/linux-runtime-license-review.json
```

The closed schema is version 1. `components` must be complete and sorted by UTF-8 identity; `licenseFiles` must be
non-empty and sorted by portable relative name. `contentsBase64` carries the exact bytes whose adjacent measurement is
checked before the contents are discarded from the path-free closure output:

```json
{
  "schemaVersion": 1,
  "derivedImageDigest": "sha256:<64 lowercase hex characters>",
  "trace": {
    "pythonTraceSha256": "<64 lowercase hex characters>",
    "processMapsSha256": "<64 lowercase hex characters>"
  },
  "components": [
    {
      "identity": "python:exact-name@exact-version",
      "reviewedLicenseExpression": "reviewed SPDX expression or LicenseRef",
      "licenseFiles": [
        {
          "relativeName": "component/LICENSE",
          "byteSize": 123,
          "sha256": "<64 lowercase hex characters>",
          "contentsBase64": "<Base64 of the exact 123 bytes>"
        }
      ],
      "reviewEvidence": {
        "byteSize": 456,
        "sha256": "<64 lowercase hex characters>",
        "contentsBase64": "<Base64 of the exact 456-byte independent review record>"
      }
    }
  ]
}
```

On 2026-09-17, a fresh offline read-only inspection reconfirmed that the installed `sentencepiece` and `tokenizers`
wheels contain no licence/notice document. The exact HPC-X installation includes Open MPI and UCX source archives with
measured top-level licence members, but no UCC source archive or matching licence document. No matching NVPL BLAS or
LAPACK document was found inside the image. Separately obtained PyPI and NVIDIA source artifacts passed the new
path-free archive/member validation locally. The NVPL runtime-member verifier was exercised against files extracted
from those official archives, but the retained DGX image was unavailable on this host, so its installed runtime match
was not confirmed and the checked-in closure summary was not regenerated. It still records 106 blockers. No review
manifest was created, no expression was inferred, and no product eligibility changed.

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
