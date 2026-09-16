"""Tests for authoritative unmanaged-native runtime ownership."""

from __future__ import annotations

import hashlib
import io
import tarfile
import tempfile
import unittest
from pathlib import Path

from diffusers_runtime_native import (
    NativeArchiveLicenseSourceSpec,
    NativeComponentSpec,
    NativePackageLicenseSourceSpec,
    RuntimeNativeEvidenceError,
    ambiguous_elf_dependency_blockers,
    verified_native_components,
)


class DiffusersRuntimeNativeTests(unittest.TestCase):
    """Protect exact marker-backed ownership and requested ELF identities."""

    def test_verified_marker_bytes_bind_exact_native_file_owners(self) -> None:
        """Unmanaged files receive one owner only through an exact version marker."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "VERSION"
            first = root / "libfirst.so.1"
            second = root / "libsecond.so.1"
            marker_bytes = b"Example Runtime 1.2.3\nrevision abc123\n"
            marker.write_bytes(marker_bytes)
            first.write_bytes(b"first")
            second.write_bytes(b"second")
            spec = NativeComponentSpec(
                name="example-runtime",
                version="1.2.3+abc123",
                marker=marker,
                marker_byte_size=len(marker_bytes),
                marker_sha256=hashlib.sha256(marker_bytes).hexdigest(),
                required_marker_fragments=("Example Runtime 1.2.3", "revision abc123"),
                owned_files=(first, second),
            )

            components, owners = verified_native_components((spec,))

            identity = "native:example-runtime@1.2.3+abc123"
            self.assertEqual(
                components[identity],
                {
                    "ecosystem": "native",
                    "name": "example-runtime",
                    "version": "1.2.3+abc123",
                    "licenseExpression": "undeclared",
                    "licenseFiles": [],
                    "provenance": {
                        "kind": "image-version-marker",
                        "byteSize": len(marker_bytes),
                        "sha256": hashlib.sha256(marker_bytes).hexdigest(),
                    },
                },
            )
            self.assertEqual(
                owners,
                {first.resolve(): {identity}, second.resolve(): {identity}},
            )

            marker.write_text("Example Runtime 1.2.4\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeNativeEvidenceError, "marker"):
                verified_native_components((spec,))

    def test_elf_ambiguity_requires_an_actual_needed_edge(self) -> None:
        """Unrequested extension-module basenames do not masquerade as linker conflicts."""
        providers = {
            "_C.cpython-312-aarch64-linux-gnu.so": {"a", "b"},
            "librequired.so.1": {"c", "d"},
        }

        self.assertEqual(
            ambiguous_elf_dependency_blockers({"librequired.so.1"}, providers),
            {"elf-soname:librequired.so.1:ambiguous"},
        )

    def test_exact_package_evidence_binds_license_bytes_without_inferring_expression(self) -> None:
        """A marker-backed native identity may reuse only one exact measured package document."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "VERSION"
            runtime = root / "libruntime.so.1"
            marker_bytes = b"Example Runtime 1.2.3\n"
            marker.write_bytes(marker_bytes)
            runtime.write_bytes(b"runtime")
            license_sha256 = "b" * 64
            source_identity = "deb:example-runtime@1.2.3-1"
            spec = NativeComponentSpec(
                name="example-runtime",
                version="1.2.3",
                marker=marker,
                marker_byte_size=len(marker_bytes),
                marker_sha256=hashlib.sha256(marker_bytes).hexdigest(),
                required_marker_fragments=("Example Runtime 1.2.3",),
                owned_files=(runtime,),
                license_source=NativePackageLicenseSourceSpec(
                    component_identity=source_identity,
                    source_relative_name="copyright",
                    evidence_relative_name="deb/example-runtime/copyright",
                    byte_size=42,
                    sha256=license_sha256,
                ),
            )
            sources = {
                source_identity: {
                    "licenseExpression": "Debian-copyright",
                    "licenseFiles": [
                        {
                            "relativeName": "copyright",
                            "byteSize": 42,
                            "sha256": license_sha256,
                        }
                    ],
                }
            }

            components, _ = verified_native_components((spec,), sources)

            component = components["native:example-runtime@1.2.3"]
            self.assertEqual(component["licenseExpression"], "undeclared")
            self.assertEqual(
                component["licenseFiles"],
                [
                    {
                        "relativeName": "deb/example-runtime/copyright",
                        "byteSize": 42,
                        "sha256": license_sha256,
                    }
                ],
            )
            self.assertEqual(
                component["provenance"]["licenseSource"],
                {
                    "kind": "environment-package-license",
                    "componentIdentity": source_identity,
                    "relativeName": "copyright",
                    "byteSize": 42,
                    "sha256": license_sha256,
                },
            )

            with self.assertRaisesRegex(RuntimeNativeEvidenceError, "licence source"):
                verified_native_components((spec,), {})

    def test_exact_source_archive_binds_one_regular_license_member(self) -> None:
        """An in-image source archive contributes only an exact measured regular member."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "VERSION"
            runtime = root / "libruntime.so.1"
            archive = root / "runtime-sources.tar.gz"
            marker_bytes = b"Example Runtime 2.0\n"
            license_bytes = b"Example licence bytes\n"
            marker.write_bytes(marker_bytes)
            runtime.write_bytes(b"runtime")
            with tarfile.open(archive, "w:gz") as bundle:
                member = tarfile.TarInfo("runtime-2.0/LICENSE")
                member.size = len(license_bytes)
                bundle.addfile(member, io.BytesIO(license_bytes))
            archive_bytes = archive.read_bytes()
            spec = NativeComponentSpec(
                name="example-runtime",
                version="2.0",
                marker=marker,
                marker_byte_size=len(marker_bytes),
                marker_sha256=hashlib.sha256(marker_bytes).hexdigest(),
                required_marker_fragments=("Example Runtime 2.0",),
                owned_files=(runtime,),
                license_source=NativeArchiveLicenseSourceSpec(
                    archive=archive,
                    archive_relative_name="sources/runtime-sources.tar.gz",
                    archive_byte_size=len(archive_bytes),
                    archive_sha256=hashlib.sha256(archive_bytes).hexdigest(),
                    member_name="runtime-2.0/LICENSE",
                    evidence_relative_name="sources/runtime-2.0/LICENSE",
                    byte_size=len(license_bytes),
                    sha256=hashlib.sha256(license_bytes).hexdigest(),
                ),
            )

            components, _ = verified_native_components((spec,), {})

            component = components["native:example-runtime@2.0"]
            self.assertEqual(component["licenseFiles"][0]["byteSize"], len(license_bytes))
            self.assertEqual(
                component["provenance"]["licenseSource"],
                {
                    "kind": "image-source-archive-license",
                    "archiveName": "sources/runtime-sources.tar.gz",
                    "archiveByteSize": len(archive_bytes),
                    "archiveSha256": hashlib.sha256(archive_bytes).hexdigest(),
                    "memberName": "runtime-2.0/LICENSE",
                    "byteSize": len(license_bytes),
                    "sha256": hashlib.sha256(license_bytes).hexdigest(),
                },
            )

            archive.write_bytes(archive_bytes + b"drift")
            with self.assertRaisesRegex(RuntimeNativeEvidenceError, "archive"):
                verified_native_components((spec,), {})


if __name__ == "__main__":
    unittest.main()
