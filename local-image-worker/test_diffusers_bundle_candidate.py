"""Contract tests for proof-only Linux Diffusers worker-bundle evidence."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from diffusers_bundle_candidate import (
    BASE_IMAGE_DIGEST,
    CandidateEvidenceError,
    build_candidate_evidence,
)
from diffusers_worker import DIFFUSERS_WORKER_IDENTITY


EXECUTABLE_PATH = "bottie/bottie-local-image-diffusers-worker"
LICENSE_MANIFEST_PATH = "metadata/third-party-licenses.json"


class DiffusersBundleCandidateTests(unittest.TestCase):
    """Exercise deterministic inventory, identity binding, and licence completeness."""

    def test_candidate_evidence_is_deterministic_and_bound_to_exact_runtime(self) -> None:
        """Creation order cannot affect the closed evidence record or canonical bundle digest."""
        with tempfile.TemporaryDirectory() as left_temporary, tempfile.TemporaryDirectory() as right_temporary:
            left = Path(left_temporary)
            right = Path(right_temporary)
            write_candidate(left, reverse=False)
            write_candidate(right, reverse=True)

            left_evidence = build_candidate_evidence(left, EXECUTABLE_PATH, LICENSE_MANIFEST_PATH)
            right_evidence = build_candidate_evidence(right, EXECUTABLE_PATH, LICENSE_MANIFEST_PATH)
            expected_digest = expected_bundle_digest(left, left_evidence)

        self.assertEqual(left_evidence, right_evidence)
        self.assertEqual(left_evidence["workerVersion"], DIFFUSERS_WORKER_IDENTITY.worker_version)
        self.assertEqual(left_evidence["runtimeId"], DIFFUSERS_WORKER_IDENTITY.runtime_id)
        self.assertEqual(left_evidence["modelId"], DIFFUSERS_WORKER_IDENTITY.model_id)
        self.assertEqual(left_evidence["modelRevision"], DIFFUSERS_WORKER_IDENTITY.model_revision)
        self.assertEqual(left_evidence["baseImageDigest"], BASE_IMAGE_DIGEST)
        self.assertEqual(
            [entry["path"] for entry in left_evidence["proofInputs"]],
            [
                "local-image-worker/Dockerfile.diffusers-proof",
                "local-image-worker/requirements-diffusers-proof.txt",
                "local-image-worker/diffusers_worker.py",
                "local-image-worker/mlx_worker.py",
            ],
        )
        self.assertEqual(left_evidence["target"], {"operatingSystem": "linux", "architecture": "aarch64"})
        self.assertFalse(left_evidence["distributionReviewed"])
        self.assertEqual(
            [entry["path"] for entry in left_evidence["bundle"]["files"]],
            sorted(entry["path"] for entry in left_evidence["bundle"]["files"]),
        )
        self.assertEqual(left_evidence["bundle"]["sha256"], expected_digest)
        self.assertEqual(
            left_evidence["bundle"]["sha256"],
            "ec5c805f95ca73de1185ec619689ce11dae09bda567b8831274528ce47347fbc",
        )
        self.assertEqual(left_evidence["executable"]["path"], EXECUTABLE_PATH)
        self.assertEqual(left_evidence["executable"]["sha256"], hashlib.sha256(b"worker-bytes").hexdigest())
        self.assertEqual(left_evidence["licenseInventory"]["componentCount"], 1)
        self.assertTrue(left_evidence["licenseInventory"]["complete"])

    def test_unclassified_file_rejects_incomplete_third_party_metadata(self) -> None:
        """Every regular file must be assigned to first-party code or one reviewed component."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_candidate(root)
            write_file(root, "unclassified/native.so", b"native")

            with self.assertRaisesRegex(CandidateEvidenceError, "unclassified bundle file"):
                build_candidate_evidence(root, EXECUTABLE_PATH, LICENSE_MANIFEST_PATH)

    def test_missing_or_placeholder_license_metadata_fails_closed(self) -> None:
        """Third-party components require exact version, source, expression, and included licence bytes."""
        for field, value in (
            ("version", ""),
            ("source", ""),
            ("source", "https://user:secret@example.invalid/dependency"),
            ("licenseExpression", "NOASSERTION"),
            ("licenseFiles", ["licenses/missing.txt"]),
            ("licenseFiles", ["licenses/dependency.txt", "licenses/dependency.txt"]),
        ):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                write_candidate(root)
                manifest_path = root / LICENSE_MANIFEST_PATH
                manifest = json.loads(manifest_path.read_text())
                manifest["thirdPartyComponents"][0][field] = value
                manifest_path.write_text(json.dumps(manifest))

                with self.assertRaises(CandidateEvidenceError):
                    build_candidate_evidence(root, EXECUTABLE_PATH, LICENSE_MANIFEST_PATH)

    def test_metadata_cannot_relabel_runtime_files_as_first_party(self) -> None:
        """Only the fixed Bottie subtree and licence manifest may bypass third-party ownership."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_candidate(root)
            manifest_path = root / LICENSE_MANIFEST_PATH
            manifest = json.loads(manifest_path.read_text())
            manifest["firstPartyPathPrefixes"].append("runtime")
            manifest["thirdPartyComponents"][0]["pathPrefixes"] = ["licenses/dependency.txt"]
            manifest_path.write_text(json.dumps(manifest))

            with self.assertRaisesRegex(CandidateEvidenceError, "first-party ownership"):
                build_candidate_evidence(root, EXECUTABLE_PATH, LICENSE_MANIFEST_PATH)

    def test_symlinks_and_escaping_inputs_are_rejected(self) -> None:
        """The proof inventory cannot hide links or read an executable outside its declared bundle."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_candidate(root)
            (root / "runtime" / "linked-license").symlink_to(root / "licenses" / "dependency.txt")
            with self.assertRaisesRegex(CandidateEvidenceError, "regular files"):
                build_candidate_evidence(root, EXECUTABLE_PATH, LICENSE_MANIFEST_PATH)

            with self.assertRaises(CandidateEvidenceError):
                build_candidate_evidence(root, "../worker", LICENSE_MANIFEST_PATH)


def write_candidate(root: Path, reverse: bool = False) -> None:
    """Create one small complete bundle fixture, optionally reversing creation order."""
    files = [
        (EXECUTABLE_PATH, b"worker-bytes"),
        ("runtime/dependency.py", b"dependency-bytes"),
        ("licenses/dependency.txt", b"MIT licence text"),
    ]
    if reverse:
        files.reverse()
    for relative_path, contents in files:
        write_file(root, relative_path, contents)
    manifest = {
        "schemaVersion": 1,
        "firstPartyPathPrefixes": ["bottie", LICENSE_MANIFEST_PATH],
        "thirdPartyComponents": [
            {
                "name": "dependency",
                "version": "1.0.0",
                "source": "https://example.invalid/dependency-1.0.0",
                "licenseExpression": "MIT",
                "pathPrefixes": ["runtime", "licenses/dependency.txt"],
                "licenseFiles": ["licenses/dependency.txt"],
            }
        ],
    }
    write_file(root, LICENSE_MANIFEST_PATH, json.dumps(manifest).encode())


def write_file(root: Path, relative_path: str, contents: bytes) -> None:
    """Write one fixture file below the temporary bundle root."""
    path = root / relative_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(contents)


def expected_bundle_digest(root: Path, evidence: dict) -> str:
    """Recompute the documented Rust-compatible digest from emitted inventory facts."""
    hasher = hashlib.sha256(b"bottie-local-image-worker-bundle-v1")
    for entry in evidence["bundle"]["files"]:
        path = entry["path"].encode()
        hasher.update(len(path).to_bytes(8, "big"))
        hasher.update(path)
        hasher.update(entry["byteSize"].to_bytes(8, "big"))
        hasher.update((root / entry["path"]).read_bytes())
    return hasher.hexdigest()


if __name__ == "__main__":
    unittest.main()
