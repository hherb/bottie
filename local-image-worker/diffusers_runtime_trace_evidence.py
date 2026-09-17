"""Bounded trace parsing for exact Diffusers runtime-closure profiles."""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
from pathlib import Path, PurePosixPath

from diffusers_runtime_closure_profiles import (
    DEFAULT_RUNTIME_CLOSURE_PROFILE,
    RuntimeClosureProfile,
)
from diffusers_runtime_native import RuntimeNativeEvidenceError, parse_elf_dependencies


MAX_TRACE_BYTES = 16 * 1024 * 1024
MAX_TRACE_PATH_BYTES = 4_096
MAX_DYNAMIC_SECTION_BYTES = 2 * 1024 * 1024
HASH_BUFFER_BYTES = 1024 * 1024
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")


class ClosureEvidenceError(RuntimeError):
    """Stable failure raised when runtime-closure evidence is malformed or ambiguous."""


def absolute_trace_path(value: object) -> Path:
    """Validate one lexical absolute path without resolving host-specific symlinks."""
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode()) > MAX_TRACE_PATH_BYTES
    ):
        raise ClosureEvidenceError("runtime trace path is invalid")
    if "\x00" in value or "\n" in value or "\r" in value:
        raise ClosureEvidenceError("runtime trace path is invalid")
    path = PurePosixPath(value)
    if (
        not path.is_absolute()
        or path.as_posix() != value
        or any(part == ".." for part in path.parts)
    ):
        raise ClosureEvidenceError("runtime trace path is invalid")
    return Path(path.as_posix())


def read_bounded_text(path: Path) -> str:
    """Read one stable bounded trace file as UTF-8."""
    try:
        before = path.stat()
        if before.st_size > MAX_TRACE_BYTES:
            raise ClosureEvidenceError("runtime trace file is unstable or oversized")
        contents = path.read_bytes()
        after = path.stat()
    except OSError as error:
        raise ClosureEvidenceError("runtime trace file is unavailable") from error
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    observed = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if stable != observed or len(contents) > MAX_TRACE_BYTES:
        raise ClosureEvidenceError("runtime trace file is unstable or oversized")
    try:
        return contents.decode("utf-8")
    except UnicodeError as error:
        raise ClosureEvidenceError("runtime trace file is not UTF-8") from error


def load_trace_context(
    path: Path,
    profile: RuntimeClosureProfile = DEFAULT_RUNTIME_CLOSURE_PROFILE,
) -> dict:
    """Require trace bytes to bind one exact retained runtime identity."""
    context_contents = read_bounded_text(path)
    try:
        context = json.loads(context_contents)
    except json.JSONDecodeError as error:
        raise ClosureEvidenceError("runtime trace context is malformed") from error
    expected = {
        "imageId": profile.derived_image_digest,
        "workerVersion": profile.identity.worker_version,
        "runtimeId": profile.identity.runtime_id,
        "modelId": profile.identity.model_id,
        "modelRevision": profile.identity.model_revision,
    }
    if not isinstance(context, dict) or any(
        context.get(key) != value for key, value in expected.items()
    ):
        raise ClosureEvidenceError("runtime trace context identity has drifted")
    if set(context) != {*expected, "pythonTraceSha256", "processMapsSha256"}:
        raise ClosureEvidenceError("runtime trace context schema is not closed")
    for key in ("pythonTraceSha256", "processMapsSha256"):
        if (
            not isinstance(context[key], str)
            or SHA256_PATTERN.fullmatch(context[key]) is None
        ):
            raise ClosureEvidenceError("runtime trace context digest is invalid")
    for name, key in (
        ("python-paths.jsonl", "pythonTraceSha256"),
        ("process-maps.txt", "processMapsSha256"),
    ):
        contents = read_bounded_text(path.parent / name).encode()
        if hashlib.sha256(contents).hexdigest() != context[key]:
            raise ClosureEvidenceError("runtime trace contents have drifted")
    context_sha256 = hashlib.sha256(context_contents.encode()).hexdigest()
    if (
        profile.accepted_trace_context_sha256s
        and context_sha256 not in profile.accepted_trace_context_sha256s
    ):
        raise ClosureEvidenceError(
            "runtime trace context is not retained by this profile"
        )
    if profile is not DEFAULT_RUNTIME_CLOSURE_PROFILE:
        return {
            "traceContextSha256": context_sha256,
            "pythonTraceSha256": context["pythonTraceSha256"],
            "processMapsSha256": context["processMapsSha256"],
        }
    return context


def measure_observed_file(path: Path) -> dict:
    """Measure one stable regular file and detect ELF bytes without retaining its path."""
    try:
        resolved = path.resolve(strict=True)
        descriptor = os.open(resolved, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            os.close(descriptor)
            digest = hashlib.sha256(os.fsencode(path)).hexdigest()
            return {
                "resolvedPath": resolved,
                "sha256": digest,
                "byteSize": 0,
                "elf": False,
            }
        hasher = hashlib.sha256()
        byte_size = 0
        prefix = b""
        with os.fdopen(descriptor, "rb") as stream:
            while chunk := stream.read(HASH_BUFFER_BYTES):
                if not prefix:
                    prefix = chunk[:4]
                hasher.update(chunk)
                byte_size += len(chunk)
            after = os.fstat(stream.fileno())
    except OSError:
        digest = hashlib.sha256(os.fsencode(path)).hexdigest()
        return {"resolvedPath": path, "sha256": digest, "byteSize": 0, "elf": False}
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    observed = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if stable != observed or before.st_size != byte_size:
        raise ClosureEvidenceError("runtime file changed while being measured")
    return {
        "resolvedPath": resolved,
        "sha256": hasher.hexdigest(),
        "byteSize": byte_size,
        "elf": prefix == b"\x7fELF",
    }


def read_elf_dependencies(path: Path) -> tuple[set[str], str | None]:
    """Read one ELF dynamic section through the retained NGC image's readelf tool."""
    try:
        completed = subprocess.run(
            ["readelf", "-d", str(path)],
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise ClosureEvidenceError("readelf is unavailable") from error
    if completed.returncode != 0 or len(completed.stdout) > MAX_DYNAMIC_SECTION_BYTES:
        raise ClosureEvidenceError("ELF dynamic section cannot be measured")
    try:
        output = completed.stdout.decode("utf-8")
    except UnicodeError as error:
        raise ClosureEvidenceError("ELF dynamic section is not UTF-8") from error
    try:
        return parse_elf_dependencies(output)
    except RuntimeNativeEvidenceError as error:
        raise ClosureEvidenceError(str(error)) from error


def verified_external_license_sources(root: Path) -> dict:
    """Load the optional NGC source helper only when that legacy route requests it."""
    from diffusers_runtime_license_sources import (
        ExternalLicenseSourceError,
        verified_external_license_sources as verify_sources,
    )

    try:
        return verify_sources(root)
    except ExternalLicenseSourceError as error:
        raise ClosureEvidenceError(str(error)) from error


def apply_external_license_sources(components: dict, sources: dict) -> dict:
    """Apply optional NGC source evidence without importing it for the clean profile."""
    from diffusers_runtime_license_sources import (
        ExternalLicenseSourceError,
        apply_external_license_sources as apply_sources,
    )

    try:
        return apply_sources(components, sources)
    except ExternalLicenseSourceError as error:
        raise ClosureEvidenceError(str(error)) from error
