#!/usr/bin/env python3
"""Build and compare two network-disabled Linux Diffusers runtime images."""

from __future__ import annotations

import hashlib
import json
import re
import stat
import tarfile
from pathlib import Path, PurePosixPath

from diffusers_clean_runtime_lock import CLEAN_BASE_IMAGE_DIGEST


SCHEMA_VERSION = 1
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
IMAGE_ID_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")
BUILD_NAME_PATTERN = re.compile(r"[a-z0-9][a-z0-9._-]{0,127}")
PLAN_FIELDS = {
    "schemaVersion",
    "baseImageDigest",
    "inputLockSha256",
    "target",
    "workerSources",
}
SOURCE_FIELDS = {"filename", "byteSize", "sha256"}
REVIEWED_WORKER_SOURCES = (
    "diffusers_pytorch_worker.py",
    "diffusers_worker.py",
    "mlx_worker.py",
)
RUNTIME_GENERATED_FILES = {
    "etc/hostname",
    "etc/hosts",
    "etc/resolv.conf",
}
READ_CHUNK_BYTES = 1024 * 1024


class CleanRuntimeRebuildError(RuntimeError):
    """Stable failure raised when offline rebuild evidence is not exact."""


def validate_rebuild_plan(
    plan: dict, source_root: Path, input_lock_sha256: str
) -> tuple[str, ...]:
    """Validate the closed rebuild plan and every reviewed worker-source byte."""
    if not isinstance(plan, dict) or set(plan) != PLAN_FIELDS:
        raise CleanRuntimeRebuildError("rebuild plan schema is not exact")
    if (
        plan.get("schemaVersion") != SCHEMA_VERSION
        or plan.get("baseImageDigest") != CLEAN_BASE_IMAGE_DIGEST
        or plan.get("target") != {"operatingSystem": "linux", "architecture": "arm64"}
    ):
        raise CleanRuntimeRebuildError("rebuild plan identity has drifted")
    if (
        not isinstance(input_lock_sha256, str)
        or not SHA256_PATTERN.fullmatch(input_lock_sha256)
        or plan.get("inputLockSha256") != input_lock_sha256
    ):
        raise CleanRuntimeRebuildError("rebuild plan input lock has drifted")
    records = plan.get("workerSources")
    if (
        not isinstance(records, list)
        or len(records) != len(REVIEWED_WORKER_SOURCES)
        or any(
            not isinstance(record, dict) or set(record) != SOURCE_FIELDS
            for record in records
        )
        or tuple(record["filename"] for record in records) != REVIEWED_WORKER_SOURCES
    ):
        raise CleanRuntimeRebuildError("rebuild source allowlist is not exact")
    try:
        root = source_root.resolve(strict=True)
    except OSError as error:
        raise CleanRuntimeRebuildError("rebuild source root is unavailable") from error
    if not source_root.is_absolute() or source_root.is_symlink() or not root.is_dir():
        raise CleanRuntimeRebuildError("rebuild source root is invalid")
    for record in records:
        filename = record["filename"]
        if not isinstance(filename, str) or Path(filename).name != filename:
            raise CleanRuntimeRebuildError("rebuild source allowlist is not exact")
        path = root / filename
        try:
            status = path.stat(follow_symlinks=False)
            byte_size, sha256 = _measure_file(path)
        except OSError as error:
            raise CleanRuntimeRebuildError(
                "rebuild source bytes are unavailable"
            ) from error
        if (
            path.is_symlink()
            or not stat.S_ISREG(status.st_mode)
            or record["byteSize"] != byte_size
            or record["sha256"] != sha256
        ):
            raise CleanRuntimeRebuildError("rebuild source bytes have drifted")
    return REVIEWED_WORKER_SOURCES


def _render_dockerfile() -> str:
    """Render the fixed offline recipe without indexes, URLs, or mutable image tags."""
    return f"""FROM ubuntu@{CLEAN_BASE_IMAGE_DIGEST}

ENV DEBIAN_FRONTEND=noninteractive \\
    HF_HUB_OFFLINE=1 \\
    HF_HUB_DISABLE_TELEMETRY=1 \\
    PIP_DISABLE_PIP_VERSION_CHECK=1 \\
    PYTHONDONTWRITEBYTECODE=1 \\
    TRANSFORMERS_OFFLINE=1

COPY --from=frozen-inputs /debian/ /tmp/debian/
RUN (dpkg -i /tmp/debian/*.deb || dpkg -i /tmp/debian/*.deb || dpkg -i /tmp/debian/*.deb) \\
    && rm -rf /tmp/debian /var/lib/apt/lists/* \\
        /var/log/alternatives.log /var/log/apt/* /var/log/dpkg.log \\
    && python3 -m venv /opt/bottie/venv

COPY --from=frozen-inputs /python/ /tmp/python/
RUN /opt/bottie/venv/bin/python -m pip install \\
    --no-index --no-deps --no-compile /tmp/python/*.whl \\
    && find /opt/bottie/venv -type d -name __pycache__ -prune -exec rm -rf '{{}}' + \\
    && rm -f /var/cache/ldconfig/aux-cache \\
    && rm -rf /tmp/python /root/.cache

COPY diffusers_pytorch_worker.py diffusers_worker.py mlx_worker.py /opt/bottie/
WORKDIR /opt/bottie
ENTRYPOINT ["/opt/bottie/venv/bin/python", "/opt/bottie/diffusers_pytorch_worker.py"]
"""


def _build_command(context: Path, inputs: Path, build_name: str) -> list[str]:
    """Return one BuildKit command with immutable local contexts and no network."""
    if not context.is_absolute() or not inputs.is_absolute():
        raise CleanRuntimeRebuildError("rebuild paths must be absolute")
    if not BUILD_NAME_PATTERN.fullmatch(build_name):
        raise CleanRuntimeRebuildError("rebuild name is invalid")
    return [
        "docker",
        "buildx",
        "build",
        "--network",
        "none",
        "--no-cache",
        "--pull=false",
        "--platform",
        "linux/arm64",
        "--build-context",
        f"frozen-inputs={inputs}",
        "--file",
        str(context / "Dockerfile"),
        "--tag",
        build_name,
        "--load",
        str(context),
    ]


def _collect_regular_files(source) -> list[tuple[str, int, int, int, int, str]]:
    """Normalize exported rootfs regular paths, ownership, modes, sizes, and bytes."""
    regular: dict[str, tuple[int, int, int, int, str]] = {}
    hardlinks: list[tuple[str, str]] = []
    seen_paths: set[str] = set()
    try:
        with tarfile.open(fileobj=source, mode="r|*") as archive:
            for member in archive:
                if member.isdir() and member.name in {".", "./"}:
                    continue
                path = _rootfs_path(member.name)
                if path in RUNTIME_GENERATED_FILES:
                    continue
                if path in seen_paths:
                    raise CleanRuntimeRebuildError("exported rootfs path is duplicated")
                seen_paths.add(path)
                if member.isreg():
                    extracted = archive.extractfile(member)
                    if extracted is None:
                        raise CleanRuntimeRebuildError(
                            "exported regular file is unreadable"
                        )
                    digest = hashlib.sha256()
                    byte_size = 0
                    while chunk := extracted.read(READ_CHUNK_BYTES):
                        byte_size += len(chunk)
                        digest.update(chunk)
                    if byte_size != member.size:
                        raise CleanRuntimeRebuildError(
                            "exported regular file is truncated"
                        )
                    regular[path] = (
                        member.mode & 0o7777,
                        member.uid,
                        member.gid,
                        byte_size,
                        digest.hexdigest(),
                    )
                elif member.islnk():
                    hardlinks.append((path, _rootfs_path(member.linkname)))
    except (tarfile.TarError, OSError) as error:
        raise CleanRuntimeRebuildError("exported rootfs is malformed") from error
    unresolved = list(hardlinks)
    while unresolved:
        remaining = []
        progressed = False
        for path, target in unresolved:
            content = regular.get(target)
            if content is None:
                remaining.append((path, target))
                continue
            regular[path] = content
            progressed = True
        if not progressed:
            raise CleanRuntimeRebuildError("exported rootfs hard link is unresolved")
        unresolved = remaining
    return [(path, *regular[path]) for path in sorted(regular, key=str.encode)]


def _compare_rebuilds(first: dict, second: dict, input_lock_sha256: str) -> dict:
    """Require independent names and byte-equivalent package and regular-file results."""
    if first.get("buildName") == second.get("buildName"):
        raise CleanRuntimeRebuildError("rebuild names are not independent")
    if not all(
        isinstance(result.get("buildName"), str)
        and BUILD_NAME_PATTERN.fullmatch(result["buildName"])
        and isinstance(result.get("imageId"), str)
        and IMAGE_ID_PATTERN.fullmatch(result["imageId"])
        for result in (first, second)
    ):
        raise CleanRuntimeRebuildError("rebuild result identity is invalid")
    if first.get("inventory") != second.get("inventory"):
        raise CleanRuntimeRebuildError("rebuild package inventory has drifted")
    files = first.get("regularFiles")
    if files != second.get("regularFiles") or not isinstance(files, list):
        raise CleanRuntimeRebuildError("rebuild normalized filesystem has drifted")
    total_bytes = sum(record[4] for record in files)
    measurement = hashlib.sha256(
        b"".join(
            json.dumps(record, separators=(",", ":")).encode() + b"\n"
            for record in files
        )
    ).hexdigest()
    return {
        "schemaVersion": SCHEMA_VERSION,
        "baseImageDigest": CLEAN_BASE_IMAGE_DIGEST,
        "inputLockSha256": input_lock_sha256,
        "target": {"operatingSystem": "linux", "architecture": "arm64"},
        "rebuildsAgree": True,
        "builds": [
            {"buildName": result["buildName"], "imageId": result["imageId"]}
            for result in (first, second)
        ],
        "pythonDistributionCount": len(first["inventory"]["pythonDistributions"]),
        "debianPackageCount": len(first["inventory"]["debianPackages"]),
        "regularFileCount": len(files),
        "regularFileBytes": total_bytes,
        "normalizedFilesystemSha256": measurement,
    }


def _rootfs_path(value: str) -> str:
    """Return one canonical relative POSIX rootfs path."""
    if not isinstance(value, str) or not value or "\\" in value:
        raise CleanRuntimeRebuildError("exported rootfs path is invalid")
    path = PurePosixPath(value)
    parts = tuple(part for part in path.parts if part != ".")
    if path.is_absolute() or not parts or any(part in {"", ".."} for part in parts):
        raise CleanRuntimeRebuildError("exported rootfs path is invalid")
    return "/".join(parts)


def _measure_file(path: Path) -> tuple[int, str]:
    """Return one regular file's byte count and SHA-256."""
    digest = hashlib.sha256()
    byte_size = 0
    with path.open("rb") as source:
        while chunk := source.read(READ_CHUNK_BYTES):
            byte_size += len(chunk)
            digest.update(chunk)
    return byte_size, digest.hexdigest()
