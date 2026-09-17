"""Tests for deterministic, offline clean-runtime rebuild evidence."""

from __future__ import annotations

import hashlib
import io
import json
import tarfile
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from diffusers_clean_runtime_lock import CLEAN_BASE_IMAGE_DIGEST
from diffusers_clean_runtime_rebuild import (
    CleanRuntimeRebuildError,
    _build_command,
    _collect_regular_files,
    _compare_rebuilds,
    _render_dockerfile,
    validate_rebuild_plan,
)


LOCK_SHA256 = "2628e5ec74886a91b6b1b68890666d2340c7a99c9f20b51b9b9dc483946c9831"
SOURCE_NAMES = (
    "diffusers_pytorch_worker.py",
    "diffusers_worker.py",
    "mlx_worker.py",
)
REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
CHECKED_IN_PLAN = (
    REPOSITORY_ROOT / "docs" / "local-image-linux-clean-runtime-rebuild-plan.json"
)


def _write_sources(root: Path) -> list[dict]:
    """Create the three reviewed worker inputs and return their plan records."""
    records = []
    for name in SOURCE_NAMES:
        contents = f'"""Reviewed {name}."""\n'.encode()
        (root / name).write_bytes(contents)
        records.append(
            {
                "filename": name,
                "byteSize": len(contents),
                "sha256": hashlib.sha256(contents).hexdigest(),
            }
        )
    return records


def _plan(records: list[dict]) -> dict:
    """Return one closed rebuild plan for focused validation."""
    return {
        "schemaVersion": 1,
        "baseImageDigest": CLEAN_BASE_IMAGE_DIGEST,
        "inputLockSha256": LOCK_SHA256,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "workerSources": records,
    }


def _tar(entries: list[tuple[str, bytes, int]], mtime: int = 0) -> io.BytesIO:
    """Return an in-memory rootfs tar with regular files and one hard link."""
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w") as archive:
        root = tarfile.TarInfo(".")
        root.type = tarfile.DIRTYPE
        root.mode = 0o755
        root.mtime = mtime
        archive.addfile(root)
        for name, contents, mode in entries:
            member = tarfile.TarInfo(name)
            member.size = len(contents)
            member.mode = mode
            member.mtime = mtime
            archive.addfile(member, io.BytesIO(contents))
        hardlink = tarfile.TarInfo("opt/bottie/worker-link.py")
        hardlink.type = tarfile.LNKTYPE
        hardlink.linkname = "opt/bottie/worker.py"
        hardlink.mode = 0o755
        hardlink.mtime = mtime
        archive.addfile(hardlink)
    output.seek(0)
    return output


class DiffusersCleanRuntimeRebuildTests(unittest.TestCase):
    """Protect the frozen-source, network-disabled two-build contract."""

    def test_checked_in_plan_binds_current_reviewed_worker_bytes(self) -> None:
        """The retained plan remains valid for the exact three checked-in worker files."""
        plan = json.loads(CHECKED_IN_PLAN.read_text(encoding="utf-8"))

        sources = validate_rebuild_plan(
            plan, REPOSITORY_ROOT / "local-image-worker", LOCK_SHA256
        )

        self.assertEqual(sources, SOURCE_NAMES)

    def test_validates_only_the_three_reviewed_source_files(self) -> None:
        """The plan binds exact source bytes and refuses unreviewed build context."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = _plan(_write_sources(root))

            validated = validate_rebuild_plan(plan, root, LOCK_SHA256)

            self.assertEqual(validated, tuple(SOURCE_NAMES))
            plan["workerSources"].append(
                {"filename": "extra.py", "byteSize": 1, "sha256": "0" * 64}
            )
            with self.assertRaisesRegex(CleanRuntimeRebuildError, "source allowlist"):
                validate_rebuild_plan(plan, root, LOCK_SHA256)

    def test_rejects_source_or_input_lock_drift(self) -> None:
        """A changed worker byte or different frozen lock cannot enter the build."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = _plan(_write_sources(root))
            (root / SOURCE_NAMES[0]).write_text("changed", encoding="utf-8")

            with self.assertRaisesRegex(CleanRuntimeRebuildError, "source bytes"):
                validate_rebuild_plan(plan, root, LOCK_SHA256)
            with self.assertRaisesRegex(CleanRuntimeRebuildError, "input lock"):
                validate_rebuild_plan(_plan(_write_sources(root)), root, "f" * 64)

    def test_renders_an_offline_exact_base_build(self) -> None:
        """The generated recipe has no index, URL, mutable base, or dependency resolution."""
        dockerfile = _render_dockerfile()

        self.assertIn(f"FROM ubuntu@{CLEAN_BASE_IMAGE_DIGEST}", dockerfile)
        self.assertIn("COPY --from=frozen-inputs /debian/ /tmp/debian/", dockerfile)
        self.assertIn("COPY --from=frozen-inputs /python/ /tmp/python/", dockerfile)
        self.assertIn("--no-index --no-deps", dockerfile)
        self.assertIn("/var/log/dpkg.log", dockerfile)
        self.assertNotIn("# syntax=", dockerfile)
        self.assertNotIn("apt-get", dockerfile)
        self.assertNotIn("http://", dockerfile)
        self.assertNotIn("https://", dockerfile)

    def test_build_command_disables_network_pull_and_cache(self) -> None:
        """Each independently named build uses only the verified local contexts."""
        command = _build_command(
            Path("/absolute/context"), Path("/absolute/inputs"), "bottie-rebuild-a"
        )

        self.assertEqual(command[:3], ["docker", "buildx", "build"])
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--no-cache", command)
        self.assertIn("--pull=false", command)
        self.assertEqual(command[command.index("--platform") + 1], "linux/arm64")
        self.assertEqual(
            command[command.index("--build-context") + 1],
            "frozen-inputs=/absolute/inputs",
        )
        self.assertEqual(command[command.index("--tag") + 1], "bottie-rebuild-a")

    def test_regular_file_measurement_ignores_time_and_runtime_hosts(self) -> None:
        """Normalization binds paths, ownership, modes, sizes, bytes, and links, not mtimes."""
        entries = [
            ("opt/bottie/worker.py", b"worker", 0o755),
            ("etc/hosts", b"runtime-specific", 0o644),
        ]

        first = _collect_regular_files(_tar(entries, mtime=1))
        second = _collect_regular_files(_tar(entries, mtime=999))

        self.assertEqual(first, second)
        self.assertEqual(
            [record[0] for record in first],
            [
                "opt/bottie/worker-link.py",
                "opt/bottie/worker.py",
            ],
        )

    def test_rejects_traversal_in_exported_rootfs(self) -> None:
        """A malformed archive cannot escape or alias the normalized filesystem namespace."""
        with self.assertRaisesRegex(CleanRuntimeRebuildError, "rootfs path"):
            _collect_regular_files(_tar([("../escape", b"bad", 0o644)]))

    def test_compares_distinct_rebuilds_with_exact_inventory_and_files(self) -> None:
        """Two names may differ only when their package inventory and normalized files agree."""
        inventory = {
            "pythonDistributions": [{"name": "example", "version": "1.0"}],
            "debianPackages": [
                {"name": "base", "version": "1", "architecture": "arm64"}
            ],
        }
        files = [
            (
                "opt/bottie/worker.py",
                0o755,
                0,
                0,
                6,
                hashlib.sha256(b"worker").hexdigest(),
            )
        ]
        first = {
            "buildName": "rebuild-a",
            "imageId": "sha256:" + "a" * 64,
            "inventory": inventory,
            "regularFiles": files,
        }
        second = {
            "buildName": "rebuild-b",
            "imageId": "sha256:" + "b" * 64,
            "inventory": json.loads(json.dumps(inventory)),
            "regularFiles": list(files),
        }

        evidence = _compare_rebuilds(first, second, LOCK_SHA256)

        self.assertTrue(evidence["rebuildsAgree"])
        self.assertEqual(evidence["regularFileCount"], 1)
        second["regularFiles"][0] = (*files[0][:-1], "f" * 64)
        with self.assertRaisesRegex(CleanRuntimeRebuildError, "filesystem"):
            _compare_rebuilds(first, second, LOCK_SHA256)


if __name__ == "__main__":
    unittest.main()
