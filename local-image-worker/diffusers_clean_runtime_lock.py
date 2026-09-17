#!/usr/bin/env python3
"""Verify immutable package inputs for the clean Linux Diffusers proof image."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import stat
import subprocess
import zipfile
from email.parser import BytesParser
from pathlib import Path, PurePosixPath
from typing import Callable

from packaging.utils import canonicalize_name, parse_wheel_filename
from packaging.version import InvalidVersion, Version


SCHEMA_VERSION = 1
CLEAN_PROOF_IMAGE_DIGEST = (
    "sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad"
)
CLEAN_BASE_IMAGE_DIGEST = (
    "sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90"
)
TARGET_PYTHON_VERSION = "3.12.3"
EXPECTED_PYTHON_ARTIFACTS = 63
EXPECTED_DEBIAN_ARTIFACTS = 112
MAX_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_FIELD_BYTES = 512
MAX_WHEEL_METADATA_BYTES = 256 * 1024
READ_CHUNK_BYTES = 1024 * 1024
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
MANIFEST_FIELDS = {
    "schemaVersion",
    "sourceImageDigest",
    "baseImageDigest",
    "target",
    "pythonVersion",
    "pythonArtifacts",
    "debianArtifacts",
    "lockSha256",
}
PYTHON_ARTIFACT_FIELDS = {"name", "version", "filename", "byteSize", "sha256"}
DEBIAN_ARTIFACT_FIELDS = {
    "name",
    "version",
    "architecture",
    "filename",
    "byteSize",
    "sha256",
}


class CleanRuntimeLockError(RuntimeError):
    """Stable failure raised when clean-runtime package evidence is not exact."""


def validate_clean_runtime_lock(manifest: dict, artifact_root: Path) -> dict:
    """Verify the exact 63-wheel and 112-Debian input set for the clean proof image."""
    return _validate_clean_runtime_lock(
        manifest,
        artifact_root,
        EXPECTED_PYTHON_ARTIFACTS,
        EXPECTED_DEBIAN_ARTIFACTS,
        _read_debian_metadata,
    )


def _validate_clean_runtime_lock(
    manifest: dict,
    artifact_root: Path,
    expected_python_count: int,
    expected_debian_count: int,
    read_debian_metadata: Callable[[Path], tuple[str, str, str]],
) -> dict:
    """Validate one lock with injected counts and Debian inspection for focused tests."""
    _validate_manifest_header(manifest)
    python_artifacts = _validate_python_records(
        manifest.get("pythonArtifacts"), expected_python_count
    )
    debian_artifacts = _validate_debian_records(
        manifest.get("debianArtifacts"), expected_debian_count
    )
    _validate_lock_digest(manifest)
    root = artifact_root.resolve(strict=True)
    if not artifact_root.is_absolute() or artifact_root.is_symlink():
        raise CleanRuntimeLockError("artifact root is not one exact absolute directory")
    python_paths = _exact_artifact_paths(root / "python", python_artifacts)
    debian_paths = _exact_artifact_paths(root / "debian", debian_artifacts)
    if _directory_names(root) != {"python", "debian"}:
        raise CleanRuntimeLockError("artifact tree is not exact")
    total_bytes = 0
    for record, path in zip(python_artifacts, python_paths, strict=True):
        total_bytes += _verify_artifact_bytes(path, record)
        _verify_wheel_identity(path, record)
    for record, path in zip(debian_artifacts, debian_paths, strict=True):
        total_bytes += _verify_artifact_bytes(path, record)
        if read_debian_metadata(path) != (
            record["name"],
            record["version"],
            record["architecture"],
        ):
            raise CleanRuntimeLockError("Debian identity has drifted")
    return {
        "schemaVersion": SCHEMA_VERSION,
        "sourceImageDigest": CLEAN_PROOF_IMAGE_DIGEST,
        "baseImageDigest": CLEAN_BASE_IMAGE_DIGEST,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "pythonVersion": TARGET_PYTHON_VERSION,
        "lockSha256": manifest["lockSha256"],
        "pythonArtifactCount": len(python_artifacts),
        "debianArtifactCount": len(debian_artifacts),
        "totalArtifactByteSize": total_bytes,
        "verified": True,
    }


def _validate_manifest_header(manifest: dict) -> None:
    """Require the closed schema and exact clean-image identities."""
    if not isinstance(manifest, dict) or set(manifest) != MANIFEST_FIELDS:
        raise CleanRuntimeLockError("clean-runtime lock schema is not exact")
    if manifest.get("schemaVersion") != SCHEMA_VERSION:
        raise CleanRuntimeLockError("clean-runtime lock version is unsupported")
    if manifest.get("sourceImageDigest") != CLEAN_PROOF_IMAGE_DIGEST:
        raise CleanRuntimeLockError("clean proof image identity has drifted")
    if manifest.get("baseImageDigest") != CLEAN_BASE_IMAGE_DIGEST:
        raise CleanRuntimeLockError("clean base image identity has drifted")
    if manifest.get("target") != {"operatingSystem": "linux", "architecture": "arm64"}:
        raise CleanRuntimeLockError("clean-runtime target has drifted")
    if manifest.get("pythonVersion") != TARGET_PYTHON_VERSION:
        raise CleanRuntimeLockError("clean-runtime Python version has drifted")
    if not isinstance(manifest.get("lockSha256"), str) or not SHA256_PATTERN.fullmatch(
        manifest["lockSha256"]
    ):
        raise CleanRuntimeLockError("lock digest is invalid")


def _validate_python_records(records: object, expected_count: int) -> list[dict]:
    """Validate sorted wheel records without permitting source distributions."""
    if not isinstance(records, list) or len(records) != expected_count:
        raise CleanRuntimeLockError("Python artifact count is not exact")
    validated = []
    for record in records:
        if not isinstance(record, dict) or set(record) != PYTHON_ARTIFACT_FIELDS:
            raise CleanRuntimeLockError("Python artifact schema is not exact")
        name = _canonical_python_name(record.get("name"))
        version = _python_version(record.get("version"))
        filename = _artifact_filename(record.get("filename"), ".whl")
        _artifact_measurement(record)
        try:
            wheel_name, wheel_version, _, wheel_tags = parse_wheel_filename(filename)
        except (InvalidVersion, ValueError) as error:
            raise CleanRuntimeLockError("wheel filename is invalid") from error
        if str(wheel_name) != name or wheel_version != Version(version):
            raise CleanRuntimeLockError("wheel identity has drifted")
        if any(
            tag.platform != "any" and not tag.platform.endswith("_aarch64")
            for tag in wheel_tags
        ):
            raise CleanRuntimeLockError("wheel target is not ARM64")
        validated.append(record)
    _require_sorted_unique(
        validated,
        lambda record: (record["name"], record["version"], record["filename"]),
        "Python artifacts",
    )
    return validated


def _validate_debian_records(records: object, expected_count: int) -> list[dict]:
    """Validate sorted exact-version Debian artifact records."""
    if not isinstance(records, list) or len(records) != expected_count:
        raise CleanRuntimeLockError("Debian artifact count is not exact")
    validated = []
    for record in records:
        if not isinstance(record, dict) or set(record) != DEBIAN_ARTIFACT_FIELDS:
            raise CleanRuntimeLockError("Debian artifact schema is not exact")
        _bounded_field(record.get("name"), "Debian package name")
        _bounded_field(record.get("version"), "Debian package version")
        architecture = _bounded_field(
            record.get("architecture"), "Debian package architecture"
        )
        if architecture not in {"all", "arm64"}:
            raise CleanRuntimeLockError("Debian package architecture is invalid")
        _artifact_filename(record.get("filename"), ".deb")
        _artifact_measurement(record)
        validated.append(record)
    _require_sorted_unique(
        validated,
        lambda record: (
            record["name"],
            record["version"],
            record["architecture"],
            record["filename"],
        ),
        "Debian artifacts",
    )
    return validated


def _canonical_python_name(value: object) -> str:
    """Return one already-canonical Python distribution name."""
    value = _bounded_field(value, "Python distribution name")
    canonical = str(canonicalize_name(value))
    if value != canonical:
        raise CleanRuntimeLockError("Python distribution name is not canonical")
    return canonical


def _python_version(value: object) -> str:
    """Return one valid normalized Python distribution version."""
    value = _bounded_field(value, "Python distribution version")
    try:
        normalized = str(Version(value))
    except InvalidVersion as error:
        raise CleanRuntimeLockError("Python distribution version is invalid") from error
    if value != normalized:
        raise CleanRuntimeLockError("Python distribution version is not normalized")
    return normalized


def _bounded_field(value: object, label: str) -> str:
    """Validate one path-free bounded identity field."""
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or len(value.encode()) > MAX_FIELD_BYTES
        or any(ord(character) < 32 for character in value)
        or "/" in value
        or "\\" in value
        or value in {".", ".."}
    ):
        raise CleanRuntimeLockError(f"{label} is invalid")
    return value


def _artifact_filename(value: object, suffix: str) -> str:
    """Validate one portable artifact basename with the required format."""
    try:
        filename = _bounded_field(value, "artifact filename")
    except CleanRuntimeLockError as error:
        raise CleanRuntimeLockError("artifact filename is invalid") from error
    if PurePosixPath(filename).name != filename or not filename.endswith(suffix):
        raise CleanRuntimeLockError("artifact filename is invalid")
    return filename


def _artifact_measurement(record: dict) -> None:
    """Validate exact positive size and lowercase SHA-256 fields."""
    byte_size = record.get("byteSize")
    sha256 = record.get("sha256")
    if not isinstance(byte_size, int) or isinstance(byte_size, bool) or byte_size <= 0:
        raise CleanRuntimeLockError("artifact byte size is invalid")
    if not isinstance(sha256, str) or not SHA256_PATTERN.fullmatch(sha256):
        raise CleanRuntimeLockError("artifact SHA-256 is invalid")


def _require_sorted_unique(
    records: list[dict], key: Callable[[dict], tuple], label: str
) -> None:
    """Require records and filenames to be uniquely sorted by UTF-8 bytes."""
    keys = [key(record) for record in records]
    encoded = [tuple(str(field).encode() for field in fields) for fields in keys]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise CleanRuntimeLockError(f"{label} are not sorted and unique")
    filenames = [record["filename"] for record in records]
    if len(filenames) != len(set(filenames)) or len(filenames) != len(
        {name.casefold() for name in filenames}
    ):
        raise CleanRuntimeLockError("artifact filenames are not unique")


def _validate_lock_digest(manifest: dict) -> None:
    """Recompute the canonical digest over every lock field except the digest itself."""
    payload = {key: value for key, value in manifest.items() if key != "lockSha256"}
    actual = hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    if actual != manifest["lockSha256"]:
        raise CleanRuntimeLockError("lock digest has drifted")


def _directory_names(directory: Path) -> set[str]:
    """Return exact direct-child names while rejecting non-directories and symlinks."""
    try:
        children = list(directory.iterdir())
    except OSError as error:
        raise CleanRuntimeLockError("artifact tree cannot be inspected") from error
    if any(child.is_symlink() or not child.is_dir() for child in children):
        raise CleanRuntimeLockError("artifact tree is not exact")
    return {child.name for child in children}


def _exact_artifact_paths(directory: Path, records: list[dict]) -> list[Path]:
    """Bind manifest basenames to one symlink-free directory with no extra entries."""
    if directory.is_symlink() or not directory.is_dir():
        raise CleanRuntimeLockError("artifact tree is not exact")
    expected = [record["filename"] for record in records]
    try:
        entries = list(directory.iterdir())
    except OSError as error:
        raise CleanRuntimeLockError("artifact tree cannot be inspected") from error
    if any(entry.is_symlink() or not entry.is_file() for entry in entries):
        raise CleanRuntimeLockError("artifact tree is not exact")
    actual = sorted((entry.name for entry in entries), key=str.encode)
    if actual != sorted(expected, key=str.encode):
        raise CleanRuntimeLockError("artifact tree is not exact")
    return [directory / filename for filename in expected]


def _verify_artifact_bytes(path: Path, record: dict) -> int:
    """Hash one regular artifact and require its exact manifest measurement."""
    digest = hashlib.sha256()
    try:
        file_stat = path.stat(follow_symlinks=False)
        if not stat.S_ISREG(file_stat.st_mode):
            raise CleanRuntimeLockError("artifact tree is not exact")
        with path.open("rb") as artifact:
            while chunk := artifact.read(READ_CHUNK_BYTES):
                digest.update(chunk)
    except OSError as error:
        raise CleanRuntimeLockError("artifact bytes cannot be read") from error
    if (
        file_stat.st_size != record["byteSize"]
        or digest.hexdigest() != record["sha256"]
    ):
        raise CleanRuntimeLockError("artifact bytes have drifted")
    return file_stat.st_size


def _verify_wheel_identity(path: Path, record: dict) -> None:
    """Require the exact wheel archive metadata to agree with its lock identity."""
    try:
        with zipfile.ZipFile(path) as archive:
            members = archive.infolist()
            if any(_unsafe_zip_member(member) for member in members):
                raise CleanRuntimeLockError("wheel archive contains unsafe members")
            names = [member.filename for member in members]
            metadata_members = [
                member
                for member in members
                if member.filename.endswith(".dist-info/METADATA")
            ]
            if (
                len(names) != len(set(names))
                or len(metadata_members) != 1
                or metadata_members[0].file_size > MAX_WHEEL_METADATA_BYTES
            ):
                raise CleanRuntimeLockError("wheel metadata is not exact")
            parsed = BytesParser().parsebytes(archive.read(metadata_members[0]))
    except (OSError, zipfile.BadZipFile, KeyError) as error:
        raise CleanRuntimeLockError("wheel archive cannot be inspected") from error
    try:
        metadata_name = str(canonicalize_name(parsed["Name"]))
        metadata_version = Version(parsed["Version"])
    except (InvalidVersion, TypeError) as error:
        raise CleanRuntimeLockError("wheel metadata is malformed") from error
    if metadata_name != record["name"] or metadata_version != Version(
        record["version"]
    ):
        raise CleanRuntimeLockError("wheel identity has drifted")


def _unsafe_zip_member(member: zipfile.ZipInfo) -> bool:
    """Report archive members that are absolute, traversing, or symlinks."""
    name = member.filename
    path = PurePosixPath(name)
    unix_mode = member.external_attr >> 16
    return (
        not name
        or "\\" in name
        or path.is_absolute()
        or any(part in {"", ".", ".."} for part in path.parts)
        or stat.S_ISLNK(unix_mode)
    )


def _read_debian_metadata(path: Path) -> tuple[str, str, str]:
    """Read one Debian archive's declared identity without extracting it."""
    try:
        completed = subprocess.run(
            [
                "dpkg-deb",
                "--field",
                str(path),
                "${Package}\t${Version}\t${Architecture}\n",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise CleanRuntimeLockError(
            "Debian artifact inspection is unavailable"
        ) from error
    fields = completed.stdout.rstrip("\n").split("\t")
    if (
        completed.returncode != 0
        or len(fields) != 3
        or any(not field for field in fields)
    ):
        raise CleanRuntimeLockError("Debian artifact metadata is malformed")
    return fields[0], fields[1], fields[2]


def _load_manifest(path: Path) -> dict:
    """Load one bounded JSON lock without retaining its path in evidence."""
    try:
        if (
            not path.is_absolute()
            or path.is_symlink()
            or path.stat().st_size > MAX_MANIFEST_BYTES
        ):
            raise CleanRuntimeLockError("clean-runtime lock file is invalid")
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CleanRuntimeLockError("clean-runtime lock file cannot be read") from error
    if not isinstance(manifest, dict):
        raise CleanRuntimeLockError("clean-runtime lock schema is not exact")
    return manifest


def main() -> None:
    """Verify one externally assembled artifact set and print normalized path-free evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("artifact_root", type=Path)
    arguments = parser.parse_args()
    try:
        evidence = validate_clean_runtime_lock(
            _load_manifest(arguments.manifest),
            arguments.artifact_root,
        )
    except CleanRuntimeLockError as error:
        parser.error(str(error))
    print(json.dumps(evidence, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
