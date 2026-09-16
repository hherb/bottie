#!/usr/bin/env python3
"""Produce closed proof-only evidence for one pinned Linux Diffusers worker bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit

from diffusers_worker import DIFFUSERS_WORKER_IDENTITY


SCHEMA_VERSION = 1
LICENSE_SCHEMA_VERSION = 1
BUNDLE_HASH_DOMAIN = b"bottie-local-image-worker-bundle-v1"
BASE_IMAGE = "nvcr.io/nvidia/pytorch:25.11-py3"
BASE_IMAGE_DIGEST = "sha256:417cbf33f87b5378849df37983552cd1f8bc8b62fe1ceabe004de816a55dff21"
TARGET_OPERATING_SYSTEM = "linux"
TARGET_ARCHITECTURE = "aarch64"
HASH_BUFFER_BYTES = 1024 * 1024
MAX_TEXT_FIELD_BYTES = 512
REJECTED_LICENSE_EXPRESSIONS = frozenset({"", "NOASSERTION", "NONE", "UNKNOWN"})
PROOF_INPUTS = (
    (
        "local-image-worker/Dockerfile.diffusers-proof",
        "Dockerfile.diffusers-proof",
        "82e0a935f2438d959989c882f4360569a5bb320f3ba9bf1dc54395941cce7d0b",
    ),
    (
        "local-image-worker/requirements-diffusers-proof.txt",
        "requirements-diffusers-proof.txt",
        "3326487f6a10cd6e1348cf7d8ffea7cd0011baa95a518a620025903c1227db7d",
    ),
    (
        "local-image-worker/diffusers_worker.py",
        "diffusers_worker.py",
        "772ed436de27afc01c043202a7815097e9d2249bd1368b2efa53e1d28d54a67c",
    ),
    (
        "local-image-worker/mlx_worker.py",
        "mlx_worker.py",
        "44a1e382713c53a7a30ab05e059156cbed0f83097bf383c02ebd744fc6cb878b",
    ),
)


class CandidateEvidenceError(ValueError):
    """Stable failure for an unsafe, incomplete, or identity-drifted bundle candidate."""


def build_candidate_evidence(
    bundle_root: Path,
    executable_relative_path: str,
    license_manifest_relative_path: str,
) -> dict:
    """Measure a closed bundle and require complete path-to-license ownership metadata."""
    root = canonical_bundle_root(bundle_root)
    executable_path = validate_relative_path(executable_relative_path)
    license_manifest_path = validate_relative_path(license_manifest_relative_path)
    files = collect_regular_files(root)
    inventory, bundle_sha256 = measure_inventory(files)
    inventory_by_path = {entry["path"]: entry for entry in inventory}
    executable = required_inventory_entry(inventory_by_path, executable_path)
    license_manifest = required_inventory_entry(inventory_by_path, license_manifest_path)
    metadata = load_license_metadata(root / license_manifest_path, license_manifest)
    ownership, components = validate_license_coverage(
        metadata,
        inventory_by_path,
        license_manifest_path,
    )
    for entry in inventory:
        entry["ownership"] = ownership[entry["path"]]
    if ownership[executable_path] != "first-party":
        raise CandidateEvidenceError("worker executable must be first-party")
    return {
        "schemaVersion": SCHEMA_VERSION,
        "workerVersion": DIFFUSERS_WORKER_IDENTITY.worker_version,
        "runtimeId": DIFFUSERS_WORKER_IDENTITY.runtime_id,
        "modelId": DIFFUSERS_WORKER_IDENTITY.model_id,
        "modelRevision": DIFFUSERS_WORKER_IDENTITY.model_revision,
        "baseImage": BASE_IMAGE,
        "baseImageDigest": BASE_IMAGE_DIGEST,
        "proofInputs": verify_proof_inputs(),
        "target": {
            "operatingSystem": TARGET_OPERATING_SYSTEM,
            "architecture": TARGET_ARCHITECTURE,
        },
        "executable": {
            "path": executable_path,
            "byteSize": executable["byteSize"],
            "sha256": executable["sha256"],
        },
        "bundle": {
            "fileCount": len(inventory),
            "byteSize": sum(entry["byteSize"] for entry in inventory),
            "sha256": bundle_sha256,
            "files": inventory,
        },
        "licenseInventory": {
            "path": license_manifest_path,
            "byteSize": license_manifest["byteSize"],
            "sha256": license_manifest["sha256"],
            "componentCount": len(components),
            "complete": True,
            "components": components,
        },
        "distributionReviewed": False,
    }


def canonical_bundle_root(bundle_root: Path) -> Path:
    """Return one real directory while rejecting a symlink at the declared root."""
    try:
        metadata = bundle_root.lstat()
    except OSError as error:
        raise CandidateEvidenceError("bundle root is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise CandidateEvidenceError("bundle root must be a real directory")
    return bundle_root.resolve(strict=True)


def validate_relative_path(value: str) -> str:
    """Normalize one portable non-empty relative file or directory path."""
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise CandidateEvidenceError("invalid relative path")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise CandidateEvidenceError("invalid relative path")
    normalized = path.as_posix()
    if normalized != value:
        raise CandidateEvidenceError("relative path is not canonical")
    return normalized


def collect_regular_files(root: Path) -> list[tuple[str, Path]]:
    """Collect every regular file without following or tolerating symlinks and special files."""
    files: list[tuple[str, Path]] = []

    def visit(directory: Path) -> None:
        """Walk one real directory using lstat-backed entry types."""
        try:
            entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except OSError as error:
            raise CandidateEvidenceError("bundle directory cannot be read") from error
        for entry in entries:
            path = Path(entry.path)
            try:
                if entry.is_symlink():
                    raise CandidateEvidenceError("bundle must contain only regular files and directories")
                if entry.is_dir(follow_symlinks=False):
                    visit(path)
                    continue
                if not entry.is_file(follow_symlinks=False):
                    raise CandidateEvidenceError("bundle must contain only regular files and directories")
            except OSError as error:
                raise CandidateEvidenceError("bundle entry cannot be inspected") from error
            relative = validate_relative_path(path.relative_to(root).as_posix())
            files.append((relative, path))

    visit(root)
    if not files:
        raise CandidateEvidenceError("bundle has no regular files")
    files.sort(key=lambda entry: entry[0].encode())
    return files


def measure_inventory(files: list[tuple[str, Path]]) -> tuple[list[dict], str]:
    """Measure exact per-file facts and the native-compatible bundle digest in one stable pass."""
    inventory = []
    bundle_hasher = hashlib.sha256(BUNDLE_HASH_DOMAIN)
    for relative_path, path in files:
        encoded_path = relative_path.encode()
        bundle_hasher.update(len(encoded_path).to_bytes(8, "big"))
        bundle_hasher.update(encoded_path)
        digest, byte_size = hash_regular_file(path, bundle_hasher)
        inventory.append({"path": relative_path, "byteSize": byte_size, "sha256": digest})
    return inventory, bundle_hasher.hexdigest()


def hash_regular_file(path: Path, bundle_hasher) -> tuple[str, int]:
    """Hash one unchanged regular file into its own digest and the canonical bundle digest."""
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        before = os.fstat(descriptor)
        hasher = hashlib.sha256()
        byte_size = 0
        bundle_hasher.update(before.st_size.to_bytes(8, "big"))
        with os.fdopen(descriptor, "rb") as stream:
            while chunk := stream.read(HASH_BUFFER_BYTES):
                hasher.update(chunk)
                bundle_hasher.update(chunk)
                byte_size += len(chunk)
            after = os.fstat(stream.fileno())
    except OSError as error:
        raise CandidateEvidenceError("bundle file cannot be measured") from error
    stable_identity = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    observed_identity = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if not stat.S_ISREG(before.st_mode) or before.st_size != byte_size or stable_identity != observed_identity:
        raise CandidateEvidenceError("bundle file changed while being measured")
    return hasher.hexdigest(), byte_size


def required_inventory_entry(inventory_by_path: dict[str, dict], relative_path: str) -> dict:
    """Return one required inventory entry without exposing a native location."""
    try:
        return inventory_by_path[relative_path]
    except KeyError as error:
        raise CandidateEvidenceError("required bundle file is absent") from error


def verify_proof_inputs() -> list[dict]:
    """Fail on drift from the exact repository inputs used by the pinned NGC proof environment."""
    source_root = Path(__file__).resolve().parent
    inputs = []
    for repository_path, local_name, expected_sha256 in PROOF_INPUTS:
        digest, _ = hash_source_file(source_root / local_name)
        if digest != expected_sha256:
            raise CandidateEvidenceError("pinned Diffusers proof input has drifted")
        inputs.append({"path": repository_path, "sha256": expected_sha256})
    return inputs


def hash_source_file(path: Path) -> tuple[str, int]:
    """Hash one trusted repository input without adding it to the candidate bundle digest."""
    try:
        hasher = hashlib.sha256()
        byte_size = 0
        with path.open("rb") as stream:
            while chunk := stream.read(HASH_BUFFER_BYTES):
                hasher.update(chunk)
                byte_size += len(chunk)
    except OSError as error:
        raise CandidateEvidenceError("pinned Diffusers proof input is unavailable") from error
    return hasher.hexdigest(), byte_size


def load_license_metadata(path: Path, expected: dict) -> dict:
    """Decode the exact already-measured JSON bytes holding reviewed path and licence ownership."""
    try:
        contents = path.read_bytes()
        value = json.loads(contents.decode("utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CandidateEvidenceError("license metadata is invalid") from error
    if len(contents) != expected["byteSize"] or hashlib.sha256(contents).hexdigest() != expected["sha256"]:
        raise CandidateEvidenceError("license metadata changed while being inspected")
    if not isinstance(value, dict) or set(value) != {
        "schemaVersion",
        "firstPartyPathPrefixes",
        "thirdPartyComponents",
    }:
        raise CandidateEvidenceError("license metadata schema is not closed")
    if value["schemaVersion"] != LICENSE_SCHEMA_VERSION:
        raise CandidateEvidenceError("license metadata schema version is unsupported")
    return value


def validate_license_coverage(
    metadata: dict,
    inventory_by_path: dict[str, dict],
    license_manifest_path: str,
) -> tuple[dict[str, str], list[dict]]:
    """Require exactly one first- or third-party owner and complete metadata for every file."""
    first_party_prefixes = validate_prefixes(metadata["firstPartyPathPrefixes"])
    if set(first_party_prefixes) != {"bottie", license_manifest_path}:
        raise CandidateEvidenceError("first-party ownership must use the fixed Bottie boundary")
    raw_components = metadata["thirdPartyComponents"]
    if not isinstance(raw_components, list) or not raw_components:
        raise CandidateEvidenceError("third-party component metadata is incomplete")
    rules: list[tuple[str, str]] = [(prefix, "first-party") for prefix in first_party_prefixes]
    components = []
    seen_components = set()
    for raw_component in raw_components:
        component, component_rules = validate_component(raw_component, inventory_by_path)
        identity = f'{component["name"]}@{component["version"]}'
        if identity in seen_components:
            raise CandidateEvidenceError("duplicate third-party component")
        seen_components.add(identity)
        rules.extend((prefix, identity) for prefix in component_rules)
        components.append(component)
    ownership = {}
    for relative_path in inventory_by_path:
        owners = {owner for prefix, owner in rules if path_matches_prefix(relative_path, prefix)}
        if not owners:
            raise CandidateEvidenceError(f"unclassified bundle file: {relative_path}")
        if len(owners) != 1:
            raise CandidateEvidenceError(f"ambiguous bundle file ownership: {relative_path}")
        ownership[relative_path] = owners.pop()
    for component in components:
        identity = f'{component["name"]}@{component["version"]}'
        if identity not in ownership.values():
            raise CandidateEvidenceError("third-party component covers no bundle files")
        for license_file in component["licenseFiles"]:
            if ownership.get(license_file["path"]) != identity:
                raise CandidateEvidenceError("component license file is not owned by that component")
    components.sort(key=lambda component: (component["name"], component["version"]))
    return ownership, components


def validate_component(raw_component: object, inventory_by_path: dict[str, dict]) -> tuple[dict, list[str]]:
    """Normalize one complete third-party component and bind its included licence files."""
    expected_fields = {
        "name",
        "version",
        "source",
        "licenseExpression",
        "pathPrefixes",
        "licenseFiles",
    }
    if not isinstance(raw_component, dict) or set(raw_component) != expected_fields:
        raise CandidateEvidenceError("third-party component schema is not closed")
    name = validate_text(raw_component["name"], "component name")
    version = validate_text(raw_component["version"], "component version")
    source = validate_https_source(raw_component["source"])
    license_expression = validate_text(raw_component["licenseExpression"], "license expression")
    if license_expression.upper() in REJECTED_LICENSE_EXPRESSIONS:
        raise CandidateEvidenceError("third-party license metadata is incomplete")
    prefixes = validate_prefixes(raw_component["pathPrefixes"])
    raw_license_files = raw_component["licenseFiles"]
    if not isinstance(raw_license_files, list) or not raw_license_files:
        raise CandidateEvidenceError("third-party component has no included license file")
    license_files = []
    for value in raw_license_files:
        relative_path = validate_relative_path(value)
        entry = required_inventory_entry(inventory_by_path, relative_path)
        license_files.append(
            {"path": relative_path, "byteSize": entry["byteSize"], "sha256": entry["sha256"]}
        )
    if len(license_files) != len({entry["path"] for entry in license_files}):
        raise CandidateEvidenceError("third-party license files contain duplicates")
    license_files.sort(key=lambda entry: entry["path"])
    return (
        {
            "name": name,
            "version": version,
            "source": source,
            "licenseExpression": license_expression,
            "licenseFiles": license_files,
        },
        prefixes,
    )


def validate_prefixes(value: object) -> list[str]:
    """Validate one non-empty duplicate-free list of portable path prefixes."""
    if not isinstance(value, list) or not value:
        raise CandidateEvidenceError("path ownership prefixes are absent")
    prefixes = [validate_relative_path(prefix) for prefix in value]
    if len(prefixes) != len(set(prefixes)):
        raise CandidateEvidenceError("path ownership prefixes contain duplicates")
    return prefixes


def validate_text(value: object, label: str) -> str:
    """Require one bounded printable metadata field."""
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or len(value.encode()) > MAX_TEXT_FIELD_BYTES
    ):
        raise CandidateEvidenceError(f"{label} is invalid")
    if any(character.isspace() and character != " " for character in value) or any(
        ord(character) < 32 for character in value
    ):
        raise CandidateEvidenceError(f"{label} is invalid")
    return value


def validate_https_source(value: object) -> str:
    """Require one public credential-free HTTPS component source without query material."""
    source = validate_text(value, "component source")
    try:
        parsed = urlsplit(source)
        port = parsed.port
    except ValueError as error:
        raise CandidateEvidenceError("component source is invalid") from error
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or port not in {None, 443}
        or parsed.query
        or parsed.fragment
    ):
        raise CandidateEvidenceError("component source is invalid")
    return source


def path_matches_prefix(relative_path: str, prefix: str) -> bool:
    """Match a complete path segment rather than an unsafe textual prefix."""
    return relative_path == prefix or relative_path.startswith(f"{prefix}/")


def parse_arguments() -> argparse.Namespace:
    """Parse the explicit proof-only candidate inputs."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle_root", type=Path)
    parser.add_argument("executable_relative_path")
    parser.add_argument("license_manifest_relative_path")
    parser.add_argument("output", type=Path)
    return parser.parse_args()


def main() -> None:
    """Write one path-free candidate record outside the measured bundle."""
    arguments = parse_arguments()
    root = canonical_bundle_root(arguments.bundle_root)
    output_parent = arguments.output.parent.resolve(strict=True)
    output = output_parent / arguments.output.name
    if output.is_relative_to(root):
        raise CandidateEvidenceError("candidate output must stay outside the measured bundle")
    evidence = build_candidate_evidence(
        root,
        arguments.executable_relative_path,
        arguments.license_manifest_relative_path,
    )
    output.write_text(f"{json.dumps(evidence, indent=2, sort_keys=True)}\n", encoding="utf-8")


if __name__ == "__main__":
    main()
