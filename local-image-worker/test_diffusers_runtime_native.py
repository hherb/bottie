"""Tests for authoritative unmanaged-native runtime ownership."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from diffusers_runtime_native import (
    NativeComponentSpec,
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


if __name__ == "__main__":
    unittest.main()
