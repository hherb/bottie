"""Resolve the active installed Python-distribution dependency closure."""

from __future__ import annotations

from packaging.requirements import InvalidRequirement, Requirement
from packaging.utils import canonicalize_name
from packaging.version import InvalidVersion, Version


class RuntimeDependencyError(RuntimeError):
    """Stable failure raised when installed dependency evidence is ambiguous."""


def resolve_required_python_components(
    roots: set[str],
    requirements_by_component: dict[str, list[str]],
    installed_by_name: dict[str, str],
    marker_environment: dict[str, str],
) -> tuple[set[str], set[str]]:
    """Close active requirements from imported roots and report missing or mismatched pins."""
    required = set(roots)
    pending = [(identity, "") for identity in sorted(roots, key=str.encode)]
    processed = set()
    blockers = set()
    while pending:
        identity, active_extra = pending.pop(0)
        context = (identity, active_extra)
        if context in processed:
            continue
        processed.add(context)
        for raw_requirement in requirements_by_component.get(identity, []):
            try:
                requirement = Requirement(raw_requirement)
            except InvalidRequirement as error:
                raise RuntimeDependencyError("installed Python requirement is malformed") from error
            environment = {**marker_environment, "extra": active_extra}
            if requirement.marker is not None and not requirement.marker.evaluate(environment):
                continue
            canonical_name = str(canonicalize_name(requirement.name))
            dependency = installed_by_name.get(canonical_name)
            if dependency is None:
                blockers.add(f"python-dependency:{canonical_name}:missing")
                continue
            version = dependency.rsplit("@", 1)[-1]
            try:
                matches = not requirement.specifier or Version(version) in requirement.specifier
            except InvalidVersion as error:
                raise RuntimeDependencyError("installed Python version is malformed") from error
            if not matches:
                blockers.add(f"python-dependency:{canonical_name}:version-mismatch")
            required.add(dependency)
            requested_contexts = {(dependency, "")}.union(
                (dependency, extra) for extra in requirement.extras
            )
            pending.extend(requested_contexts.difference(processed).difference(pending))
            pending.sort(key=lambda item: (item[0].encode(), item[1].encode()))
    return required, blockers
