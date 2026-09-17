"""Collect and validate installed identities for the clean Linux Diffusers runtime."""

from __future__ import annotations

import importlib.metadata as metadata
import platform
import subprocess

from packaging.utils import canonicalize_name
from packaging.version import InvalidVersion, Version

from diffusers_clean_runtime_lock import (
    CLEAN_PROOF_IMAGE_DIGEST,
    MAX_FIELD_BYTES,
    SCHEMA_VERSION,
    TARGET_PYTHON_VERSION,
)


INVENTORY_FIELDS = {
    "schemaVersion",
    "sourceImageDigest",
    "target",
    "pythonVersion",
    "pythonDistributions",
    "debianPackages",
}
PYTHON_INVENTORY_FIELDS = {"name", "version"}
DEBIAN_INVENTORY_FIELDS = {"name", "version", "architecture"}


class CleanRuntimeInventoryError(RuntimeError):
    """Stable failure raised when clean-image inventory cannot bind exact artifacts."""


def collect_unbound_inventory() -> dict:
    """Measure installed Python and Debian identities inside an externally selected image."""
    if (
        platform.system() != "Linux"
        or platform.machine() != "aarch64"
        or platform.python_version() != TARGET_PYTHON_VERSION
    ):
        raise CleanRuntimeInventoryError("clean-runtime inventory target is not exact")
    python_distributions = []
    for distribution in metadata.distributions():
        name = distribution.metadata.get("Name")
        version = distribution.version
        if not name or not version:
            raise CleanRuntimeInventoryError("installed Python identity is incomplete")
        try:
            python_distributions.append(
                {"name": str(canonicalize_name(name)), "version": str(Version(version))}
            )
        except InvalidVersion as error:
            raise CleanRuntimeInventoryError(
                "installed Python version is invalid"
            ) from error
    try:
        output = subprocess.check_output(
            [
                "dpkg-query",
                "-W",
                "-f=${db:Status-Abbrev}\t${Package}\t${Version}\t${Architecture}\n",
            ],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise CleanRuntimeInventoryError(
            "installed Debian inventory is unavailable"
        ) from error
    debian_packages = []
    for line in output.splitlines():
        fields = line.split("\t")
        if len(fields) != 4 or any(not field for field in fields):
            raise CleanRuntimeInventoryError("installed Debian inventory is malformed")
        if fields[0] != "ii ":
            continue
        debian_packages.append(
            {"name": fields[1], "version": fields[2], "architecture": fields[3]}
        )
    python_distributions.sort(key=python_record_sort_key)
    debian_packages.sort(key=debian_record_sort_key)
    return {
        "schemaVersion": SCHEMA_VERSION,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "pythonVersion": TARGET_PYTHON_VERSION,
        "pythonDistributions": python_distributions,
        "debianPackages": debian_packages,
    }


def validate_inventory(inventory: dict, python_count: int, debian_count: int) -> None:
    """Require a closed image-bound inventory with exact sorted component counts."""
    if not isinstance(inventory, dict) or set(inventory) != INVENTORY_FIELDS:
        raise CleanRuntimeInventoryError("clean-runtime inventory schema is not exact")
    if (
        inventory.get("schemaVersion") != SCHEMA_VERSION
        or inventory.get("sourceImageDigest") != CLEAN_PROOF_IMAGE_DIGEST
        or inventory.get("target")
        != {"operatingSystem": "linux", "architecture": "arm64"}
        or inventory.get("pythonVersion") != TARGET_PYTHON_VERSION
    ):
        raise CleanRuntimeInventoryError("clean-runtime inventory identity has drifted")
    python_records = inventory.get("pythonDistributions")
    debian_records = inventory.get("debianPackages")
    if not isinstance(python_records, list) or len(python_records) != python_count:
        raise CleanRuntimeInventoryError(
            "clean-runtime Python inventory count is not exact"
        )
    if not isinstance(debian_records, list) or len(debian_records) != debian_count:
        raise CleanRuntimeInventoryError(
            "clean-runtime Debian inventory count is not exact"
        )
    if any(
        not isinstance(record, dict) or set(record) != PYTHON_INVENTORY_FIELDS
        for record in python_records
    ):
        raise CleanRuntimeInventoryError(
            "clean-runtime Python inventory schema is not exact"
        )
    if any(
        not isinstance(record, dict) or set(record) != DEBIAN_INVENTORY_FIELDS
        for record in debian_records
    ):
        raise CleanRuntimeInventoryError(
            "clean-runtime Debian inventory schema is not exact"
        )
    for record in python_records:
        try:
            name = _identity_field(record["name"], "Python distribution name")
            version = _identity_field(record["version"], "Python distribution version")
            if name != str(canonicalize_name(name)) or version != str(Version(version)):
                raise CleanRuntimeInventoryError(
                    "clean-runtime Python inventory is not normalized"
                )
        except InvalidVersion as error:
            raise CleanRuntimeInventoryError(
                "clean-runtime Python inventory is not normalized"
            ) from error
    for record in debian_records:
        _identity_field(record["name"], "Debian package name")
        _identity_field(record["version"], "Debian package version")
        architecture = _identity_field(
            record["architecture"], "Debian package architecture"
        )
        if architecture not in {"all", "arm64"}:
            raise CleanRuntimeInventoryError(
                "clean-runtime Debian architecture is invalid"
            )
    if python_records != sorted(python_records, key=python_record_sort_key):
        raise CleanRuntimeInventoryError("clean-runtime Python inventory is not sorted")
    if debian_records != sorted(debian_records, key=debian_record_sort_key):
        raise CleanRuntimeInventoryError("clean-runtime Debian inventory is not sorted")
    if len(python_identities(python_records)) != len(python_records):
        raise CleanRuntimeInventoryError(
            "clean-runtime Python inventory contains duplicates"
        )
    if len(debian_identities(debian_records)) != len(debian_records):
        raise CleanRuntimeInventoryError(
            "clean-runtime Debian inventory contains duplicates"
        )


def python_identities(records: list[dict]) -> set[tuple[str, str]]:
    """Return normalized Python identities for exact-set comparison."""
    return {(record["name"], record["version"]) for record in records}


def debian_identities(records: list[dict]) -> set[tuple[str, str, str]]:
    """Return Debian identities for exact-set comparison."""
    return {
        (record["name"], record["version"], record["architecture"])
        for record in records
    }


def python_record_sort_key(record: dict) -> tuple[bytes, bytes]:
    """Return the stable UTF-8 sort key for Python identities or artifacts."""
    return record["name"].encode(), record["version"].encode()


def debian_record_sort_key(record: dict) -> tuple[bytes, bytes, bytes]:
    """Return the stable UTF-8 sort key for Debian identities or artifacts."""
    return (
        record["name"].encode(),
        record["version"].encode(),
        record["architecture"].encode(),
    )


def _identity_field(value: object, label: str) -> str:
    """Return one bounded path-free installed-package identity field."""
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
        raise CleanRuntimeInventoryError(f"{label} is invalid")
    return value
