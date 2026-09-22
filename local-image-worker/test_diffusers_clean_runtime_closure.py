"""Tests for the rebuilt clean-runtime closure profile and agreement gate."""

from __future__ import annotations

import json
import os
import struct
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import diffusers_clean_runtime_closure as clean_closure
import diffusers_clean_runtime_closure_host as clean_closure_host
import diffusers_runtime_closure_host as closure_host
from diffusers_clean_runtime_closure import CleanRuntimeClosureError
from diffusers_runtime_closure_profiles import (
    CLEAN_RUNTIME_PROFILE_NAME,
    ClosureProfileError,
    runtime_closure_profile,
)


SHA256 = "a" * 64


def _lock() -> dict:
    """Return one minimal exact-shape lock for focused identity tests."""
    return {
        "schemaVersion": 1,
        "sourceImageDigest": clean_closure.CLEAN_SOURCE_IMAGE_DIGEST,
        "baseImageDigest": clean_closure.CLEAN_BASE_IMAGE_DIGEST,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "pythonVersion": clean_closure.CLEAN_PYTHON_VERSION,
        "pythonArtifacts": [
            {
                "name": "example",
                "version": "1.0",
                "filename": "example-1.0-py3-none-any.whl",
                "byteSize": 10,
                "sha256": SHA256,
            }
        ],
        "debianArtifacts": [
            {
                "name": "libexample",
                "version": "1.0-1",
                "architecture": "arm64",
                "filename": "libexample_1.0-1_arm64.deb",
                "byteSize": 20,
                "sha256": "b" * 64,
            }
        ],
        "lockSha256": "c" * 64,
    }


def _closure(trace: dict, file_sha256: str = SHA256) -> dict:
    """Return one path-free clean closure result for agreement tests."""
    return {
        "schemaVersion": 1,
        "workerVersion": "worker",
        "runtimeId": "runtime",
        "modelId": "model",
        "modelRevision": "revision",
        "target": {"operatingSystem": "linux", "architecture": "aarch64"},
        "pythonVersion": "3.12.3",
        "trace": trace,
        "environment": {
            "pythonComponentCount": 1,
            "nativeComponentCount": 1,
            "unmanagedNativeComponentCount": 0,
        },
        "closure": {
            "fileCount": 1,
            "byteSize": 10,
            "elfFileCount": 0,
            "componentCount": 1,
            "components": [
                {
                    "identity": "python:example@1",
                    "fileCount": 1,
                    "byteSize": 10,
                    "licenseFileCount": 0,
                    "declaredLicenseExpression": "undeclared",
                    "reviewedLicenseExpression": None,
                }
            ],
        },
        "runtimeFilesSha256": file_sha256,
        "hostDriverBoundary": {"allowedSonames": [], "observedSonames": []},
        "blockers": [
            "python:example@1:missing-license-bytes",
            "python:example@1:undeclared-license",
            "python:example@1:unreviewed-license-expression",
        ],
        "closureComplete": True,
        "licenseReviewed": False,
        "assemblyEligible": False,
        "distributionReviewed": False,
        "baseImage": "ubuntu:24.04",
        "baseImageDigest": clean_closure.CLEAN_BASE_IMAGE_DIGEST,
        "derivedImageDigest": clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
    }


class DiffusersCleanRuntimeClosureTests(unittest.TestCase):
    """Protect clean-runtime identity, collection, and two-trace agreement."""

    def test_profile_is_closed_over_rebuilt_image_worker_and_traces(self) -> None:
        """The clean profile accepts only the retained rebuild and two fresh contexts."""
        profile = runtime_closure_profile(CLEAN_RUNTIME_PROFILE_NAME)

        self.assertEqual(
            profile.derived_image_digest, clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST
        )
        self.assertEqual(
            profile.base_image_digest, clean_closure.CLEAN_BASE_IMAGE_DIGEST
        )
        self.assertEqual(profile.python_executable, "/opt/bottie/venv/bin/python")
        self.assertEqual(profile.identity.runtime_id, clean_closure.CLEAN_RUNTIME_ID)
        self.assertEqual(len(profile.accepted_trace_context_sha256s), 2)
        self.assertFalse(profile.use_ngc_native_components)
        self.assertFalse(profile.allow_license_review)
        self.assertEqual(
            profile.license_source_components,
            frozenset(
                {
                    "python:sentencepiece@0.2.2",
                    "python:tokenizers@0.23.2",
                }
            ),
        )
        self.assertTrue(profile.require_all_license_sources)

        with self.assertRaises(ClosureProfileError):
            runtime_closure_profile("caller-selected-profile")

    def test_clean_environment_requires_the_complete_locked_identities(self) -> None:
        """Clean collection compares all Python and Debian identities with the frozen lock."""
        lock = _lock()
        python_components = [
            {
                "ecosystem": "python",
                "name": "example",
                "version": "1.0",
                "licenseExpression": "undeclared",
                "licenseFiles": [],
            }
        ]
        native_components = [
            {
                "ecosystem": "deb",
                "name": "libexample",
                "version": "1.0-1",
                "architecture": "arm64",
                "licenseExpression": "undeclared",
                "licenseFiles": [],
            }
        ]

        clean_closure.validate_installed_identities(
            lock,
            python_components,
            native_components,
            expected_python_count=1,
            expected_debian_count=1,
            validate_digest=False,
        )
        python_components[0]["version"] = "2.0"
        with self.assertRaisesRegex(CleanRuntimeClosureError, "Python identities"):
            clean_closure.validate_installed_identities(
                lock,
                python_components,
                native_components,
                expected_python_count=1,
                expected_debian_count=1,
                validate_digest=False,
            )

    def test_checked_in_lock_supplies_every_expected_clean_identity(self) -> None:
        """The retained lock itself satisfies the profile's complete identity contract."""
        lock_path = (
            Path(__file__).resolve().parent.parent
            / "docs"
            / ("local-image-linux-clean-runtime-input-lock.json")
        )
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
        python_components = [
            {
                "name": artifact["name"],
                "version": artifact["version"],
            }
            for artifact in lock["pythonArtifacts"]
        ]
        native_components = [
            {
                "name": artifact["name"],
                "version": artifact["version"],
                "architecture": artifact["architecture"],
            }
            for artifact in lock["debianArtifacts"]
        ]

        clean_closure.validate_installed_identities(
            lock,
            python_components,
            native_components,
        )

    def test_clean_components_use_the_locks_canonical_python_identity(self) -> None:
        """Underscore and case variants normalize before lock and ownership comparison."""
        component = clean_closure.normalize_clean_python_component(
            {
                "ecosystem": "python",
                "name": "Example_Package",
                "version": "1.0.0",
                "licenseExpression": "MIT",
                "licenseFiles": [],
            }
        )

        self.assertEqual(component["name"], "example-package")
        self.assertEqual(component["version"], "1.0.0")

    def test_reads_aarch64_elf_dependencies_without_an_external_tool(self) -> None:
        """The frozen image needs no readelf package to close NEEDED and SONAME edges."""
        contents = bytearray(0x300)
        identity = b"\x7fELF\x02\x01\x01" + bytes(9)
        contents[:64] = struct.pack(
            "<16sHHIQQQIHHHHHH",
            identity,
            3,
            clean_closure.ELF_MACHINE_AARCH64,
            1,
            0,
            64,
            0,
            0,
            64,
            56,
            2,
            0,
            0,
            0,
        )
        contents[64:120] = struct.pack(
            "<IIQQQQQQ",
            1,
            5,
            0,
            0x400000,
            0x400000,
            len(contents),
            len(contents),
            0x1000,
        )
        contents[120:176] = struct.pack(
            "<IIQQQQQQ",
            2,
            4,
            0x100,
            0x400100,
            0x400100,
            80,
            80,
            8,
        )
        strings = b"\0libneeded.so\0libself.so\0"
        soname_offset = strings.index(b"libself.so")
        dynamic = [
            (clean_closure.ELF_DT_NEEDED, 1),
            (clean_closure.ELF_DT_STRTAB, 0x400200),
            (clean_closure.ELF_DT_STRSZ, len(strings)),
            (clean_closure.ELF_DT_SONAME, soname_offset),
            (clean_closure.ELF_DT_NULL, 0),
        ]
        contents[0x100:0x150] = b"".join(
            struct.pack("<qQ", tag, value) for tag, value in dynamic
        )
        contents[0x200 : 0x200 + len(strings)] = strings

        with tempfile.TemporaryDirectory() as directory:
            elf = Path(directory) / "runtime.so"
            elf.write_bytes(contents)

            self.assertEqual(
                clean_closure.read_clean_elf_dependencies(elf),
                ({"libneeded.so"}, "libself.so"),
            )
            unterminated = bytearray(contents)
            unterminated[0x140:0x150] = struct.pack(
                "<qQ", clean_closure.ELF_DT_NEEDED, 1
            )
            elf.write_bytes(unterminated)
            with self.assertRaisesRegex(CleanRuntimeClosureError, "unterminated"):
                clean_closure.read_clean_elf_dependencies(elf)
            elf.write_bytes(b"\x7fELF")
            with self.assertRaisesRegex(CleanRuntimeClosureError, "header"):
                clean_closure.read_clean_elf_dependencies(elf)

    def test_dependency_free_elf_still_requires_stable_bytes(self) -> None:
        """An ELF without a dynamic segment cannot bypass the before/after check."""
        identity = b"\x7fELF\x02\x01\x01" + bytes(9)
        contents = struct.pack(
            "<16sHHIQQQIHHHHHH",
            identity,
            3,
            clean_closure.ELF_MACHINE_AARCH64,
            1,
            0,
            64,
            0,
            0,
            64,
            56,
            0,
            0,
            0,
            0,
        )
        with tempfile.TemporaryDirectory() as directory:
            elf = Path(directory) / "static-runtime"
            elf.write_bytes(contents)
            observed = elf.stat()
            changed = SimpleNamespace(
                st_dev=observed.st_dev,
                st_ino=observed.st_ino,
                st_size=observed.st_size,
                st_mtime_ns=observed.st_mtime_ns + 1,
            )
            with patch.object(
                clean_closure.os, "fstat", side_effect=[observed, changed]
            ):
                with self.assertRaisesRegex(CleanRuntimeClosureError, "changed"):
                    clean_closure.read_clean_elf_dependencies(elf)

    def test_host_collection_rejects_a_root_caller(self) -> None:
        """Read-only container flags never substitute for an explicit non-root host identity."""
        with patch.object(closure_host.os, "getuid", return_value=0):
            with self.assertRaisesRegex(closure_host.ClosureEvidenceError, "non-root"):
                closure_host.collect_verified_runtime_closure(
                    "proof-image",
                    Path("/trace"),
                    profile_name=CLEAN_RUNTIME_PROFILE_NAME,
                    clean_runtime_lock=Path("/lock"),
                )

    def test_legacy_profile_preserves_root_host_behavior(self) -> None:
        """The clean profile's host policy does not alter the retained NGC route."""
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(closure_host.os, "getuid", return_value=0),
                patch.object(
                    closure_host, "_inspect_derived_image", return_value=SHA256
                ),
                patch.object(
                    closure_host.subprocess,
                    "run",
                    return_value=SimpleNamespace(
                        returncode=0, stdout='{"closure": {}}', stderr=""
                    ),
                ),
            ):
                closure_host.collect_verified_runtime_closure(
                    "proof-image", Path(directory)
                )

    def test_host_profile_uses_exact_interpreter_and_read_only_lock(self) -> None:
        """The clean classifier runs only by immutable ID with the frozen lock mounted read-only."""
        completed = SimpleNamespace(returncode=0, stdout='{"closure": {}}', stderr="")
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(
                closure_host,
                "_inspect_exact_image",
                return_value=clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
            ),
            patch.object(closure_host.subprocess, "run", return_value=completed) as run,
        ):
            root = Path(directory)
            trace = root / "trace"
            trace.mkdir()
            sources = root / "sources"
            sources.mkdir()
            lock = root / "lock.json"
            lock.write_text("{}", encoding="utf-8")
            closure_host.collect_verified_runtime_closure(
                clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
                trace,
                license_sources=sources,
                profile_name=CLEAN_RUNTIME_PROFILE_NAME,
                clean_runtime_lock=lock,
            )

        command = run.call_args.args[0]
        self.assertEqual(
            command[command.index("--entrypoint") + 1], "/opt/bottie/venv/bin/python"
        )
        self.assertEqual(command[command.index("--network") + 1], "none")
        expected_user = f"{os.getuid()}:{os.getgid()}"
        self.assertEqual(command[command.index("--user") + 1], expected_user)
        lock_mount = next(
            value for value in command if "bottie-clean-runtime-lock.json" in value
        )
        self.assertIn(str(lock.resolve()), lock_mount)
        self.assertIn("readonly", lock_mount)
        self.assertEqual(
            command[command.index("--profile") + 1], CLEAN_RUNTIME_PROFILE_NAME
        )

    def test_two_trace_gate_retains_one_summary_only_when_closures_agree(self) -> None:
        """Trace-specific digests may differ while every classified byte and blocker must match."""
        first_trace = {
            "traceContextSha256": "1" * 64,
            "pythonTraceSha256": "2" * 64,
            "processMapsSha256": "3" * 64,
        }
        second_trace = {
            "traceContextSha256": "4" * 64,
            "pythonTraceSha256": "5" * 64,
            "processMapsSha256": "6" * 64,
        }

        summary = clean_closure_host.agreed_closure_summary(
            [_closure(first_trace), _closure(second_trace)]
        )

        self.assertEqual(summary["traces"], [first_trace, second_trace])
        self.assertNotIn("trace", summary)
        self.assertFalse(summary["assemblyEligible"])
        with self.assertRaisesRegex(CleanRuntimeClosureError, "do not agree"):
            clean_closure_host.agreed_closure_summary(
                [_closure(first_trace), _closure(second_trace, "b" * 64)]
            )


if __name__ == "__main__":
    unittest.main()
