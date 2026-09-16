#!/usr/bin/env python3
"""Inventory the exact DGX Diffusers proof environment before bundle assembly."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata as metadata
import json
import os
import platform
import re
import subprocess
from pathlib import Path, PurePosixPath

from diffusers_bundle_candidate import BASE_IMAGE, BASE_IMAGE_DIGEST, verify_proof_inputs
from diffusers_worker import DIFFUSERS_WORKER_IDENTITY


SCHEMA_VERSION = 1
TARGET_PYTHON_VERSION = "3.12.3"
TARGET_ARCHITECTURE = "aarch64"
DERIVED_IMAGE_DIGEST = "sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c"
NVIDIA_TERMS_FILE = Path("/opt/nvidia/entrypoint.d/30-container-license.txt")
MAX_FIELD_BYTES = 512
MAX_LICENSE_EXPRESSION_BYTES = 1_024
LICENSE_NAME_TOKENS = {"license", "licence", "copying", "notice", "copyright"}
LICENSE_DIRECTORY_NAMES = {"license", "licenses", "licence", "licences", "notices"}
NON_DOCUMENT_SUFFIXES = {
    ".c",
    ".cc",
    ".cpp",
    ".dll",
    ".dylib",
    ".json",
    ".py",
    ".pyc",
    ".pyo",
    ".so",
    ".toml",
    ".yaml",
    ".yml",
}
REJECTED_LICENSE_EXPRESSIONS = {"N/A", "NONE", "TBD", "UNLICENSED", "UNKNOWN"}
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
PINNED_PYTHON_DISTRIBUTIONS = {
    "accelerate": "1.15.0",
    "annotated-doc": "0.0.5",
    "click": "8.5.0",
    "diffusers": "0.40.0",
    "hf-xet": "1.6.0",
    "huggingface-hub": "1.31.0",
    "safetensors": "0.8.0",
    "sentencepiece": "0.2.2",
    "tokenizers": "0.23.2",
    "torch": "2.10.0a0+b558c986e8.nv25.11",
    "transformers": "5.17.0",
    "typer": "0.27.2",
}


class EnvironmentEvidenceError(RuntimeError):
    """Stable failure raised when the proof environment cannot be measured exactly."""


def normalize_component(
    ecosystem: str,
    name: str,
    version: str,
    architecture: str | None,
    license_expression: str,
    license_files: list[tuple[str, int, str]],
) -> dict:
    """Return one bounded path-free component and its measured licence evidence."""
    if ecosystem not in {"python", "deb"}:
        raise ValueError("component ecosystem is unsupported")
    component = {
        "ecosystem": ecosystem,
        "name": _component_field(name, "component name").lower(),
        "version": _component_field(version, "component version"),
        "licenseExpression": _license_expression(license_expression),
        "licenseFiles": sorted(
            (_license_file(relative_name, byte_size, sha256) for relative_name, byte_size, sha256 in license_files),
            key=lambda item: item["relativeName"].encode(),
        ),
    }
    if architecture is not None:
        component["architecture"] = _component_field(architecture, "component architecture").lower()
    if len({item["relativeName"] for item in component["licenseFiles"]}) != len(component["licenseFiles"]):
        raise ValueError("component licence files contain duplicates")
    return component


def environment_review(
    python_components: list[dict],
    native_components: list[dict],
    container_terms_sha256: str | None,
    container_terms_byte_size: int | None,
) -> dict:
    """Build the canonical review record and refuse assembly on incomplete licence bytes."""
    python_components = _sorted_unique_components(python_components, "python")
    native_components = _sorted_unique_components(native_components, "deb")
    blockers = []
    for component in [*python_components, *native_components]:
        identity = f'{component["ecosystem"]}:{component["name"]}@{component["version"]}'
        if component["licenseExpression"] == "undeclared":
            blockers.append(f"{identity}:undeclared-license")
        if not component["licenseFiles"]:
            blockers.append(f"{identity}:missing-license-bytes")
    container_terms = None
    if container_terms_sha256 is None or container_terms_byte_size is None:
        blockers.append("container-terms:missing-license-bytes")
    else:
        container_terms = _measured_file(container_terms_byte_size, container_terms_sha256)
    blockers.sort(key=str.encode)
    return {
        "schemaVersion": SCHEMA_VERSION,
        "workerVersion": DIFFUSERS_WORKER_IDENTITY.worker_version,
        "runtimeId": DIFFUSERS_WORKER_IDENTITY.runtime_id,
        "modelId": DIFFUSERS_WORKER_IDENTITY.model_id,
        "modelRevision": DIFFUSERS_WORKER_IDENTITY.model_revision,
        "baseImage": BASE_IMAGE,
        "baseImageDigest": BASE_IMAGE_DIGEST,
        "derivedImageDigest": DERIVED_IMAGE_DIGEST,
        "target": {"operatingSystem": "linux", "architecture": TARGET_ARCHITECTURE},
        "pythonVersion": TARGET_PYTHON_VERSION,
        "pythonComponents": python_components,
        "nativeComponents": native_components,
        "containerTerms": container_terms,
        "blockers": blockers,
        "assemblyEligible": not blockers,
        "distributionReviewed": False,
    }


def portable_license_name(declared_name: str, index: int) -> str:
    """Return a stable package-relative label without retaining traversal syntax."""
    if not isinstance(index, int) or isinstance(index, bool) or index < 0:
        raise ValueError("licence index is invalid")
    path = PurePosixPath(declared_name)
    if (
        declared_name
        and not path.is_absolute()
        and all(part not in {"", ".", ".."} for part in path.parts)
        and path.as_posix() == declared_name
    ):
        return declared_name
    basename = path.name or "license"
    basename = re.sub(r"[^A-Za-z0-9._+-]", "_", basename)[:128] or "license"
    return f"external/{index:04d}-{basename}"


def collect_environment_review() -> dict:
    """Measure installed Python, Debian, and NVIDIA licence evidence on the named target."""
    _verify_environment()
    python_components = [_python_component(distribution) for distribution in metadata.distributions()]
    native_components = [_native_component(*fields) for fields in _installed_debian_packages()]
    terms = _hash_optional_file(NVIDIA_TERMS_FILE)
    return environment_review(
        python_components,
        native_components,
        terms[0] if terms else None,
        terms[1] if terms else None,
    )


def _verify_environment() -> None:
    """Fail unless collection runs inside the exact retained Linux ARM64 proof image."""
    if platform.system() != "Linux" or platform.machine() != TARGET_ARCHITECTURE:
        raise EnvironmentEvidenceError("proof environment target is not exact")
    if platform.python_version() != TARGET_PYTHON_VERSION:
        raise EnvironmentEvidenceError("proof environment Python version is not exact")
    if os.environ.get("BOTTIE_BASE_IMAGE_DIGEST") != BASE_IMAGE_DIGEST:
        raise EnvironmentEvidenceError("proof environment base-image digest is not bound")
    if os.environ.get("BOTTIE_DERIVED_IMAGE_DIGEST") != DERIVED_IMAGE_DIGEST:
        raise EnvironmentEvidenceError("proof environment derived-image digest is not bound")
    for name, expected_version in PINNED_PYTHON_DISTRIBUTIONS.items():
        try:
            installed_version = metadata.version(name)
        except metadata.PackageNotFoundError as error:
            raise EnvironmentEvidenceError("pinned Python distribution is absent") from error
        if installed_version != expected_version:
            raise EnvironmentEvidenceError("pinned Python distribution has drifted")
    verify_proof_inputs()


def _python_component(distribution: metadata.Distribution) -> dict:
    """Measure one installed Python distribution without retaining an absolute location."""
    name = distribution.metadata.get("Name")
    version = distribution.version
    if not name or not version:
        raise EnvironmentEvidenceError("installed Python distribution identity is incomplete")
    license_files = []
    for index, relative in enumerate(sorted(distribution.files or [], key=lambda item: str(item).encode())):
        relative_name = PurePosixPath(str(relative)).as_posix()
        if not _looks_like_license_file(relative_name):
            continue
        path = Path(distribution.locate_file(relative))
        measurement = _hash_optional_file(path)
        if measurement:
            license_files.append(
                (portable_license_name(relative_name, index), measurement[1], measurement[0])
            )
    return normalize_component(
        "python",
        name,
        version,
        None,
        _python_license_expression(distribution),
        license_files,
    )


def _python_license_expression(distribution: metadata.Distribution) -> str:
    """Prefer the standardized expression and keep legacy declarations bounded."""
    expression = distribution.metadata.get("License-Expression")
    if expression:
        return expression
    legacy = distribution.metadata.get("License")
    if not legacy or "\n" in legacy or len(legacy.encode()) > MAX_LICENSE_EXPRESSION_BYTES:
        return "undeclared"
    return legacy


def _installed_debian_packages() -> list[tuple[str, str, str]]:
    """Return the complete installed Debian package set in stable identity order."""
    try:
        output = subprocess.check_output(
            ["dpkg-query", "-W", "-f=${binary:Package}\t${Version}\t${Architecture}\n"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise EnvironmentEvidenceError("native package inventory is unavailable") from error
    packages = []
    for line in output.splitlines():
        fields = line.split("\t")
        if len(fields) != 3:
            raise EnvironmentEvidenceError("native package inventory is malformed")
        packages.append((fields[0], fields[1], fields[2]))
    if not packages:
        raise EnvironmentEvidenceError("native package inventory is empty")
    return sorted(packages, key=lambda item: tuple(field.encode() for field in item))


def _native_component(name: str, version: str, architecture: str) -> dict:
    """Measure the installed Debian copyright bytes for one native component."""
    base_name = name.split(":", 1)[0]
    copyright_file = Path("/usr/share/doc") / base_name / "copyright"
    measurement = _hash_optional_file(copyright_file)
    license_files = [] if measurement is None else [("copyright", measurement[1], measurement[0])]
    return normalize_component(
        "deb", base_name, version, architecture, "Debian-copyright", license_files
    )


def _sorted_unique_components(components: list[dict], expected_ecosystem: str) -> list[dict]:
    """Validate and sort one complete ecosystem without ambiguous duplicate identities."""
    identities = []
    for component in components:
        if component.get("ecosystem") != expected_ecosystem:
            raise ValueError("component ecosystem does not match its inventory")
        identities.append((component["name"], component["version"], component.get("architecture", "")))
    if len(identities) != len(set(identities)):
        raise ValueError("component inventory contains duplicate identities")
    return [
        component
        for _, component in sorted(
            zip(identities, components, strict=True),
            key=lambda item: tuple(value.encode() for value in item[0]),
        )
    ]


def _component_field(value: object, label: str) -> str:
    """Require a bounded printable identity field without filesystem syntax."""
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or len(value.encode()) > MAX_FIELD_BYTES
        or any(character in value for character in ("/", "\\", "\x00"))
        or any(ord(character) < 32 for character in value)
    ):
        raise ValueError(f"{label} is invalid")
    return value


def _license_expression(value: object) -> str:
    """Normalize one bounded printable declaration without interpreting legal meaning."""
    if not isinstance(value, str) or not value:
        return "undeclared"
    if (
        value != value.strip()
        or len(value.encode()) > MAX_LICENSE_EXPRESSION_BYTES
        or any(ord(character) < 32 for character in value)
        or value.upper() in REJECTED_LICENSE_EXPRESSIONS
    ):
        return "undeclared"
    return value


def _license_file(relative_name: str, byte_size: int, sha256: str) -> dict:
    """Validate one measured package-relative licence file without a native path."""
    path = PurePosixPath(relative_name)
    if (
        not relative_name
        or path.is_absolute()
        or any(part in {"", ".", ".."} for part in path.parts)
        or path.as_posix() != relative_name
    ):
        raise ValueError("licence relative name is invalid")
    return {"relativeName": relative_name, **_measured_file(byte_size, sha256)}


def _measured_file(byte_size: int, sha256: str) -> dict:
    """Validate a positive exact byte measurement and lowercase SHA-256 digest."""
    if not isinstance(byte_size, int) or isinstance(byte_size, bool) or byte_size <= 0:
        raise ValueError("licence byte size is invalid")
    if not isinstance(sha256, str) or SHA256_PATTERN.fullmatch(sha256) is None:
        raise ValueError("licence SHA-256 is invalid")
    return {"byteSize": byte_size, "sha256": sha256}


def _looks_like_license_file(relative_name: str) -> bool:
    """Recognize package-declared licence and notice files by final path segment."""
    path = PurePosixPath(relative_name)
    file_name = path.name.lower()
    if path.suffix.lower() in NON_DOCUMENT_SUFFIXES or file_name.startswith("test_"):
        return False
    if any(part.lower() in LICENSE_DIRECTORY_NAMES for part in path.parts[:-1]):
        return True
    tokens = {token for token in re.split(r"[._-]+", file_name) if token}
    return not LICENSE_NAME_TOKENS.isdisjoint(tokens)


def _hash_optional_file(path: Path) -> tuple[str, int] | None:
    """Hash one real non-empty regular file, returning absence for missing package evidence."""
    try:
        if path.is_symlink():
            path = path.resolve(strict=True)
        if not path.is_file():
            return None
        contents = path.read_bytes()
    except FileNotFoundError:
        return None
    except OSError as error:
        raise EnvironmentEvidenceError("licence evidence cannot be measured") from error
    if not contents:
        return None
    return hashlib.sha256(contents).hexdigest(), len(contents)


def _write_review(review: dict, output: Path) -> None:
    """Atomically write one deterministic path-free environment record."""
    parent = output.parent.resolve(strict=True)
    destination = parent / output.name
    temporary = parent / f".{output.name}.tmp"
    temporary.write_text(f"{json.dumps(review, indent=2, sort_keys=True)}\n", encoding="utf-8")
    os.replace(temporary, destination)


def parse_arguments() -> argparse.Namespace:
    """Parse the explicit review-record destination."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    return parser.parse_args()


def main() -> None:
    """Collect and persist one exact environment review record."""
    arguments = parse_arguments()
    review = collect_environment_review()
    _write_review(review, arguments.output)
    if not review["assemblyEligible"]:
        raise SystemExit(3)


if __name__ == "__main__":
    main()
