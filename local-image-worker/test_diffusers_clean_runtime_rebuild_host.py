"""Tests for host orchestration of two offline clean-runtime rebuilds."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import patch

from diffusers_clean_runtime_lock import CLEAN_BASE_IMAGE_DIGEST
from diffusers_clean_runtime_rebuild import REVIEWED_WORKER_SOURCES
from diffusers_clean_runtime_rebuild_host import (
    CleanRuntimeRebuildError,
    _collect_inventory_command,
    _expected_inventory,
    _inspect_image,
    _stage_context,
    _validate_rebuilt_inventory,
    run_rebuilds,
)


class DiffusersCleanRuntimeRebuildHostTests(unittest.TestCase):
    """Protect exact host commands and source staging around BuildKit."""

    def test_stages_only_reviewed_worker_sources_and_recipe(self) -> None:
        """No repository file outside the closed allowlist enters the build context."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            context = root / "context"
            source.mkdir()
            for filename in REVIEWED_WORKER_SOURCES:
                (source / filename).write_text(filename, encoding="utf-8")
            (source / "unreviewed.py").write_text("no", encoding="utf-8")

            _stage_context(source, context, REVIEWED_WORKER_SOURCES)

            self.assertEqual(
                sorted(path.name for path in context.iterdir()),
                ["Dockerfile", *REVIEWED_WORKER_SOURCES],
            )
            self.assertIn(
                f"ubuntu@{CLEAN_BASE_IMAGE_DIGEST}",
                (context / "Dockerfile").read_text(),
            )

    def test_collects_inventory_from_immutable_image_without_network(self) -> None:
        """Rebuilt inventory collection is read-only and cannot contact package indexes."""
        command = _collect_inventory_command(
            "sha256:" + "a" * 64, Path("/absolute/inspection")
        )

        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--read-only", command)
        self.assertEqual(command[command.index("--cap-drop") + 1], "ALL")
        self.assertEqual(
            command[command.index("--security-opt") + 1], "no-new-privileges"
        )
        self.assertEqual(command[-1], "--collect-unbound")

    @patch("diffusers_clean_runtime_rebuild_host.subprocess.run")
    def test_inspects_exact_linux_arm64_image(self, run) -> None:
        """A caller name resolves once to one immutable Linux ARM64 image ID."""
        image_id = "sha256:" + "a" * 64
        run.return_value = SimpleNamespace(
            returncode=0,
            stdout=json.dumps(
                [{"Id": image_id, "Os": "linux", "Architecture": "arm64"}]
            ),
            stderr="",
        )

        self.assertEqual(_inspect_image("rebuild-a"), image_id)
        inspected = json.loads(run.return_value.stdout)
        inspected[0]["Architecture"] = "amd64"
        run.return_value.stdout = json.dumps(inspected)
        with self.assertRaisesRegex(CleanRuntimeRebuildError, "target"):
            _inspect_image("rebuild-a")

    def test_rebuilt_inventory_must_equal_the_frozen_lock(self) -> None:
        """Agreement between two builds is insufficient if both drift from the input lock."""
        lock = {
            "pythonArtifacts": [{"name": "example", "version": "1.0", "filename": "x"}],
            "debianArtifacts": [
                {
                    "name": "base",
                    "version": "1",
                    "architecture": "arm64",
                    "filename": "x",
                }
            ],
        }
        expected = _expected_inventory(lock)

        _validate_rebuilt_inventory(expected, lock)
        expected["pythonDistributions"][0]["version"] = "2.0"
        with self.assertRaisesRegex(CleanRuntimeRebuildError, "frozen input lock"):
            _validate_rebuilt_inventory(expected, lock)

    def test_reverifies_frozen_inputs_between_and_after_builds(self) -> None:
        """A concurrent archive mutation cannot escape the initial validation pass."""
        verification = {"lockSha256": "a" * 64, "totalArtifactByteSize": 10}
        result = {
            "buildName": "unused",
            "imageId": "sha256:" + "b" * 64,
            "inventory": {},
            "regularFiles": [],
        }
        with (
            patch(
                "diffusers_clean_runtime_rebuild_host.validate_clean_runtime_lock",
                side_effect=[verification, verification, verification],
            ) as verify,
            patch(
                "diffusers_clean_runtime_rebuild_host.validate_rebuild_plan",
                return_value=REVIEWED_WORKER_SOURCES,
            ),
            patch("diffusers_clean_runtime_rebuild_host._stage_context"),
            patch("diffusers_clean_runtime_rebuild_host._stage_inspection"),
            patch(
                "diffusers_clean_runtime_rebuild_host._run_build",
                side_effect=[result, result],
            ),
            patch(
                "diffusers_clean_runtime_rebuild_host._compare_rebuilds",
                return_value={},
            ),
        ):
            run_rebuilds(
                {},
                {},
                Path("/absolute/inputs"),
                Path("/absolute/source"),
                "rebuild-a",
                "rebuild-b",
            )

        self.assertEqual(verify.call_count, 3)


if __name__ == "__main__":
    unittest.main()
