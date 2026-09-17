"""Tests for clean-runtime inventory collection and lock generation."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import patch

from diffusers_clean_runtime_lock import CLEAN_PROOF_IMAGE_DIGEST
from diffusers_clean_runtime_lock_host import (
    CleanRuntimeInventoryError,
    _build_clean_runtime_lock,
    _write_manifest,
    collect_verified_inventory,
)
from test_diffusers_clean_runtime_lock import _locked_manifest


def _inventory(manifest: dict) -> dict:
    """Return exact installed identities corresponding to one test artifact manifest."""
    return {
        "schemaVersion": 1,
        "sourceImageDigest": CLEAN_PROOF_IMAGE_DIGEST,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "pythonVersion": "3.12.3",
        "pythonDistributions": [
            {"name": item["name"], "version": item["version"]}
            for item in manifest["pythonArtifacts"]
        ],
        "debianPackages": [
            {
                "name": item["name"],
                "version": item["version"],
                "architecture": item["architecture"],
            }
            for item in manifest["debianArtifacts"]
        ],
    }


class DiffusersCleanRuntimeLockHostTests(unittest.TestCase):
    """Protect image-bound inventory and deterministic offline lock creation."""

    def test_builds_and_revalidates_lock_from_exact_installed_inventory(self) -> None:
        """Artifact identities must exactly cover the independently collected image inventory."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            expected, debian_metadata = _locked_manifest(root)

            manifest = _build_clean_runtime_lock(
                _inventory(expected),
                root,
                expected_python_count=1,
                expected_debian_count=1,
                read_debian_metadata=lambda path: debian_metadata[path.resolve()],
            )

        self.assertEqual(manifest, expected)

    def test_rejects_artifact_substitution_or_incomplete_inventory(self) -> None:
        """A valid artifact cannot stand in for a different installed image component."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            expected, debian_metadata = _locked_manifest(root)
            inventory = _inventory(expected)
            inventory["pythonDistributions"][0]["version"] = "1.2.4"

            with self.assertRaisesRegex(
                CleanRuntimeInventoryError, "Python inventory does not match"
            ):
                _build_clean_runtime_lock(
                    inventory,
                    root,
                    1,
                    1,
                    lambda path: debian_metadata[path.resolve()],
                )

    def test_writes_only_outside_the_verified_artifact_tree(self) -> None:
        """Successful output cannot add a late extra file to the just-verified inputs."""
        with TemporaryDirectory() as temporary:
            parent = Path(temporary)
            artifact_root = parent / "artifacts"
            artifact_root.mkdir()
            outside = parent / "lock.json"

            with self.assertRaisesRegex(
                CleanRuntimeInventoryError, "outside the artifact tree"
            ):
                _write_manifest({}, artifact_root / "lock.json", artifact_root)

            _write_manifest({"locked": True}, outside, artifact_root)

            self.assertEqual(
                json.loads(outside.read_text(encoding="utf-8")), {"locked": True}
            )

    @patch("diffusers_clean_runtime_lock_host.subprocess.run")
    def test_collects_by_inspected_immutable_image_id_without_network(
        self, run
    ) -> None:
        """A mutable caller tag is resolved once, then the exact clean ID is executed offline."""
        inventory = {
            "schemaVersion": 1,
            "target": {"operatingSystem": "linux", "architecture": "arm64"},
            "pythonVersion": "3.12.3",
            "pythonDistributions": [
                {"name": f"package-{index:03d}", "version": "1.0.0"}
                for index in range(63)
            ],
            "debianPackages": [
                {
                    "name": f"debian-package-{index:03d}",
                    "version": "1.0-1",
                    "architecture": "arm64",
                }
                for index in range(112)
            ],
        }
        run.side_effect = [
            SimpleNamespace(
                returncode=0,
                stdout=json.dumps(
                    [
                        {
                            "Id": CLEAN_PROOF_IMAGE_DIGEST,
                            "Os": "linux",
                            "Architecture": "arm64",
                        }
                    ]
                ),
                stderr="",
            ),
            SimpleNamespace(returncode=0, stdout=json.dumps(inventory), stderr=""),
        ]

        collected = collect_verified_inventory("clean-proof:mutable-tag")

        self.assertEqual(collected["sourceImageDigest"], CLEAN_PROOF_IMAGE_DIGEST)
        command = run.call_args_list[1].args[0]
        self.assertIn(CLEAN_PROOF_IMAGE_DIGEST, command)
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--read-only", command)
        self.assertEqual(command[command.index("--cap-drop") + 1], "ALL")
        self.assertEqual(
            command[command.index("--security-opt") + 1], "no-new-privileges"
        )


if __name__ == "__main__":
    unittest.main()
