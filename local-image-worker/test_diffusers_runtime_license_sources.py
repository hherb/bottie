"""Tests for exact external licence-source archives used by the runtime closure."""

from __future__ import annotations

import hashlib
import io
import tarfile
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from diffusers_runtime_license_sources import (
    EXTERNAL_LICENSE_SOURCE_SPECS,
    ExternalLicenseSourceError,
    ExternalLicenseSourceSpec,
    RuntimeArchiveMemberSpec,
    apply_external_license_sources,
    verified_external_license_sources,
)


LICENSE_BYTES = b"Exact licence bytes\n"
RUNTIME_BYTES = b"exact runtime bytes"


def _write_archive(path: Path, members: list[tuple[str, bytes]]) -> bytes:
    """Write one deterministic gzip tar fixture and return its exact bytes."""
    with tarfile.open(path, "w:gz") as archive:
        for name, contents in members:
            member = tarfile.TarInfo(name)
            member.size = len(contents)
            member.mtime = 0
            archive.addfile(member, io.BytesIO(contents))
    return path.read_bytes()


def _source_spec(root: Path, archive_bytes: bytes) -> ExternalLicenseSourceSpec:
    """Return a source spec bound to the synthetic archive and runtime file."""
    return ExternalLicenseSourceSpec(
        component_identity="native:example@1.2.3",
        archive_name="example-1.2.3.tar.gz",
        archive_byte_size=len(archive_bytes),
        archive_sha256=hashlib.sha256(archive_bytes).hexdigest(),
        source_url="https://authoritative.example/example-1.2.3.tar.gz",
        member_name="example-1.2.3/LICENSE",
        evidence_relative_name="upstream/example-1.2.3/LICENSE",
        byte_size=len(LICENSE_BYTES),
        sha256=hashlib.sha256(LICENSE_BYTES).hexdigest(),
        runtime_members=(
            RuntimeArchiveMemberSpec(
                runtime_path=root / "libexample.so.1.2.3",
                member_name="example-1.2.3/lib/libexample.so.1.2.3",
            ),
        ),
    )


class DiffusersRuntimeLicenseSourceTests(unittest.TestCase):
    """Protect authoritative archive, member, and runtime-byte bindings."""

    def test_catalog_covers_only_the_four_resolved_exact_identities(self) -> None:
        """The unresolved HPC-X UCC revision is not replaced by nearby source bytes."""
        self.assertEqual(
            {spec.component_identity for spec in EXTERNAL_LICENSE_SOURCE_SPECS},
            {
                "native:nvidia-nvpl-blas@0.2.0",
                "native:nvidia-nvpl-lapack@0.2.2",
                "python:sentencepiece@0.2.2",
                "python:tokenizers@0.23.2",
            },
        )
        self.assertTrue(
            all(spec.source_url.startswith("https://") for spec in EXTERNAL_LICENSE_SOURCE_SPECS)
        )
        self.assertNotIn(
            "native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e",
            {spec.component_identity for spec in EXTERNAL_LICENSE_SOURCE_SPECS},
        )

    def test_binds_exact_archive_member_and_matching_runtime_bytes(self) -> None:
        """A fixed archive contributes only its measured licence after runtime matching."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "libexample.so.1.2.3"
            runtime.write_bytes(RUNTIME_BYTES)
            archive = root / "example-1.2.3.tar.gz"
            archive_bytes = _write_archive(
                archive,
                [
                    ("example-1.2.3/LICENSE", LICENSE_BYTES),
                    ("example-1.2.3/lib/libexample.so.1.2.3", RUNTIME_BYTES),
                ],
            )

            sources = verified_external_license_sources(root, (_source_spec(root, archive_bytes),))

            source = sources["native:example@1.2.3"]
            self.assertEqual(
                source["licenseFiles"],
                [
                    {
                        "relativeName": "upstream/example-1.2.3/LICENSE",
                        "byteSize": len(LICENSE_BYTES),
                        "sha256": hashlib.sha256(LICENSE_BYTES).hexdigest(),
                    }
                ],
            )
            self.assertEqual(source["provenance"]["kind"], "authoritative-source-archive")
            self.assertEqual(source["provenance"]["runtimeMemberCount"], 1)
            self.assertNotIn(str(root), str(source))
            self.assertNotIn("licenseExpression", source)

    def test_skips_absent_known_archives_but_rejects_all_unrecognized_inputs(self) -> None:
        """Partial evidence stays partial, while a supplied empty source root fails closed."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            spec = ExternalLicenseSourceSpec(
                component_identity="python:example@1",
                archive_name="example-1.tar.gz",
                archive_byte_size=1,
                archive_sha256="a" * 64,
                source_url="https://authoritative.example/example-1.tar.gz",
                member_name="example-1/LICENSE",
                evidence_relative_name="upstream/example-1/LICENSE",
                byte_size=1,
                sha256="b" * 64,
            )

            with self.assertRaisesRegex(ExternalLicenseSourceError, "recognized"):
                verified_external_license_sources(root, (spec,))

            for invalid in (
                replace(spec, archive_sha256="A" * 64),
                replace(spec, member_name="../LICENSE"),
                replace(spec, evidence_relative_name="/absolute/LICENSE"),
                replace(
                    spec,
                    runtime_members=(
                        RuntimeArchiveMemberSpec(
                            runtime_path=Path("relative.so"),
                            member_name="example/libexample.so",
                        ),
                    ),
                ),
            ):
                with self.subTest(invalid=invalid):
                    with self.assertRaisesRegex(ExternalLicenseSourceError, "specification"):
                        verified_external_license_sources(root, (invalid,))

    def test_rejects_archive_member_or_runtime_drift(self) -> None:
        """Changed archives, licence members, and installed runtime bytes fail closed."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "libexample.so.1.2.3"
            runtime.write_bytes(RUNTIME_BYTES)
            archive = root / "example-1.2.3.tar.gz"
            archive_bytes = _write_archive(
                archive,
                [
                    ("example-1.2.3/LICENSE", LICENSE_BYTES),
                    ("example-1.2.3/lib/libexample.so.1.2.3", RUNTIME_BYTES),
                ],
            )
            spec = _source_spec(root, archive_bytes)

            archive.write_bytes(archive_bytes + b"drift")
            with self.assertRaisesRegex(ExternalLicenseSourceError, "archive"):
                verified_external_license_sources(root, (spec,))

            archive.write_bytes(archive_bytes)
            runtime.write_bytes(b"changed")
            with self.assertRaisesRegex(ExternalLicenseSourceError, "runtime member"):
                verified_external_license_sources(root, (spec,))

    def test_rejects_duplicate_target_members_and_source_symlinks(self) -> None:
        """Ambiguous members and links cannot stand in for one exact regular source archive."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "libexample.so.1.2.3"
            runtime.write_bytes(RUNTIME_BYTES)
            archive = root / "example-1.2.3.tar.gz"
            archive_bytes = _write_archive(
                archive,
                [
                    ("example-1.2.3/LICENSE", LICENSE_BYTES),
                    ("example-1.2.3/LICENSE", LICENSE_BYTES),
                    ("example-1.2.3/lib/libexample.so.1.2.3", RUNTIME_BYTES),
                ],
            )
            spec = _source_spec(root, archive_bytes)

            with self.assertRaisesRegex(ExternalLicenseSourceError, "member"):
                verified_external_license_sources(root, (spec,))

            archive.unlink()
            archive.symlink_to(root / "elsewhere.tar.gz")
            with self.assertRaisesRegex(ExternalLicenseSourceError, "archive"):
                verified_external_license_sources(root, (spec,))

    def test_applies_sources_without_inheriting_or_replacing_an_expression(self) -> None:
        """External bytes enrich only an exact source-less component record."""
        components = {
            "python:example@1": {
                "licenseExpression": "undeclared",
                "licenseFiles": [],
            }
        }
        sources = {
            "python:example@1": {
                "licenseFiles": [
                    {
                        "relativeName": "upstream/example/LICENSE",
                        "byteSize": len(LICENSE_BYTES),
                        "sha256": hashlib.sha256(LICENSE_BYTES).hexdigest(),
                    }
                ],
                "provenance": {"kind": "authoritative-source-archive"},
            }
        }

        enriched = apply_external_license_sources(components, sources)

        self.assertEqual(enriched["python:example@1"]["licenseExpression"], "undeclared")
        self.assertEqual(enriched["python:example@1"]["licenseFiles"], sources["python:example@1"]["licenseFiles"])
        self.assertEqual(
            enriched["python:example@1"]["provenance"]["licenseSource"],
            sources["python:example@1"]["provenance"],
        )
        self.assertEqual(components["python:example@1"]["licenseFiles"], [])

        with self.assertRaisesRegex(ExternalLicenseSourceError, "absent"):
            apply_external_license_sources({}, sources)
        existing = {
            "python:example@1": {
                "licenseExpression": "MIT",
                "licenseFiles": [{"relativeName": "LICENSE"}],
            }
        }
        with self.assertRaisesRegex(ExternalLicenseSourceError, "already has"):
            apply_external_license_sources(existing, sources)


if __name__ == "__main__":
    unittest.main()
