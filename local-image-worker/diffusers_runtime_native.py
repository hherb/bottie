"""Bind unmanaged native runtime files to exact in-image version evidence."""

from __future__ import annotations

import hashlib
import re
import stat
from dataclasses import dataclass
from pathlib import Path


HPCX_MARKER = Path("/opt/hpcx/VERSION")
CUSPARSELT_MARKER = Path(
    "/usr/local/cuda-13.0/targets/sbsa-linux/include/cusparseLt.h"
)
NVPL_BLAS_MARKER = Path("/usr/local/include/nvpl_blas_version.h")
NVPL_LAPACK_MARKER = Path("/usr/local/include/nvpl_lapack_version.h")
NEEDED_PATTERN = re.compile(r"\(NEEDED\).*\[([^\]]+)\]")
SONAME_PATTERN = re.compile(r"\(SONAME\).*\[([^\]]+)\]")


class RuntimeNativeEvidenceError(RuntimeError):
    """Stable failure raised when unmanaged native provenance is absent or changed."""


@dataclass(frozen=True)
class NativeComponentSpec:
    """Describe one exact native component, authoritative marker, and owned files."""

    name: str
    version: str
    marker: Path
    marker_byte_size: int
    marker_sha256: str
    required_marker_fragments: tuple[str, ...]
    owned_files: tuple[Path, ...]


NATIVE_COMPONENT_SPECS = (
    NativeComponentSpec(
        name="hpcx-open-mpi",
        version="92f9fca4eb832bb874928d25485be6e007c62e31",
        marker=HPCX_MARKER,
        marker_byte_size=530,
        marker_sha256="12f67f55bcfd8989111d49611cdb76ade93683ea7fbd5d2bacc9f0fb289143e2",
        required_marker_fragments=(
            "HPC-X v2.24.1",
            "ompi-92f9fca4eb832bb874928d25485be6e007c62e31  gitclone (92f9fca)",
        ),
        owned_files=(
            Path("/opt/hpcx/ompi/lib/libmpi.so.40.30.8"),
            Path("/opt/hpcx/ompi/lib/libopen-pal.so.40.30.4"),
            Path("/opt/hpcx/ompi/lib/libopen-rte.so.40.30.4"),
        ),
    ),
    NativeComponentSpec(
        name="hpcx-ucc",
        version="1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e",
        marker=HPCX_MARKER,
        marker_byte_size=530,
        marker_sha256="12f67f55bcfd8989111d49611cdb76ade93683ea7fbd5d2bacc9f0fb289143e2",
        required_marker_fragments=(
            "HPC-X v2.24.1",
            "ucc-ec95a0a96fc7220e1627157439c508cafc82274e  1.5.0 (ec95a0a)",
        ),
        owned_files=(Path("/opt/hpcx/ucc/lib/libucc.so.1.0.0"),),
    ),
    NativeComponentSpec(
        name="hpcx-ucx",
        version="1.20.0+777ac8418278502c5639be25377d3f626bfe7dc7",
        marker=HPCX_MARKER,
        marker_byte_size=530,
        marker_sha256="12f67f55bcfd8989111d49611cdb76ade93683ea7fbd5d2bacc9f0fb289143e2",
        required_marker_fragments=(
            "HPC-X v2.24.1",
            "ucx-777ac8418278502c5639be25377d3f626bfe7dc7  1.20.0 (777ac84)",
        ),
        owned_files=(
            Path("/opt/hpcx/ucx/lib/libucm.so.0.0.0"),
            Path("/opt/hpcx/ucx/lib/libucp.so.0.0.0"),
            Path("/opt/hpcx/ucx/lib/libucs.so.0.0.0"),
            Path("/opt/hpcx/ucx/lib/libuct.so.0.0.0"),
            Path("/opt/hpcx/ucx/lib/ucx/libucs_fuse.so.0.0.0"),
        ),
    ),
    NativeComponentSpec(
        name="nvidia-cusparselt",
        version="0.8.1",
        marker=CUSPARSELT_MARKER,
        marker_byte_size=18_157,
        marker_sha256="0df84b580688771d78be62bceebc408867a161c641fe0bca8a5075ba043b63b3",
        required_marker_fragments=(
            "#define CUSPARSELT_VER_MAJOR 0",
            "#define CUSPARSELT_VER_MINOR 8",
            "#define CUSPARSELT_VER_PATCH 1",
        ),
        owned_files=(
            Path("/usr/local/cuda-13.0/targets/sbsa-linux/lib/libcusparseLt.so.0.8.1.1"),
        ),
    ),
    NativeComponentSpec(
        name="nvidia-nvpl-blas",
        version="0.2.0",
        marker=NVPL_BLAS_MARKER,
        marker_byte_size=167,
        marker_sha256="1ced82338f50c7ba19fae815634789122cb3d9b4f69d5a0fdc39e1ab5ef3434f",
        required_marker_fragments=(
            "#define NVPL_BLAS_VERSION_MAJOR 0",
            "#define NVPL_BLAS_VERSION_MINOR 2",
            "#define NVPL_BLAS_VERSION_PATCH 0",
        ),
        owned_files=(
            Path("/usr/local/lib/libnvpl_blas_core.so.0.2.0"),
            Path("/usr/local/lib/libnvpl_blas_lp64_gomp.so.0.2.0"),
        ),
    ),
    NativeComponentSpec(
        name="nvidia-nvpl-lapack",
        version="0.2.2",
        marker=NVPL_LAPACK_MARKER,
        marker_byte_size=177,
        marker_sha256="6b474db285b0292797c5058f5b5cbf18b25c74bc38817821e7dad337ac080d9e",
        required_marker_fragments=(
            "#define NVPL_LAPACK_VERSION_MAJOR 0",
            "#define NVPL_LAPACK_VERSION_MINOR 2",
            "#define NVPL_LAPACK_VERSION_PATCH 2",
        ),
        owned_files=(
            Path("/usr/local/lib/libnvpl_lapack_core.so.0.2.2"),
            Path("/usr/local/lib/libnvpl_lapack_lp64_gomp.so.0.2.2"),
        ),
    ),
)


def verified_native_components(
    specs: tuple[NativeComponentSpec, ...] = NATIVE_COMPONENT_SPECS,
) -> tuple[dict[str, dict], dict[Path, set[str]]]:
    """Return exact component evidence and file owners after verifying every marker."""
    components = {}
    owners = {}
    for spec in specs:
        marker_bytes = _stable_marker_bytes(spec.marker)
        marker_sha256 = hashlib.sha256(marker_bytes).hexdigest()
        try:
            marker_text = marker_bytes.decode("utf-8")
        except UnicodeError as error:
            raise RuntimeNativeEvidenceError("native version marker is not UTF-8") from error
        if (
            len(marker_bytes) != spec.marker_byte_size
            or marker_sha256 != spec.marker_sha256
            or any(fragment not in marker_text for fragment in spec.required_marker_fragments)
        ):
            raise RuntimeNativeEvidenceError("native version marker has drifted")
        identity = f"native:{spec.name}@{spec.version}"
        if identity in components:
            raise RuntimeNativeEvidenceError("native component identity is duplicated")
        components[identity] = {
            "ecosystem": "native",
            "name": spec.name,
            "version": spec.version,
            "licenseExpression": "undeclared",
            "licenseFiles": [],
            "provenance": {
                "kind": "image-version-marker",
                "byteSize": len(marker_bytes),
                "sha256": marker_sha256,
            },
        }
        for path in spec.owned_files:
            try:
                resolved = path.resolve(strict=True)
            except OSError as error:
                raise RuntimeNativeEvidenceError("native owned file is unavailable") from error
            if not resolved.is_file() or resolved in owners:
                raise RuntimeNativeEvidenceError("native owned file is invalid or ambiguous")
            owners[resolved] = {identity}
    return components, owners


def ambiguous_elf_dependency_blockers(
    needed_names: set[str],
    providers: dict[str, set[str]],
) -> set[str]:
    """Report only requested ELF names that resolve to multiple distinct byte identities."""
    return {
        f"elf-soname:{name}:ambiguous"
        for name in needed_names
        if len(providers.get(name, set())) > 1
    }


def parse_elf_dependencies(output: str) -> tuple[set[str], str | None]:
    """Extract exact NEEDED names and an optional SONAME from readelf output."""
    needed = set()
    soname = None
    for line in output.splitlines():
        needed_match = NEEDED_PATTERN.search(line)
        if needed_match:
            needed.add(validated_soname(needed_match.group(1)))
        soname_match = SONAME_PATTERN.search(line)
        if soname_match:
            candidate = validated_soname(soname_match.group(1))
            if soname is not None and soname != candidate:
                raise RuntimeNativeEvidenceError("ELF file declares multiple SONAME values")
            soname = candidate
    return needed, soname


def host_driver_soname(
    file_name: str,
    declared_soname: str | None,
    allowed_sonames: set[str] | frozenset[str],
) -> str | None:
    """Return the exact allowlisted NVIDIA interface named by a mapped ELF file."""
    for candidate in (declared_soname, file_name):
        if candidate in allowed_sonames:
            return candidate
    return None


def validated_soname(value: object) -> str:
    """Validate one bounded filename-only ELF dependency identity."""
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode()) > 256
        or "/" in value
        or "\\" in value
        or any(ord(character) < 32 for character in value)
    ):
        raise RuntimeNativeEvidenceError("ELF dependency name is invalid")
    return value


def _stable_marker_bytes(path: Path) -> bytes:
    """Read one regular version marker while detecting replacement or mutation."""
    try:
        before = path.stat()
        contents = path.read_bytes()
        after = path.stat()
    except OSError as error:
        raise RuntimeNativeEvidenceError("native version marker is unavailable") from error
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    observed = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if stable != observed or not stat.S_ISREG(before.st_mode):
        raise RuntimeNativeEvidenceError("native version marker is unstable")
    return contents
