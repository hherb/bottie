#!/usr/bin/env python3
"""Collect clean-image package inventory and create its exact offline input lock."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Callable

from packaging.utils import parse_wheel_filename
from packaging.version import InvalidVersion

from diffusers_clean_runtime_lock import (
    CLEAN_BASE_IMAGE_DIGEST,
    CLEAN_PROOF_IMAGE_DIGEST,
    DEBIAN_ARTIFACT_FIELDS,
    EXPECTED_DEBIAN_ARTIFACTS,
    EXPECTED_PYTHON_ARTIFACTS,
    PYTHON_ARTIFACT_FIELDS,
    SCHEMA_VERSION,
    TARGET_PYTHON_VERSION,
    CleanRuntimeLockError,
    _read_debian_metadata,
    _validate_clean_runtime_lock,
    _verify_wheel_identity,
)
from diffusers_clean_runtime_inventory import (
    CleanRuntimeInventoryError,
    collect_unbound_inventory,
    debian_identities,
    debian_record_sort_key,
    python_identities,
    python_record_sort_key,
    validate_inventory,
)


CONTAINER_SOURCE_ROOT = "/opt/bottie-clean-runtime-lock"
READ_CHUNK_BYTES = 1024 * 1024


def collect_verified_inventory(image_reference: str) -> dict:
    """Collect complete installed identities by executing the inspected clean image ID offline."""
    image_id = _inspect_clean_image(image_reference)
    inventory = _collect_from_verified_image(image_id)
    if "sourceImageDigest" in inventory:
        raise CleanRuntimeInventoryError(
            "unbound clean-runtime inventory contains image identity"
        )
    bound = {**inventory, "sourceImageDigest": image_id}
    validate_inventory(bound, EXPECTED_PYTHON_ARTIFACTS, EXPECTED_DEBIAN_ARTIFACTS)
    return bound


def build_clean_runtime_lock(inventory: dict, artifact_root: Path) -> dict:
    """Create and reverify the production-count lock from one exact installed inventory."""
    return _build_clean_runtime_lock(
        inventory,
        artifact_root,
        EXPECTED_PYTHON_ARTIFACTS,
        EXPECTED_DEBIAN_ARTIFACTS,
        _read_debian_metadata,
    )


def _build_clean_runtime_lock(
    inventory: dict,
    artifact_root: Path,
    expected_python_count: int,
    expected_debian_count: int,
    read_debian_metadata: Callable[[Path], tuple[str, str, str]],
) -> dict:
    """Build one deterministic lock with injected counts and Debian reader for focused tests."""
    validate_inventory(inventory, expected_python_count, expected_debian_count)
    root = artifact_root.resolve(strict=True)
    if not artifact_root.is_absolute() or artifact_root.is_symlink():
        raise CleanRuntimeInventoryError(
            "artifact root is not one exact absolute directory"
        )
    python_records = [
        _wheel_record(path)
        for path in _artifact_files(root / "python", ".whl", expected_python_count)
    ]
    debian_records = [
        _debian_record(path, read_debian_metadata)
        for path in _artifact_files(root / "debian", ".deb", expected_debian_count)
    ]
    python_records.sort(key=python_record_sort_key)
    debian_records.sort(key=debian_record_sort_key)
    if python_identities(python_records) != python_identities(
        inventory["pythonDistributions"]
    ):
        raise CleanRuntimeInventoryError(
            "Python inventory does not match locked artifacts"
        )
    if debian_identities(debian_records) != debian_identities(
        inventory["debianPackages"]
    ):
        raise CleanRuntimeInventoryError(
            "Debian inventory does not match locked artifacts"
        )
    manifest = {
        "schemaVersion": SCHEMA_VERSION,
        "sourceImageDigest": CLEAN_PROOF_IMAGE_DIGEST,
        "baseImageDigest": CLEAN_BASE_IMAGE_DIGEST,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "pythonVersion": TARGET_PYTHON_VERSION,
        "pythonArtifacts": python_records,
        "debianArtifacts": debian_records,
    }
    manifest["lockSha256"] = hashlib.sha256(
        json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    try:
        _validate_clean_runtime_lock(
            manifest,
            artifact_root,
            expected_python_count,
            expected_debian_count,
            read_debian_metadata,
        )
    except CleanRuntimeLockError as error:
        raise CleanRuntimeInventoryError(str(error)) from error
    return manifest


def _inspect_clean_image(image_reference: str) -> str:
    """Resolve one Docker reference and require the exact clean proof image and target."""
    if (
        not isinstance(image_reference, str)
        or not image_reference
        or image_reference != image_reference.strip()
        or image_reference.startswith("-")
        or any(ord(character) < 32 for character in image_reference)
    ):
        raise CleanRuntimeInventoryError("clean-image reference is invalid")
    try:
        completed = subprocess.run(
            ["docker", "image", "inspect", image_reference],
            capture_output=True,
            text=True,
            check=False,
        )
        inspected = json.loads(completed.stdout)
    except (OSError, json.JSONDecodeError, TypeError) as error:
        raise CleanRuntimeInventoryError("clean-image inspection failed") from error
    if (
        completed.returncode != 0
        or not isinstance(inspected, list)
        or len(inspected) != 1
        or not isinstance(inspected[0], dict)
    ):
        raise CleanRuntimeInventoryError("clean-image inspection is malformed")
    image = inspected[0]
    if image.get("Id") != CLEAN_PROOF_IMAGE_DIGEST:
        raise CleanRuntimeInventoryError("clean-image identity has drifted")
    if image.get("Os") != "linux" or image.get("Architecture") != "arm64":
        raise CleanRuntimeInventoryError("clean-image target has drifted")
    return CLEAN_PROOF_IMAGE_DIGEST


def _collect_from_verified_image(image_id: str) -> dict:
    """Run the unbound inventory collector in the already verified image ID."""
    if image_id != CLEAN_PROOF_IMAGE_DIGEST:
        raise CleanRuntimeInventoryError("clean-image identity is not verified")
    source_root = Path(__file__).resolve().parent
    command = [
        "docker",
        "run",
        "--rm",
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--user",
        "65534:65534",
        "--entrypoint",
        "python",
        "--mount",
        f"type=bind,src={source_root},dst={CONTAINER_SOURCE_ROOT},readonly",
        image_id,
        f"{CONTAINER_SOURCE_ROOT}/diffusers_clean_runtime_lock_host.py",
        "--collect-unbound",
    ]
    try:
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        inventory = json.loads(completed.stdout)
    except (OSError, json.JSONDecodeError, TypeError) as error:
        raise CleanRuntimeInventoryError(
            "verified clean-image collection failed"
        ) from error
    if completed.returncode != 0 or not isinstance(inventory, dict):
        raise CleanRuntimeInventoryError("verified clean-image collection is malformed")
    return inventory


def _artifact_files(directory: Path, suffix: str, expected_count: int) -> list[Path]:
    """Return one exact symlink-free artifact directory in stable filename order."""
    if directory.is_symlink() or not directory.is_dir():
        raise CleanRuntimeInventoryError("artifact tree is not exact")
    entries = list(directory.iterdir())
    if len(entries) != expected_count or any(
        entry.is_symlink() or not entry.is_file() or not entry.name.endswith(suffix)
        for entry in entries
    ):
        raise CleanRuntimeInventoryError("artifact tree is not exact")
    return sorted(entries, key=lambda path: path.name.encode())


def _wheel_record(path: Path) -> dict:
    """Measure one wheel and bind both filename and embedded metadata identity."""
    try:
        name, version, _, _ = parse_wheel_filename(path.name)
    except (InvalidVersion, ValueError) as error:
        raise CleanRuntimeInventoryError("wheel filename is invalid") from error
    byte_size, sha256 = _measure_file(path)
    record = {
        "name": str(name),
        "version": str(version),
        "filename": path.name,
        "byteSize": byte_size,
        "sha256": sha256,
    }
    try:
        _verify_wheel_identity(path, record)
    except CleanRuntimeLockError as error:
        raise CleanRuntimeInventoryError(str(error)) from error
    if set(record) != PYTHON_ARTIFACT_FIELDS:
        raise AssertionError("generated Python artifact schema drifted")
    return record


def _debian_record(
    path: Path,
    read_debian_metadata: Callable[[Path], tuple[str, str, str]],
) -> dict:
    """Measure one Debian archive and bind its independently inspected package identity."""
    name, version, architecture = read_debian_metadata(path)
    byte_size, sha256 = _measure_file(path)
    record = {
        "name": name,
        "version": version,
        "architecture": architecture,
        "filename": path.name,
        "byteSize": byte_size,
        "sha256": sha256,
    }
    if set(record) != DEBIAN_ARTIFACT_FIELDS:
        raise AssertionError("generated Debian artifact schema drifted")
    return record


def _measure_file(path: Path) -> tuple[int, str]:
    """Return the exact positive byte size and SHA-256 of one regular artifact."""
    digest = hashlib.sha256()
    try:
        byte_size = path.stat(follow_symlinks=False).st_size
        with path.open("rb") as source:
            while chunk := source.read(READ_CHUNK_BYTES):
                digest.update(chunk)
    except OSError as error:
        raise CleanRuntimeInventoryError("artifact bytes cannot be read") from error
    if byte_size <= 0:
        raise CleanRuntimeInventoryError("artifact byte size is invalid")
    return byte_size, digest.hexdigest()


def _write_manifest(manifest: dict, output: Path, artifact_root: Path) -> None:
    """Atomically persist one deterministic lock outside the artifact tree."""
    if not output.is_absolute() or output.is_symlink():
        raise CleanRuntimeInventoryError("clean-runtime lock output is invalid")
    parent = output.parent.resolve(strict=True)
    artifact_root = artifact_root.resolve(strict=True)
    destination = parent / output.name
    if destination == artifact_root or artifact_root in destination.parents:
        raise CleanRuntimeInventoryError(
            "clean-runtime lock output must be outside the artifact tree"
        )
    descriptor = None
    temporary_name = None
    try:
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{output.name}.", dir=parent
        )
        with os.fdopen(descriptor, "w", encoding="utf-8") as temporary:
            descriptor = None
            temporary.write(f"{json.dumps(manifest, indent=2, sort_keys=True)}\n")
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, destination)
        temporary_name = None
    except OSError as error:
        raise CleanRuntimeInventoryError(
            "clean-runtime lock cannot be written"
        ) from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def parse_arguments() -> argparse.Namespace:
    """Parse host lock creation or private in-container collection arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image_reference", nargs="?")
    parser.add_argument("artifact_root", nargs="?", type=Path)
    parser.add_argument("output", nargs="?", type=Path)
    parser.add_argument(
        "--collect-unbound", action="store_true", help=argparse.SUPPRESS
    )
    return parser.parse_args()


def main() -> None:
    """Collect inventory or create one image-bound package lock from offline artifacts."""
    arguments = parse_arguments()
    if arguments.collect_unbound:
        if any(
            value is not None
            for value in (
                arguments.image_reference,
                arguments.artifact_root,
                arguments.output,
            )
        ):
            raise CleanRuntimeInventoryError(
                "unbound collection accepts no host arguments"
            )
        print(
            json.dumps(
                collect_unbound_inventory(), sort_keys=True, separators=(",", ":")
            )
        )
        return
    if (
        arguments.image_reference is None
        or arguments.artifact_root is None
        or arguments.output is None
    ):
        raise CleanRuntimeInventoryError(
            "image reference, artifact root, and output are required"
        )
    inventory = collect_verified_inventory(arguments.image_reference)
    manifest = build_clean_runtime_lock(inventory, arguments.artifact_root)
    _write_manifest(manifest, arguments.output, arguments.artifact_root)


if __name__ == "__main__":
    main()
