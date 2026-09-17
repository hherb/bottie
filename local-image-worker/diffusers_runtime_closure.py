#!/usr/bin/env python3
"""Derive a path-free runtime closure from one traced Diffusers proof run."""

from __future__ import annotations

import hashlib
import json
import os
import re
from collections import defaultdict
from pathlib import Path

from diffusers_bundle_candidate import verify_proof_inputs
from diffusers_license_review import LicenseReviewError, validate_license_review
from diffusers_bundle_environment import (
    _collect_environment_measurement,
    _verify_environment_contents,
)
from diffusers_clean_runtime_closure import (
    CleanRuntimeClosureError,
    clean_installed_python_requirements,
    clean_python_file_owners,
    collect_clean_environment_measurement,
    read_clean_elf_dependencies,
)
from diffusers_runtime_native import (
    RuntimeNativeEvidenceError,
    ambiguous_elf_dependency_blockers,
    host_driver_soname,
    parse_elf_dependencies,
    validated_soname,
    verified_native_components,
)
from diffusers_runtime_ownership import (
    RuntimeOwnershipError,
    component_identity,
    file_owner,
    installed_python_requirements,
    native_file_owners,
    python_file_owners,
)
from diffusers_runtime_closure_profiles import (
    DEFAULT_RUNTIME_CLOSURE_PROFILE,
    NGC_RUNTIME_PROFILE_NAME,
    ClosureProfileError,
    runtime_closure_profile,
)
from diffusers_runtime_trace_evidence import (
    ClosureEvidenceError,
    absolute_trace_path as _absolute_trace_path,
    apply_external_license_sources,
    load_trace_context as _load_trace_context,
    measure_observed_file as _measure_observed_file,
    read_elf_dependencies as _read_elf_dependencies,
    read_bounded_text as _read_bounded_text,
    verified_external_license_sources,
)


SCHEMA_VERSION = 1
FIRST_PARTY_FILES = {
    Path("/opt/bottie/diffusers_worker.py"),
}
EXCLUDED_RUNTIME_PREFIXES = (
    Path("/dev"),
    Path("/model"),
    Path("/output"),
    Path("/proc"),
    Path("/runtime-trace"),
    Path("/sys"),
    Path("/tmp"),
    Path("/trace-source"),
)
HOST_DRIVER_SONAMES = frozenset(
    {
        "libcuda.so.1",
        "libnvidia-allocator.so.1",
        "libnvidia-gpucomp.so.1",
        "libnvidia-ml.so.1",
        "libnvidia-nvvm.so.4",
        "libnvidia-ptxjitcompiler.so.1",
    }
)
TRACE_KINDS = frozenset({"dlopen", "module", "open"})
CLOSURE_TRACE_KINDS = frozenset({"dlopen", "module"})
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")


def parse_trace_paths(contents: str) -> list[Path]:
    """Return unique canonical absolute paths from bounded JSON-line audit events."""
    paths = set()
    for line in contents.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ClosureEvidenceError("runtime trace is malformed") from error
        if not isinstance(event, dict) or set(event) != {"kind", "path"}:
            raise ClosureEvidenceError("runtime trace schema is not closed")
        if event["kind"] not in TRACE_KINDS:
            raise ClosureEvidenceError("runtime trace kind is unsupported")
        path = _absolute_trace_path(event["path"])
        if event["kind"] in CLOSURE_TRACE_KINDS:
            paths.add(path)
    return sorted(paths, key=lambda path: os.fsencode(path))


def parse_process_maps(contents: str) -> list[Path]:
    """Return unique real file paths from one Linux process-maps snapshot."""
    paths = set()
    for line in contents.splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) < 6 or not fields[5].startswith("/"):
            continue
        raw_path = fields[5]
        if raw_path.endswith(" (deleted)"):
            deleted_path = raw_path.removesuffix(" (deleted)")
            if deleted_path == "/dev/zero" or deleted_path.startswith("/dev/shm/sem."):
                continue
            raise ClosureEvidenceError("process maps contain a deleted runtime file")
        paths.add(_absolute_trace_path(raw_path))
    return sorted(paths, key=lambda path: os.fsencode(path))


def build_closure_review(
    files: list[dict],
    environment_components: dict[str, dict],
    host_driver_sonames: set[str] | frozenset[str],
    observed_host_driver_sonames: set[str],
    missing_elf_dependencies: set[str],
    required_component_ids: set[str] | None = None,
    dependency_blockers: set[str] | None = None,
    license_review_components: dict[str, dict] | None = None,
) -> dict:
    """Build a deterministic review and keep assembly closed until every check passes."""
    license_review_components = license_review_components or {}
    _validate_file_records(files)
    used_component_ids = sorted(
        (
            {
                file["owner"]
                for file in files
                if isinstance(file["owner"], str)
                and not file["owner"].startswith(("first-party:", "host-driver:"))
            }
            | (required_component_ids or set())
        ),
        key=str.encode,
    )
    missing_components = [
        identity
        for identity in used_component_ids
        if identity not in environment_components
    ]
    if missing_components:
        raise ClosureEvidenceError(
            "runtime owner is absent from the complete environment"
        )
    components = []
    blockers = []
    for identity in used_component_ids:
        environment = environment_components[identity]
        owned_files = [file for file in files if file["owner"] == identity]
        component_review = license_review_components.get(identity)
        reviewed_expression = (
            component_review["reviewedLicenseExpression"] if component_review else None
        )
        license_files = (
            component_review["licenseFiles"]
            if component_review
            else environment.get("licenseFiles", [])
        )
        if not license_files:
            blockers.append(f"{identity}:missing-license-bytes")
        if (
            environment.get("licenseExpression") == "undeclared"
            and reviewed_expression is None
        ):
            blockers.append(f"{identity}:undeclared-license")
        if reviewed_expression is None:
            blockers.append(f"{identity}:unreviewed-license-expression")
        component = {
            "identity": identity,
            "fileCount": len(owned_files),
            "byteSize": sum(file["byteSize"] for file in owned_files),
            "licenseFileCount": len(license_files),
            "declaredLicenseExpression": environment.get("licenseExpression"),
            "reviewedLicenseExpression": reviewed_expression,
        }
        if component_review is not None:
            component["reviewEvidence"] = component_review["reviewEvidence"]
        if environment.get("provenance") is not None:
            component["provenance"] = environment["provenance"]
        components.append(component)
    for file in files:
        if file["owner"] is None:
            blockers.append(f'unowned-file:{file["sha256"]}')
    for soname in missing_elf_dependencies:
        blockers.append(f"elf-dependency:{validated_soname(soname)}:unresolved")
    blockers.extend(dependency_blockers or set())
    unexpected_host_drivers = observed_host_driver_sonames.difference(
        host_driver_sonames
    )
    for soname in unexpected_host_drivers:
        blockers.append(f"host-driver:{validated_soname(soname)}:outside-boundary")
    blockers = sorted(set(blockers), key=str.encode)
    closure_blockers = [
        blocker
        for blocker in blockers
        if blocker.startswith(
            (
                "elf-dependency:",
                "elf-soname:",
                "host-driver:",
                "python-dependency:",
                "unowned-file:",
            )
        )
    ]
    license_blockers = [
        blocker for blocker in blockers if blocker not in closure_blockers
    ]
    return {
        "closure": {
            "fileCount": len(files),
            "byteSize": sum(file["byteSize"] for file in files),
            "elfFileCount": sum(file["elf"] for file in files),
            "componentCount": len(components),
            "components": components,
        },
        "hostDriverBoundary": {
            "allowedSonames": sorted(host_driver_sonames, key=str.encode),
            "observedSonames": sorted(observed_host_driver_sonames, key=str.encode),
        },
        "blockers": blockers,
        "closureComplete": not closure_blockers,
        "licenseReviewed": not license_blockers,
        "assemblyEligible": not blockers,
        "distributionReviewed": False,
    }


def collect_runtime_closure(
    trace_root: Path,
    license_review_manifest: object | None = None,
    license_source_root: Path | None = None,
    profile_name: str = NGC_RUNTIME_PROFILE_NAME,
    clean_runtime_lock: Path | None = None,
) -> dict:
    """Classify one traced proof against the exact installed environment and ELF graph."""
    try:
        profile = runtime_closure_profile(profile_name)
    except ClosureProfileError as error:
        raise ClosureEvidenceError(str(error)) from error
    if not profile.allow_license_evidence and (
        license_review_manifest is not None or license_source_root is not None
    ):
        raise ClosureEvidenceError(
            "runtime closure profile does not accept licence evidence"
        )
    if profile.requires_clean_runtime_lock:
        if clean_runtime_lock is None:
            raise ClosureEvidenceError("clean-runtime lock is required")
        try:
            environment = collect_clean_environment_measurement(clean_runtime_lock)
        except CleanRuntimeClosureError as error:
            raise ClosureEvidenceError(str(error)) from error
    else:
        if clean_runtime_lock is not None:
            raise ClosureEvidenceError(
                "clean-runtime lock is not accepted by this profile"
            )
        _verify_environment_contents()
        verify_proof_inputs()
        environment = _collect_environment_measurement()
    first_party_files = (
        FIRST_PARTY_FILES
        if profile is DEFAULT_RUNTIME_CLOSURE_PROFILE
        else profile.first_party_files
    )
    context = _load_trace_context(trace_root / "context.json", profile)
    python_paths = parse_trace_paths(
        _read_bounded_text(trace_root / "python-paths.jsonl")
    )
    mapped_paths = parse_process_maps(
        _read_bounded_text(trace_root / "process-maps.txt")
    )
    observed_paths = set(python_paths).union(mapped_paths).union(first_party_files)
    observed_paths = {
        path for path in observed_paths if not _is_excluded_runtime_path(path)
    }
    components = _environment_components(environment)
    try:
        python_owners = (
            clean_python_file_owners()
            if profile.requires_clean_runtime_lock
            else python_file_owners()
        )
        native_owners = native_file_owners(environment["nativeComponents"])
        if profile.use_ngc_native_components:
            unmanaged_components, unmanaged_owners = verified_native_components(
                license_source_components=components
            )
        else:
            unmanaged_components, unmanaged_owners = {}, {}
    except (RuntimeOwnershipError, RuntimeNativeEvidenceError) as error:
        raise ClosureEvidenceError(str(error)) from error
    if components.keys() & unmanaged_components.keys():
        raise ClosureEvidenceError(
            "native component identity conflicts with the complete environment"
        )
    components.update(unmanaged_components)
    if license_source_root is not None:
        external_sources = verified_external_license_sources(license_source_root)
        components = apply_external_license_sources(components, external_sources)
    for path, owners in unmanaged_owners.items():
        native_owners[path].update(owners)
    files = []
    measured_paths: dict[Path, tuple[int, str | None]] = {}
    elf_sonames: dict[str, set[str]] = defaultdict(set)
    elf_needed: set[str] = set()
    observed_host_driver_sonames = set()
    elf_dependency_reader = (
        read_clean_elf_dependencies
        if profile.requires_clean_runtime_lock
        else _read_elf_dependencies
    )
    for path in sorted(observed_paths, key=lambda item: os.fsencode(item)):
        measured = _measure_observed_file(path)
        duplicate = measured_paths.get(measured["resolvedPath"])
        if duplicate is not None:
            file_index, soname = duplicate
            driver_soname = host_driver_soname(path.name, soname, HOST_DRIVER_SONAMES)
            if path in mapped_paths and driver_soname is not None:
                files[file_index]["owner"] = f"host-driver:nvidia-{driver_soname}"
                observed_host_driver_sonames.add(driver_soname)
            continue
        try:
            owner = file_owner(
                path,
                measured["resolvedPath"],
                first_party_files,
                python_owners,
                native_owners,
            )
        except RuntimeOwnershipError as error:
            raise ClosureEvidenceError(str(error)) from error
        elf = measured["elf"]
        soname = None
        if elf:
            try:
                needed, soname = elf_dependency_reader(measured["resolvedPath"])
            except CleanRuntimeClosureError as error:
                raise ClosureEvidenceError(str(error)) from error
            elf_needed.update(needed)
            names = {path.name, measured["resolvedPath"].name}
            if soname:
                names.add(soname)
            for name in names:
                elf_sonames[name].add(measured["sha256"])
        driver_soname = host_driver_soname(path.name, soname, HOST_DRIVER_SONAMES)
        if path in mapped_paths and driver_soname is not None:
            owner = f"host-driver:nvidia-{driver_soname}"
            observed_host_driver_sonames.add(driver_soname)
        files.append(
            {
                "sha256": measured["sha256"],
                "byteSize": measured["byteSize"],
                "owner": owner,
                "elf": elf,
            }
        )
        measured_paths[measured["resolvedPath"]] = (len(files) - 1, soname)
    missing_elf_dependencies = {
        needed
        for needed in elf_needed
        if needed not in elf_sonames and needed not in HOST_DRIVER_SONAMES
    }
    ambiguous_elf_dependencies = ambiguous_elf_dependency_blockers(
        elf_needed, elf_sonames
    )
    imported_python_components = {
        file["owner"]
        for file in files
        if isinstance(file["owner"], str) and file["owner"].startswith("python:")
    }
    try:
        dependency_resolver = (
            clean_installed_python_requirements
            if profile.requires_clean_runtime_lock
            else installed_python_requirements
        )
        required_components, dependency_blockers = dependency_resolver(
            imported_python_components
        )
    except RuntimeOwnershipError as error:
        raise ClosureEvidenceError(str(error)) from error
    license_review_components = None
    if license_review_manifest is not None:
        used_component_ids = {
            file["owner"]
            for file in files
            if isinstance(file["owner"], str)
            and not file["owner"].startswith(("first-party:", "host-driver:"))
        }.union(required_components)
        try:
            license_review_components = validate_license_review(
                license_review_manifest,
                used_component_ids,
                profile.derived_image_digest,
                {
                    "pythonTraceSha256": context["pythonTraceSha256"],
                    "processMapsSha256": context["processMapsSha256"],
                },
            )
        except LicenseReviewError as error:
            raise ClosureEvidenceError(str(error)) from error
    review = build_closure_review(
        files,
        components,
        HOST_DRIVER_SONAMES,
        observed_host_driver_sonames,
        missing_elf_dependencies,
        required_component_ids=required_components,
        dependency_blockers=dependency_blockers.union(ambiguous_elf_dependencies),
        license_review_components=license_review_components,
    )
    result = {
        "schemaVersion": SCHEMA_VERSION,
        "workerVersion": profile.identity.worker_version,
        "runtimeId": profile.identity.runtime_id,
        "modelId": profile.identity.model_id,
        "modelRevision": profile.identity.model_revision,
        "target": {
            "operatingSystem": "linux",
            "architecture": profile.target_architecture,
        },
        "pythonVersion": profile.python_version,
        "trace": context,
        "environment": {
            "pythonComponentCount": len(environment["pythonComponents"]),
            "nativeComponentCount": len(environment["nativeComponents"]),
            "unmanagedNativeComponentCount": len(unmanaged_components),
        },
        **review,
    }
    if profile is not DEFAULT_RUNTIME_CLOSURE_PROFILE:
        result["runtimeFilesSha256"] = hashlib.sha256(
            json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
    return result


def _validate_file_records(files: list[dict]) -> None:
    """Require exact path-free file records without duplicate bytes posing as files."""
    for file in files:
        if set(file) != {"sha256", "byteSize", "owner", "elf"}:
            raise ClosureEvidenceError("runtime file schema is not closed")
        if (
            not isinstance(file["sha256"], str)
            or SHA256_PATTERN.fullmatch(file["sha256"]) is None
        ):
            raise ClosureEvidenceError("runtime file digest is invalid")
        if not isinstance(file["byteSize"], int) or isinstance(file["byteSize"], bool):
            raise ClosureEvidenceError("runtime file byte size is invalid")
        if file["byteSize"] < 0 or not isinstance(file["elf"], bool):
            raise ClosureEvidenceError("runtime file measurement is invalid")
        if file["owner"] is not None and not isinstance(file["owner"], str):
            raise ClosureEvidenceError("runtime file owner is invalid")


def _environment_components(environment: dict) -> dict[str, dict]:
    """Index the separately complete package-manager record by stable identity."""
    components = {}
    for component in [
        *environment["pythonComponents"],
        *environment["nativeComponents"],
    ]:
        identity = component_identity(component)
        if identity in components:
            raise ClosureEvidenceError(
                "complete environment contains duplicate identities"
            )
        components[identity] = component
    return components


def _is_excluded_runtime_path(path: Path) -> bool:
    """Exclude separately verified model/output bytes and proof-only tracing instrumentation."""
    return any(
        path == prefix or prefix in path.parents for prefix in EXCLUDED_RUNTIME_PREFIXES
    )
