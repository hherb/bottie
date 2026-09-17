"""Validate the frozen clean Linux runtime before closure classification."""

from __future__ import annotations

import hashlib
import importlib.metadata as metadata
import os
import platform
import stat
import struct
from collections import defaultdict
from pathlib import Path

from packaging.markers import default_environment
from packaging.utils import canonicalize_name
from packaging.version import InvalidVersion, Version

from diffusers_bundle_environment import (
    _installed_debian_packages,
    _native_component,
    _python_component,
)
from diffusers_clean_runtime_lock import (
    CLEAN_BASE_IMAGE_DIGEST,
    CLEAN_PROOF_IMAGE_DIGEST,
    EXPECTED_DEBIAN_ARTIFACTS,
    EXPECTED_PYTHON_ARTIFACTS,
    TARGET_PYTHON_VERSION,
    CleanRuntimeLockError,
    _load_manifest,
    _validate_debian_records,
    _validate_lock_digest,
    _validate_manifest_header,
    _validate_python_records,
)
from diffusers_pytorch_worker import PYTORCH_WORKER_IDENTITY
from diffusers_runtime_dependencies import (
    RuntimeDependencyError,
    resolve_required_python_components,
)
from diffusers_runtime_ownership import RuntimeOwnershipError


CLEAN_SOURCE_IMAGE_DIGEST = CLEAN_PROOF_IMAGE_DIGEST
CLEAN_PYTHON_VERSION = TARGET_PYTHON_VERSION
CLEAN_RUNTIME_ID = PYTORCH_WORKER_IDENTITY.runtime_id
HASH_BUFFER_BYTES = 1024 * 1024
ELF_HEADER_BYTES = 64
ELF_PROGRAM_HEADER_BYTES = 56
ELF_DYNAMIC_ENTRY_BYTES = 16
ELF_MACHINE_AARCH64 = 183
ELF_PT_LOAD = 1
ELF_PT_DYNAMIC = 2
ELF_DT_NULL = 0
ELF_DT_NEEDED = 1
ELF_DT_STRTAB = 5
ELF_DT_STRSZ = 10
ELF_DT_SONAME = 14
MAX_ELF_PROGRAM_HEADERS = 128
MAX_ELF_DYNAMIC_BYTES = 4 * 1024 * 1024
MAX_ELF_STRING_TABLE_BYTES = 16 * 1024 * 1024
EXPECTED_WORKER_SOURCES = {
    Path("/opt/bottie/diffusers_pytorch_worker.py"): (
        974,
        "18a4a9a63292358197cd7f58ed4f3515e0317c54ceb78b5db08099ea1e9a8811",
    ),
    Path("/opt/bottie/diffusers_worker.py"): (
        2_894,
        "772ed436de27afc01c043202a7815097e9d2249bd1368b2efa53e1d28d54a67c",
    ),
    Path("/opt/bottie/mlx_worker.py"): (
        18_820,
        "44a1e382713c53a7a30ab05e059156cbed0f83097bf383c02ebd744fc6cb878b",
    ),
}


class CleanRuntimeClosureError(RuntimeError):
    """Stable failure raised when the clean closure environment has drifted."""


def validate_installed_identities(
    lock: dict,
    python_components: list[dict],
    native_components: list[dict],
    *,
    expected_python_count: int = EXPECTED_PYTHON_ARTIFACTS,
    expected_debian_count: int = EXPECTED_DEBIAN_ARTIFACTS,
    validate_digest: bool = True,
) -> None:
    """Require every installed Python and Debian identity to equal the frozen lock."""
    try:
        _validate_manifest_header(lock)
        python_artifacts = _validate_python_records(
            lock.get("pythonArtifacts"), expected_python_count
        )
        debian_artifacts = _validate_debian_records(
            lock.get("debianArtifacts"), expected_debian_count
        )
        if validate_digest:
            _validate_lock_digest(lock)
    except CleanRuntimeLockError as error:
        raise CleanRuntimeClosureError(str(error)) from error
    installed_python = {
        (component.get("name"), component.get("version"))
        for component in python_components
    }
    expected_python = {
        (artifact["name"], artifact["version"]) for artifact in python_artifacts
    }
    if (
        len(installed_python) != len(python_components)
        or installed_python != expected_python
    ):
        raise CleanRuntimeClosureError(
            "clean-runtime Python identities differ from the frozen lock"
        )
    installed_debian = {
        (
            component.get("name"),
            component.get("version"),
            component.get("architecture"),
        )
        for component in native_components
    }
    expected_debian = {
        (artifact["name"], artifact["version"], artifact["architecture"])
        for artifact in debian_artifacts
    }
    if (
        len(installed_debian) != len(native_components)
        or installed_debian != expected_debian
    ):
        raise CleanRuntimeClosureError(
            "clean-runtime Debian identities differ from the frozen lock"
        )


def collect_clean_environment_measurement(lock_path: Path) -> dict:
    """Collect package evidence only after target, source, and lock identities are exact."""
    if (
        platform.system() != "Linux"
        or platform.machine() != "aarch64"
        or platform.python_version() != CLEAN_PYTHON_VERSION
    ):
        raise CleanRuntimeClosureError("clean-runtime closure target is not exact")
    lock = _load_clean_lock(lock_path)
    _verify_worker_sources()
    python_components = [
        normalize_clean_python_component(_python_component(distribution))
        for distribution in metadata.distributions()
    ]
    native_components = [
        _native_component(*fields) for fields in _installed_debian_packages()
    ]
    validate_installed_identities(lock, python_components, native_components)
    return {
        "pythonComponents": python_components,
        "nativeComponents": native_components,
    }


def normalize_clean_python_component(component: dict) -> dict:
    """Return a copy using the canonical name and version forms frozen by the lock."""
    try:
        return {
            **component,
            "name": str(canonicalize_name(component["name"])),
            "version": str(Version(component["version"])),
        }
    except (InvalidVersion, KeyError, TypeError) as error:
        raise CleanRuntimeClosureError(
            "clean-runtime Python identity is invalid"
        ) from error


def clean_python_file_owners() -> dict[Path, set[str]]:
    """Map installed Python files to canonical lock-compatible component identities."""
    owners: dict[Path, set[str]] = defaultdict(set)
    for distribution in metadata.distributions():
        identity = _clean_distribution_identity(distribution)
        for relative in distribution.files or []:
            path = Path(distribution.locate_file(relative))
            try:
                resolved = path.resolve(strict=True)
            except (FileNotFoundError, OSError):
                continue
            if resolved.is_file():
                owners[resolved].add(identity)
    return owners


def clean_installed_python_requirements(roots: set[str]) -> tuple[set[str], set[str]]:
    """Close active dependencies with the same canonical identities as the frozen lock."""
    requirements = {}
    installed = {}
    for distribution in metadata.distributions():
        identity = _clean_distribution_identity(distribution)
        canonical_name = str(canonicalize_name(distribution.metadata["Name"]))
        if canonical_name in installed:
            raise RuntimeOwnershipError(
                "installed Python distribution name is ambiguous"
            )
        installed[canonical_name] = identity
        requirements[identity] = list(distribution.requires or [])
    try:
        return resolve_required_python_components(
            roots,
            requirements,
            installed,
            {**default_environment(), "extra": ""},
        )
    except RuntimeDependencyError as error:
        raise RuntimeOwnershipError(str(error)) from error


def read_clean_elf_dependencies(path: Path) -> tuple[set[str], str | None]:
    """Read AArch64 ELF dynamic dependencies without adding a runtime executable."""
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        before = os.fstat(descriptor)
        header = os.pread(descriptor, ELF_HEADER_BYTES, 0)
        if len(header) != ELF_HEADER_BYTES or header[:7] != b"\x7fELF\x02\x01\x01":
            raise CleanRuntimeClosureError("clean-runtime ELF header is unsupported")
        fields = struct.unpack("<16sHHIQQQIHHHHHH", header)
        machine, program_offset, entry_size, entry_count = (
            fields[2],
            fields[5],
            fields[9],
            fields[10],
        )
        if (
            machine != ELF_MACHINE_AARCH64
            or entry_size != ELF_PROGRAM_HEADER_BYTES
            or entry_count > MAX_ELF_PROGRAM_HEADERS
        ):
            raise CleanRuntimeClosureError(
                "clean-runtime ELF program headers are unsupported"
            )
        program_bytes = os.pread(descriptor, entry_size * entry_count, program_offset)
        if len(program_bytes) != entry_size * entry_count:
            raise CleanRuntimeClosureError(
                "clean-runtime ELF program headers are truncated"
            )
        load_segments = []
        dynamic_segments = []
        for index in range(entry_count):
            start = index * entry_size
            program = struct.unpack(
                "<IIQQQQQQ", program_bytes[start : start + entry_size]
            )
            segment_type, offset, address, file_size = (
                program[0],
                program[2],
                program[3],
                program[5],
            )
            if segment_type == ELF_PT_LOAD:
                load_segments.append((address, offset, file_size))
            elif segment_type == ELF_PT_DYNAMIC:
                dynamic_segments.append((offset, file_size))
        if not dynamic_segments:
            _verify_elf_stability(before, os.fstat(descriptor))
            return set(), None
        if len(dynamic_segments) != 1:
            raise CleanRuntimeClosureError(
                "clean-runtime ELF dynamic segment is ambiguous"
            )
        dynamic_offset, dynamic_size = dynamic_segments[0]
        if (
            dynamic_size > MAX_ELF_DYNAMIC_BYTES
            or dynamic_size % ELF_DYNAMIC_ENTRY_BYTES != 0
        ):
            raise CleanRuntimeClosureError(
                "clean-runtime ELF dynamic section is invalid"
            )
        dynamic_bytes = os.pread(descriptor, dynamic_size, dynamic_offset)
        if len(dynamic_bytes) != dynamic_size:
            raise CleanRuntimeClosureError(
                "clean-runtime ELF dynamic section is truncated"
            )
        needed_offsets = []
        soname_offsets = []
        string_addresses = set()
        string_sizes = set()
        terminated = False
        for index in range(0, dynamic_size, ELF_DYNAMIC_ENTRY_BYTES):
            tag, value = struct.unpack(
                "<qQ", dynamic_bytes[index : index + ELF_DYNAMIC_ENTRY_BYTES]
            )
            if tag == ELF_DT_NULL:
                terminated = True
                break
            if tag == ELF_DT_NEEDED:
                needed_offsets.append(value)
            elif tag == ELF_DT_STRTAB:
                string_addresses.add(value)
            elif tag == ELF_DT_STRSZ:
                string_sizes.add(value)
            elif tag == ELF_DT_SONAME:
                soname_offsets.append(value)
        if not terminated:
            raise CleanRuntimeClosureError(
                "clean-runtime ELF dynamic section is unterminated"
            )
        if not needed_offsets and not soname_offsets:
            _verify_elf_stability(before, os.fstat(descriptor))
            return set(), None
        if len(string_addresses) != 1 or len(string_sizes) != 1:
            raise CleanRuntimeClosureError(
                "clean-runtime ELF string table is ambiguous"
            )
        string_address = next(iter(string_addresses))
        string_size = next(iter(string_sizes))
        if string_size <= 0 or string_size > MAX_ELF_STRING_TABLE_BYTES:
            raise CleanRuntimeClosureError("clean-runtime ELF string table is invalid")
        string_offset = _elf_address_to_offset(
            string_address, string_size, load_segments
        )
        string_bytes = os.pread(descriptor, string_size, string_offset)
        after = os.fstat(descriptor)
    except OSError as error:
        raise CleanRuntimeClosureError(
            "clean-runtime ELF file is unavailable"
        ) from error
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if len(string_bytes) != string_size:
        raise CleanRuntimeClosureError("clean-runtime ELF string table is truncated")
    _verify_elf_stability(before, after)
    needed = {_elf_string(string_bytes, offset) for offset in needed_offsets}
    sonames = {_elf_string(string_bytes, offset) for offset in soname_offsets}
    if len(sonames) > 1:
        raise CleanRuntimeClosureError("ELF file declares multiple SONAME values")
    return needed, next(iter(sonames), None)


def _verify_elf_stability(before: os.stat_result, after: os.stat_result) -> None:
    """Reject an ELF file whose identity or bytes changed during inspection."""
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    observed = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if stable != observed:
        raise CleanRuntimeClosureError(
            "clean-runtime ELF file changed while being measured"
        )


def _elf_address_to_offset(
    address: int,
    byte_size: int,
    load_segments: list[tuple[int, int, int]],
) -> int:
    """Map one bounded virtual-address range into exactly one file-backed load segment."""
    offsets = {
        offset + address - segment_address
        for segment_address, offset, file_size in load_segments
        if segment_address <= address
        and address + byte_size <= segment_address + file_size
    }
    if len(offsets) != 1:
        raise CleanRuntimeClosureError(
            "clean-runtime ELF string table is not file-backed"
        )
    return offsets.pop()


def _elf_string(string_table: bytes, offset: int) -> str:
    """Return one bounded filename-only dependency from an ELF string table."""
    if not isinstance(offset, int) or offset < 0 or offset >= len(string_table):
        raise CleanRuntimeClosureError("clean-runtime ELF string offset is invalid")
    end = string_table.find(b"\0", offset)
    if end < 0:
        raise CleanRuntimeClosureError("clean-runtime ELF string is unterminated")
    try:
        value = string_table[offset:end].decode("utf-8")
    except UnicodeError as error:
        raise CleanRuntimeClosureError(
            "clean-runtime ELF dependency is not UTF-8"
        ) from error
    if (
        not value
        or len(value.encode()) > 256
        or "/" in value
        or "\\" in value
        or any(ord(character) < 32 for character in value)
    ):
        raise CleanRuntimeClosureError("clean-runtime ELF dependency name is invalid")
    return value


def _clean_distribution_identity(distribution: metadata.Distribution) -> str:
    """Return one canonical lock-compatible installed distribution identity."""
    name = distribution.metadata.get("Name")
    version = distribution.version
    if not name or not version:
        raise RuntimeOwnershipError("Python distribution identity is incomplete")
    try:
        return f"python:{canonicalize_name(name)}@{Version(version)}"
    except InvalidVersion as error:
        raise RuntimeOwnershipError("Python distribution version is invalid") from error


def _load_clean_lock(path: Path) -> dict:
    """Translate bounded lock parsing failures into the clean-closure boundary."""
    try:
        return _load_manifest(path)
    except CleanRuntimeLockError as error:
        raise CleanRuntimeClosureError(str(error)) from error


def _verify_worker_sources() -> None:
    """Require the exact three first-party bytes frozen by the rebuild plan."""
    for path, expected in EXPECTED_WORKER_SOURCES.items():
        try:
            descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode):
                raise CleanRuntimeClosureError(
                    "clean-runtime worker source is not regular"
                )
            hasher = hashlib.sha256()
            byte_size = 0
            with os.fdopen(descriptor, "rb") as stream:
                while chunk := stream.read(HASH_BUFFER_BYTES):
                    hasher.update(chunk)
                    byte_size += len(chunk)
                after = os.fstat(stream.fileno())
        except OSError as error:
            raise CleanRuntimeClosureError(
                "clean-runtime worker source is unavailable"
            ) from error
        stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        observed = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        if stable != observed or (byte_size, hasher.hexdigest()) != expected:
            raise CleanRuntimeClosureError("clean-runtime worker source has drifted")
