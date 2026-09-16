"""Map observed runtime files to installed Python and Debian component owners."""

from __future__ import annotations

import importlib.metadata as metadata
from collections import defaultdict
from pathlib import Path

from packaging.markers import default_environment
from packaging.utils import canonicalize_name

from diffusers_runtime_dependencies import (
    RuntimeDependencyError,
    resolve_required_python_components,
)


class RuntimeOwnershipError(RuntimeError):
    """Stable failure raised when installed ownership evidence is ambiguous."""


def python_file_owners() -> dict[Path, set[str]]:
    """Map every installed Python-distribution file to its exact component identity."""
    owners: dict[Path, set[str]] = defaultdict(set)
    for distribution in metadata.distributions():
        identity = _distribution_identity(distribution)
        for relative in distribution.files or []:
            path = Path(distribution.locate_file(relative))
            try:
                resolved = path.resolve(strict=True)
            except (FileNotFoundError, OSError):
                continue
            if resolved.is_file():
                owners[resolved].add(identity)
    return owners


def installed_python_requirements(roots: set[str]) -> tuple[set[str], set[str]]:
    """Resolve every active installed requirement recursively from observed package roots."""
    requirements = {}
    installed = {}
    for distribution in metadata.distributions():
        identity = _distribution_identity(distribution)
        name = distribution.metadata["Name"]
        canonical_name = str(canonicalize_name(name))
        if canonical_name in installed:
            raise RuntimeOwnershipError("installed Python distribution name is ambiguous")
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


def native_file_owners(components: list[dict]) -> dict[Path, set[str]]:
    """Map Debian package list entries to exact identities from the full environment."""
    owners: dict[Path, set[str]] = defaultdict(set)
    info_root = Path("/var/lib/dpkg/info")
    for component in components:
        identity = component_identity(component)
        name = component["name"]
        architecture = component["architecture"]
        candidates = [info_root / f"{name}:{architecture}.list", info_root / f"{name}.list"]
        list_path = next((candidate for candidate in candidates if candidate.is_file()), None)
        if list_path is None:
            continue
        try:
            entries = list_path.read_text(encoding="utf-8").splitlines()
        except OSError as error:
            raise RuntimeOwnershipError("Debian ownership list cannot be read") from error
        for entry in entries:
            path = Path(entry)
            try:
                path.lstat()
            except (FileNotFoundError, OSError):
                continue
            if path.is_symlink() or path.is_file():
                owners[path].add(identity)
    return owners


def file_owner(
    observed: Path,
    resolved: Path,
    first_party_files: set[Path],
    python_owners: dict[Path, set[str]],
    native_owners: dict[Path, set[str]],
) -> str | None:
    """Resolve one runtime file to exactly one first-party or package-manager owner."""
    if observed in first_party_files:
        return "first-party:bottie-worker"
    native = native_owners.get(observed)
    owners = python_owners.get(resolved, set()).union(
        native if native is not None else native_owners.get(resolved, set())
    )
    if len(owners) > 1:
        identities = ",".join(sorted(owners, key=str.encode))
        raise RuntimeOwnershipError(f"runtime file has ambiguous package ownership: {identities}")
    return next(iter(owners), None)


def component_identity(component: dict) -> str:
    """Return the path-free ecosystem, normalized name, and exact version identity."""
    return f'{component["ecosystem"]}:{component["name"]}@{component["version"]}'


def _distribution_identity(distribution: metadata.Distribution) -> str:
    """Return one installed Python distribution's exact normalized identity."""
    name = distribution.metadata.get("Name")
    version = distribution.version
    if not name or not version:
        raise RuntimeOwnershipError("Python distribution identity is incomplete")
    return f"python:{name.lower()}@{version}"
