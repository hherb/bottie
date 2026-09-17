"""Tests for the deterministic Diffusers runtime-closure review."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import diffusers_runtime_closure as runtime_closure
import diffusers_runtime_closure_host as runtime_closure_host
from diffusers_runtime_closure import (
    ClosureEvidenceError,
    build_closure_review,
    host_driver_soname,
    parse_elf_dependencies,
    parse_process_maps,
    parse_trace_paths,
)
from diffusers_runtime_dependencies import resolve_required_python_components


SHA256 = "a" * 64


class DiffusersRuntimeClosureTests(unittest.TestCase):
    """Protect exact closure parsing, ownership, and fail-closed eligibility."""

    def test_first_party_boundary_contains_only_the_diffusers_worker(self) -> None:
        """An unrelated proof-image worker must not inflate the minimal closure."""
        self.assertEqual(
            runtime_closure.FIRST_PARTY_FILES,
            {Path("/opt/bottie/diffusers_worker.py")},
        )

    def test_parses_only_canonical_absolute_trace_paths(self) -> None:
        """The trace accepts bounded absolute file events without retaining event order."""
        trace = "\n".join(
            [
                json.dumps({"kind": "open", "path": "/usr/share/metadata/NOTICE"}),
                json.dumps({"kind": "module", "path": "/usr/lib/example.py"}),
                json.dumps({"kind": "dlopen", "path": "/usr/lib/libexample.so"}),
            ]
        )

        self.assertEqual(
            parse_trace_paths(trace),
            [Path("/usr/lib/example.py"), Path("/usr/lib/libexample.so")],
        )

        for invalid in ["relative.py", "/usr//lib/example.py", "/usr/lib/../tmp/example.py", "/bad\npath"]:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ClosureEvidenceError):
                    parse_trace_paths(json.dumps({"kind": "open", "path": invalid}))

    def test_process_maps_rejects_deleted_files_and_ignores_anonymous_regions(self) -> None:
        """Native evidence keeps real mapped files and rejects unstable deleted mappings."""
        maps = "\n".join(
            [
                "00400000-00452000 r-xp 00000000 08:02 1 /usr/bin/python3.12",
                "7f000000-7f100000 rw-p 00000000 00:00 0 [heap]",
                "7f200000-7f300000 r--p 00000000 08:02 2 /usr/lib/libz.so.1",
                "7f400000-7f500000 rw-s 00000000 00:01 3 /dev/zero (deleted)",
                "7f600000-7f600100 rw-s 00000000 00:bf 4 /dev/shm/sem.example (deleted)",
            ]
        )

        self.assertEqual(
            parse_process_maps(maps),
            [Path("/usr/bin/python3.12"), Path("/usr/lib/libz.so.1")],
        )

        with self.assertRaisesRegex(ClosureEvidenceError, "deleted"):
            parse_process_maps(
                "00400000-00452000 r-xp 00000000 08:02 1 /tmp/runtime.so (deleted)"
            )

    def test_parses_exact_needed_and_soname_entries(self) -> None:
        """ELF dependency parsing ignores unrelated dynamic-section text."""
        output = """
 0x0000000000000001 (NEEDED) Shared library: [libtorch.so]
 0x000000000000000e (SONAME) Library soname: [libexample.so.1]
 0x000000000000001d (RUNPATH) Library runpath: [$ORIGIN]
"""

        self.assertEqual(
            parse_elf_dependencies(output),
            ({"libtorch.so"}, "libexample.so.1"),
        )

    def test_read_elf_dependencies_translates_native_evidence_errors(self) -> None:
        """The closure boundary exposes one stable error type for malformed ELF evidence."""
        completed = SimpleNamespace(
            returncode=0,
            stdout=(
                b"(SONAME) Library soname: [libfirst.so.1]\n"
                b"(SONAME) Library soname: [libsecond.so.1]\n"
            ),
        )

        with patch(
            "diffusers_runtime_trace_evidence.subprocess.run", return_value=completed
        ):
            with self.assertRaisesRegex(ClosureEvidenceError, "multiple SONAME"):
                runtime_closure._read_elf_dependencies(Path("/runtime.so"))

    def test_host_driver_boundary_uses_declared_soname_not_versioned_filename(self) -> None:
        """An injected versioned driver file is classified only through its allowlisted SONAME."""
        self.assertEqual(
            host_driver_soname(
                "libcuda.so.580.173.02",
                "libcuda.so.1",
                {"libcuda.so.1"},
            ),
            "libcuda.so.1",
        )
        self.assertIsNone(host_driver_soname("libcuda.so.580", None, {"libcuda.so.1"}))

    def test_resolves_required_distributions_recursively_without_optional_extras(self) -> None:
        """Installed dependency metadata closes the active runtime graph deterministically."""
        requirements = {
            "python:diffusers@0.40": [
                "torch[cuda]>=2.0",
                "sphinx; extra == 'docs'",
            ],
            "python:torch@2.10": [
                "triton; extra == 'cuda'",
                "typing-extensions>=4",
            ],
            "python:triton@3.5": [],
            "python:typing-extensions@4.15": [],
        }
        installed = {
            "diffusers": "python:diffusers@0.40",
            "torch": "python:torch@2.10",
            "triton": "python:triton@3.5",
            "typing-extensions": "python:typing-extensions@4.15",
        }

        resolved, blockers = resolve_required_python_components(
            {"python:diffusers@0.40"},
            requirements,
            installed,
            {"python_version": "3.12", "extra": ""},
        )

        self.assertEqual(
            resolved,
            {
                "python:diffusers@0.40",
                "python:torch@2.10",
                "python:triton@3.5",
                "python:typing-extensions@4.15",
            },
        )
        self.assertEqual(blockers, set())

    def test_review_compares_components_and_refuses_unowned_or_unreviewed_files(self) -> None:
        """Every runtime byte needs environment ownership and reviewed licence evidence."""
        environment_components = {
            "python:torch@2.10": {
                "licenseExpression": "BSD-3-Clause",
                "licenseFiles": [{"relativeName": "LICENSE", "byteSize": 5, "sha256": SHA256}],
            },
            "deb:libc6@2.39": {
                "licenseExpression": "Debian-copyright",
                "licenseFiles": [{"relativeName": "copyright", "byteSize": 5, "sha256": SHA256}],
            },
        }
        files = [
            {
                "sha256": "b" * 64,
                "byteSize": 10,
                "owner": "first-party:bottie-worker",
                "elf": False,
            },
            {
                "sha256": "c" * 64,
                "byteSize": 20,
                "owner": "python:torch@2.10",
                "elf": True,
            },
            {
                "sha256": "d" * 64,
                "byteSize": 30,
                "owner": None,
                "elf": False,
            },
        ]

        review = build_closure_review(
            files,
            environment_components,
            host_driver_sonames={"libcuda.so.1"},
            observed_host_driver_sonames={"libcuda.so.1"},
            missing_elf_dependencies={"libmissing.so"},
            dependency_blockers={"elf-soname:libexample.so.1:ambiguous"},
        )

        self.assertEqual(review["closure"]["fileCount"], 3)
        self.assertEqual(review["closure"]["componentCount"], 1)
        self.assertEqual(review["hostDriverBoundary"]["observedSonames"], ["libcuda.so.1"])
        self.assertIn("elf-dependency:libmissing.so:unresolved", review["blockers"])
        self.assertIn("elf-soname:libexample.so.1:ambiguous", review["blockers"])
        self.assertIn(f"unowned-file:{'d' * 64}", review["blockers"])
        self.assertIn("python:torch@2.10:unreviewed-license-expression", review["blockers"])
        self.assertFalse(review["closureComplete"])
        self.assertFalse(review["licenseReviewed"])
        self.assertFalse(review["assemblyEligible"])
        self.assertFalse(review["distributionReviewed"])

    def test_reviewed_component_uses_exact_manifest_bytes_and_clears_licence_blockers(self) -> None:
        """Validated source and review bytes replace missing or undeclared package metadata."""
        review = build_closure_review(
            [
                {
                    "sha256": "b" * 64,
                    "byteSize": 10,
                    "owner": "python:example@1",
                    "elf": False,
                }
            ],
            {
                "python:example@1": {
                    "licenseExpression": "undeclared",
                    "licenseFiles": [],
                }
            },
            set(),
            set(),
            set(),
            license_review_components={
                "python:example@1": {
                    "identity": "python:example@1",
                    "reviewedLicenseExpression": "MIT",
                    "licenseFiles": [
                        {"relativeName": "example/LICENSE", "byteSize": 12, "sha256": SHA256}
                    ],
                    "reviewEvidence": {"byteSize": 42, "sha256": "b" * 64},
                }
            },
        )

        component = review["closure"]["components"][0]
        self.assertEqual(component["reviewedLicenseExpression"], "MIT")
        self.assertEqual(component["licenseFileCount"], 1)
        self.assertEqual(component["reviewEvidence"]["sha256"], "b" * 64)
        self.assertEqual(review["blockers"], [])
        self.assertTrue(review["licenseReviewed"])
        self.assertTrue(review["assemblyEligible"])

    def test_review_rejects_components_absent_from_the_complete_environment(self) -> None:
        """A claimed package owner must exist in the separately collected environment record."""
        files = [
            {
                "sha256": SHA256,
                "byteSize": 10,
                "owner": "python:invented@1",
                "elf": False,
            }
        ]

        with self.assertRaisesRegex(ClosureEvidenceError, "complete environment"):
            build_closure_review(files, {}, set(), set(), set())

    def test_mapped_alias_upgrades_a_deduplicated_driver_file(self) -> None:
        """A host-mapped alias must win even when an image alias was measured first."""
        image_alias = Path("/image/libcuda.so.1")
        mapped_alias = Path("/host/libcuda.so.580")
        measured = {
            "resolvedPath": Path("/resolved/libcuda.so.580"),
            "sha256": SHA256,
            "byteSize": 10,
            "elf": True,
        }
        environment = {"pythonComponents": [], "nativeComponents": []}
        context = {
            "imageId": "ignored-by-this-test",
            "workerVersion": "worker",
            "runtimeId": "runtime",
            "modelId": "model",
            "modelRevision": "revision",
            "pythonTraceSha256": SHA256,
            "processMapsSha256": SHA256,
        }

        with (
            patch.object(runtime_closure, "FIRST_PARTY_FILES", set()),
            patch.object(runtime_closure, "_verify_environment_contents"),
            patch.object(runtime_closure, "verify_proof_inputs"),
            patch.object(runtime_closure, "_load_trace_context", return_value=context),
            patch.object(runtime_closure, "_read_bounded_text", return_value=""),
            patch.object(runtime_closure, "parse_trace_paths", return_value=[image_alias]),
            patch.object(runtime_closure, "parse_process_maps", return_value=[mapped_alias]),
            patch.object(runtime_closure, "_collect_environment_measurement", return_value=environment),
            patch.object(runtime_closure, "python_file_owners", return_value={}),
            patch.object(runtime_closure, "native_file_owners", return_value={}),
            patch.object(runtime_closure, "verified_native_components", return_value=({}, {})),
            patch.object(runtime_closure, "file_owner", return_value="deb:image-driver@1"),
            patch.object(runtime_closure, "_measure_observed_file", return_value=measured),
            patch.object(
                runtime_closure,
                "_read_elf_dependencies",
                return_value=(set(), "libcuda.so.1"),
            ),
            patch.object(runtime_closure, "installed_python_requirements", return_value=(set(), set())),
        ):
            review = runtime_closure.collect_runtime_closure(Path("/trace"))

        self.assertEqual(review["hostDriverBoundary"]["observedSonames"], ["libcuda.so.1"])
        self.assertEqual(review["closure"]["componentCount"], 0)
        self.assertNotIn("deb:image-driver@1", review["blockers"])

    def test_host_collector_injects_the_driver_boundary_without_network(self) -> None:
        """The classifier must see proof-time driver files but retain network isolation."""
        completed = SimpleNamespace(returncode=0, stdout='{"closure": {}}', stderr="")
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(runtime_closure_host, "_inspect_derived_image", return_value="sha256:id"),
            patch.object(runtime_closure_host.subprocess, "run", return_value=completed) as run,
        ):
            runtime_closure_host.collect_verified_runtime_closure("proof-image", Path(directory))

        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--gpus") + 1], "all")
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--read-only", command)
        self.assertEqual(command[command.index("--cap-drop") + 1], "ALL")
        self.assertEqual(command[command.index("--security-opt") + 1], "no-new-privileges")
        expected_user = f"{runtime_closure_host.os.getuid()}:{runtime_closure_host.os.getgid()}"
        self.assertEqual(command[command.index("--user") + 1], expected_user)

    def test_host_collector_mounts_an_explicit_review_read_only(self) -> None:
        """The optional review enters the exact image as a read-only file, never caller environment text."""
        completed = SimpleNamespace(returncode=0, stdout='{"closure": {}}', stderr="")
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(runtime_closure_host, "_inspect_derived_image", return_value="sha256:id"),
            patch.object(runtime_closure_host.subprocess, "run", return_value=completed) as run,
        ):
            review = Path(directory) / "review.json"
            review.write_text("{}", encoding="utf-8")
            runtime_closure_host.collect_verified_runtime_closure(
                "proof-image",
                Path(directory),
                review,
            )

        command = run.call_args.args[0]
        review_mount = next(value for value in command if "bottie-license-review.json" in value)
        self.assertIn(str(review.resolve()), review_mount)
        self.assertIn("readonly", review_mount)
        self.assertEqual(command[command.index("--license-review") + 1], "/opt/bottie-license-review.json")

    def test_host_collector_mounts_external_license_sources_read_only(self) -> None:
        """Authoritative archives enter the verified image through one explicit offline directory."""
        completed = SimpleNamespace(returncode=0, stdout='{"closure": {}}', stderr="")
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(runtime_closure_host, "_inspect_derived_image", return_value="sha256:id"),
            patch.object(runtime_closure_host.subprocess, "run", return_value=completed) as run,
        ):
            root = Path(directory)
            trace = root / "trace"
            sources = root / "sources"
            trace.mkdir()
            sources.mkdir()
            runtime_closure_host.collect_verified_runtime_closure(
                "proof-image",
                trace,
                license_sources=sources,
            )

        command = run.call_args.args[0]
        source_mount = next(value for value in command if "bottie-license-sources" in value)
        self.assertIn(str(sources.resolve()), source_mount)
        self.assertIn("readonly", source_mount)
        self.assertEqual(
            command[command.index("--license-sources") + 1],
            "/opt/bottie-license-sources",
        )

    def test_external_sources_clear_only_missing_byte_blockers(self) -> None:
        """Source archives add measured bytes while undeclared and unreviewed gates remain closed."""
        observed_path = Path("/runtime/example.py")
        environment = {
            "pythonComponents": [
                {
                    "ecosystem": "python",
                    "name": "example",
                    "version": "1",
                    "licenseExpression": "undeclared",
                    "licenseFiles": [],
                }
            ],
            "nativeComponents": [],
        }
        context = {
            "imageId": "ignored-by-this-test",
            "workerVersion": "worker",
            "runtimeId": "runtime",
            "modelId": "model",
            "modelRevision": "revision",
            "pythonTraceSha256": SHA256,
            "processMapsSha256": SHA256,
        }
        sources = {
            "python:example@1": {
                "licenseFiles": [
                    {"relativeName": "upstream/example/LICENSE", "byteSize": 5, "sha256": SHA256}
                ],
                "provenance": {"kind": "authoritative-source-archive"},
            }
        }

        with (
            patch.object(runtime_closure, "FIRST_PARTY_FILES", set()),
            patch.object(runtime_closure, "_verify_environment_contents"),
            patch.object(runtime_closure, "verify_proof_inputs"),
            patch.object(runtime_closure, "_load_trace_context", return_value=context),
            patch.object(runtime_closure, "_read_bounded_text", return_value=""),
            patch.object(runtime_closure, "parse_trace_paths", return_value=[observed_path]),
            patch.object(runtime_closure, "parse_process_maps", return_value=[]),
            patch.object(runtime_closure, "_collect_environment_measurement", return_value=environment),
            patch.object(runtime_closure, "python_file_owners", return_value={}),
            patch.object(runtime_closure, "native_file_owners", return_value={}),
            patch.object(runtime_closure, "verified_native_components", return_value=({}, {})),
            patch.object(runtime_closure, "file_owner", return_value="python:example@1"),
            patch.object(
                runtime_closure,
                "_measure_observed_file",
                return_value={
                    "resolvedPath": observed_path,
                    "sha256": SHA256,
                    "byteSize": 10,
                    "elf": False,
                },
            ),
            patch.object(runtime_closure, "installed_python_requirements", return_value=(set(), set())),
            patch.object(runtime_closure, "verified_external_license_sources", return_value=sources),
        ):
            review = runtime_closure.collect_runtime_closure(
                Path("/trace"),
                license_source_root=Path("/sources"),
            )

        self.assertNotIn("python:example@1:missing-license-bytes", review["blockers"])
        self.assertIn("python:example@1:undeclared-license", review["blockers"])
        self.assertIn("python:example@1:unreviewed-license-expression", review["blockers"])
        self.assertEqual(review["closure"]["components"][0]["licenseFileCount"], 1)
        self.assertFalse(review["licenseReviewed"])
        self.assertFalse(review["assemblyEligible"])


if __name__ == "__main__":
    unittest.main()
