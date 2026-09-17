"""Tests for the clean Linux Diffusers runtime input lock."""

from __future__ import annotations

import hashlib
import json
import unittest
import zipfile
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import patch

from packaging.tags import parse_tag

from diffusers_clean_runtime_lock import (
    CLEAN_BASE_IMAGE_DIGEST,
    CLEAN_PROOF_IMAGE_DIGEST,
    CleanRuntimeLockError,
    _read_debian_metadata,
    _validate_clean_runtime_lock,
    _validate_debian_records,
    _validate_lock_digest,
    _validate_manifest_header,
    _validate_python_records,
    _wheel_tags_match,
)


CHECKED_IN_LOCK = (
    Path(__file__).resolve().parents[1]
    / "docs"
    / "local-image-linux-clean-runtime-input-lock.json"
)


def _write_wheel(
    path: Path,
    name: str,
    version: str,
    wheel_tags: tuple[str, ...] = ("py3-none-any",),
) -> tuple[int, str]:
    """Create one minimal wheel-shaped archive and return its exact measurement."""
    metadata = f"Metadata-Version: 2.4\nName: {name}\nVersion: {version}\n\n"
    tag_fields = "".join(f"Tag: {tag}\n" for tag in wheel_tags)
    wheel = f"Wheel-Version: 1.0\nGenerator: bottie-test\n{tag_fields}\n"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(f"{name}-{version}.dist-info/METADATA", metadata)
        archive.writestr(f"{name}-{version}.dist-info/WHEEL", wheel)
    contents = path.read_bytes()
    return len(contents), hashlib.sha256(contents).hexdigest()


def _write_deb(path: Path) -> tuple[int, str]:
    """Create opaque Debian bytes for the metadata-reader seam."""
    path.write_bytes(b"exact-debian-package")
    contents = path.read_bytes()
    return len(contents), hashlib.sha256(contents).hexdigest()


def _refresh_lock_digest(manifest: dict) -> None:
    """Recompute the canonical self-digest after an intentional test mutation."""
    payload = {key: value for key, value in manifest.items() if key != "lockSha256"}
    manifest["lockSha256"] = hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _locked_manifest(root: Path) -> tuple[dict, dict[Path, tuple[str, str, str]]]:
    """Create one small valid manifest and its independently supplied Debian metadata."""
    python_directory = root / "python"
    debian_directory = root / "debian"
    python_directory.mkdir(parents=True)
    debian_directory.mkdir()
    wheel_name = "example_pkg-1.2.3-py3-none-any.whl"
    wheel_size, wheel_sha256 = _write_wheel(
        python_directory / wheel_name, "example_pkg", "1.2.3"
    )
    debian_name = "libexample_2.0-1_arm64.deb"
    debian_path = debian_directory / debian_name
    debian_size, debian_sha256 = _write_deb(debian_path)
    manifest = {
        "schemaVersion": 1,
        "sourceImageDigest": CLEAN_PROOF_IMAGE_DIGEST,
        "baseImageDigest": CLEAN_BASE_IMAGE_DIGEST,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "pythonVersion": "3.12.3",
        "pythonArtifacts": [
            {
                "name": "example-pkg",
                "version": "1.2.3",
                "filename": wheel_name,
                "byteSize": wheel_size,
                "sha256": wheel_sha256,
            }
        ],
        "debianArtifacts": [
            {
                "name": "libexample",
                "version": "2.0-1",
                "architecture": "arm64",
                "filename": debian_name,
                "byteSize": debian_size,
                "sha256": debian_sha256,
            }
        ],
    }
    manifest["lockSha256"] = hashlib.sha256(
        json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    return manifest, {debian_path.resolve(): ("libexample", "2.0-1", "arm64")}


class DiffusersCleanRuntimeLockTests(unittest.TestCase):
    """Protect immutable, complete, path-free clean-runtime inputs."""

    @patch("diffusers_clean_runtime_lock.subprocess.run")
    def test_reads_debian_identity_with_dpkg_show_format(self, run) -> None:
        """Real Debian inspection uses the dpkg mode that expands format fields."""
        run.return_value = SimpleNamespace(
            returncode=0, stdout="example\t1.2-3\tarm64\n", stderr=""
        )
        path = Path("/tmp/example.deb")

        identity = _read_debian_metadata(path)

        self.assertEqual(identity, ("example", "1.2-3", "arm64"))
        self.assertEqual(
            run.call_args.args[0],
            [
                "dpkg-deb",
                "--show",
                "--showformat=${Package}\t${Version}\t${Architecture}\n",
                str(path),
            ],
        )

    def test_checked_in_lock_remains_closed_and_path_free(self) -> None:
        """The retained manifest keeps its exact identities, counts, order, and digest."""
        manifest = json.loads(CHECKED_IN_LOCK.read_text(encoding="utf-8"))

        _validate_manifest_header(manifest)
        python_artifacts = _validate_python_records(
            manifest["pythonArtifacts"], expected_count=63
        )
        debian_artifacts = _validate_debian_records(
            manifest["debianArtifacts"], expected_count=112
        )
        _validate_lock_digest(manifest)

        self.assertEqual(len(python_artifacts), 63)
        self.assertEqual(len(debian_artifacts), 112)
        self.assertNotIn("/home/", json.dumps(manifest))

    def test_verifies_exact_wheel_and_debian_bytes(self) -> None:
        """The verifier binds both artifact ecosystems and emits only path-free evidence."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)

            evidence = _validate_clean_runtime_lock(
                manifest,
                root,
                expected_python_count=1,
                expected_debian_count=1,
                read_debian_metadata=debian_metadata.__getitem__,
            )

        self.assertEqual(evidence["pythonArtifactCount"], 1)
        self.assertEqual(evidence["debianArtifactCount"], 1)
        self.assertEqual(evidence["lockSha256"], manifest["lockSha256"])
        self.assertTrue(evidence["verified"])
        self.assertNotIn(str(root), json.dumps(evidence))

    def test_rejects_byte_drift_and_unlisted_artifacts(self) -> None:
        """Artifact mutation or an extra file invalidates the complete input set."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            (root / "python" / manifest["pythonArtifacts"][0]["filename"]).write_bytes(
                b"drift"
            )

            with self.assertRaisesRegex(
                CleanRuntimeLockError, "artifact bytes have drifted"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    root,
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

            manifest, debian_metadata = _locked_manifest(Path(temporary) / "second")
            second_root = Path(temporary) / "second"
            (second_root / "debian" / "unlisted.deb").write_bytes(b"extra")
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "artifact tree is not exact"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    second_root,
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

    def test_rejects_identity_and_manifest_digest_drift(self) -> None:
        """File labels cannot substitute for inspected wheel, Debian, or manifest identities."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            manifest["pythonArtifacts"][0]["name"] = "substitute"
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "wheel identity has drifted"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    root,
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

            manifest, debian_metadata = _locked_manifest(Path(temporary) / "second")
            manifest["debianArtifacts"][0]["version"] = "2.0-2"
            _refresh_lock_digest(manifest)
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "Debian identity has drifted"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    Path(temporary) / "second",
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

            manifest, debian_metadata = _locked_manifest(Path(temporary) / "third")
            manifest["lockSha256"] = "f" * 64
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "lock digest has drifted"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    Path(temporary) / "third",
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

    def test_rejects_incomplete_unsorted_or_path_shaped_records(self) -> None:
        """The exact clean-image counts and portable sorted names are closed requirements."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "Python artifact count is not exact"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    root,
                    2,
                    1,
                    debian_metadata.__getitem__,
                )

            manifest, debian_metadata = _locked_manifest(Path(temporary) / "second")
            manifest["pythonArtifacts"][0]["filename"] = "../example.whl"
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "artifact filename is invalid"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    Path(temporary) / "second",
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

    def test_rejects_wrong_architecture_artifacts(self) -> None:
        """A correctly hashed x86 wheel or foreign Debian archive cannot enter the ARM64 lock."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            python_directory = root / "python"
            old_path = python_directory / manifest["pythonArtifacts"][0]["filename"]
            filename = "example_pkg-1.2.3-py3-none-manylinux_2_28_x86_64.whl"
            path = old_path.rename(python_directory / filename)
            contents = path.read_bytes()
            manifest["pythonArtifacts"][0].update(
                filename=filename,
                byteSize=len(contents),
                sha256=hashlib.sha256(contents).hexdigest(),
            )
            _refresh_lock_digest(manifest)
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "wheel target is not ARM64"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    root,
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

            second_root = Path(temporary) / "second"
            manifest, debian_metadata = _locked_manifest(second_root)
            manifest["debianArtifacts"][0]["architecture"] = "amd64"
            _refresh_lock_digest(manifest)
            with self.assertRaisesRegex(
                CleanRuntimeLockError, "Debian package architecture is invalid"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    second_root,
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

    def test_rejects_wheel_with_embedded_tags_that_drift_from_filename(self) -> None:
        """Renaming a foreign wheel cannot substitute an ARM64-compatible archive."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            old_filename = manifest["pythonArtifacts"][0]["filename"]
            filename = (
                "example_pkg-1.2.3-cp312-cp312-manylinux_2_39_aarch64.whl"
            )
            path = root / "python" / filename
            (root / "python" / old_filename).unlink()
            wheel_size, wheel_sha256 = _write_wheel(
                path,
                "example_pkg",
                "1.2.3",
                ("cp312-cp312-manylinux_2_39_x86_64",),
            )
            manifest["pythonArtifacts"][0].update(
                filename=filename,
                byteSize=wheel_size,
                sha256=wheel_sha256,
            )
            _refresh_lock_digest(manifest)

            with self.assertRaisesRegex(
                CleanRuntimeLockError, "wheel tags have drifted"
            ):
                _validate_clean_runtime_lock(
                    manifest,
                    root,
                    1,
                    1,
                    debian_metadata.__getitem__,
                )

    def test_accepts_only_byte_bound_nvidia_sbsa_tag_mismatch(self) -> None:
        """The sole upstream SBSA mismatch is bound to its official immutable wheel."""
        record = {
            "name": "nvidia-cusparselt-cu13",
            "version": "0.8.0",
            "filename": (
                "nvidia_cusparselt_cu13-0.8.0-py3-none-"
                "manylinux2014_aarch64.whl"
            ),
            "byteSize": 220_791_277,
            "sha256": (
                "400c6ed1cf6780fc6efedd64ec9f1345871767e6a1a0a552a1ea0578117ea77c"
            ),
        }
        filename_tags = parse_tag("py3-none-manylinux2014_aarch64")
        embedded_tags = set(parse_tag("py3-none-manylinux2014_sbsa"))

        self.assertTrue(_wheel_tags_match(record, filename_tags, embedded_tags))
        record["sha256"] = "f" * 64
        self.assertFalse(_wheel_tags_match(record, filename_tags, embedded_tags))

    def test_accepts_expanded_tags_for_a_compressed_wheel_filename(self) -> None:
        """Expanded embedded tags remain equivalent to one compressed filename tag set."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            old_filename = manifest["pythonArtifacts"][0]["filename"]
            filename = "example_pkg-1.2.3-py2.py3-none-any.whl"
            path = root / "python" / filename
            (root / "python" / old_filename).unlink()
            wheel_size, wheel_sha256 = _write_wheel(
                path,
                "example_pkg",
                "1.2.3",
                ("py2-none-any", "py3-none-any"),
            )
            manifest["pythonArtifacts"][0].update(
                filename=filename,
                byteSize=wheel_size,
                sha256=wheel_sha256,
            )
            _refresh_lock_digest(manifest)

            evidence = _validate_clean_runtime_lock(
                manifest,
                root,
                1,
                1,
                debian_metadata.__getitem__,
            )

        self.assertTrue(evidence["verified"])

    def test_ignores_vendored_dist_info_below_the_wheel_root(self) -> None:
        """Vendored package metadata cannot replace or duplicate the wheel's own metadata."""
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, debian_metadata = _locked_manifest(root)
            record = manifest["pythonArtifacts"][0]
            path = root / "python" / record["filename"]
            with zipfile.ZipFile(path, "a") as archive:
                archive.writestr(
                    "example_pkg/_vendor/vendored-1.0.dist-info/METADATA",
                    "Metadata-Version: 2.4\nName: vendored\nVersion: 1.0\n\n",
                )
                archive.writestr(
                    "example_pkg/_vendor/vendored-1.0.dist-info/WHEEL",
                    "Wheel-Version: 1.0\nTag: py3-none-any\n",
                )
            contents = path.read_bytes()
            record.update(
                byteSize=len(contents), sha256=hashlib.sha256(contents).hexdigest()
            )
            _refresh_lock_digest(manifest)

            evidence = _validate_clean_runtime_lock(
                manifest,
                root,
                1,
                1,
                debian_metadata.__getitem__,
            )

        self.assertTrue(evidence["verified"])


if __name__ == "__main__":
    unittest.main()
