"""Fail-closed UCC ablation evidence for Bottie's Linux Diffusers proof."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

TRACE_FILE_LIMIT = 16 * 1024 * 1024
UCC_CONTAINER_PATH = "/opt/hpcx/ucc"
UCC_PROCESS_MAP_MARKERS = (b"/opt/hpcx/ucc/", b"libucc")


def assert_no_ucc_process_maps(process_maps: bytes) -> None:
    """Reject ablation evidence when any HPC-X UCC file remains mapped."""
    normalized = process_maps.lower()
    if any(marker in normalized for marker in UCC_PROCESS_MAP_MARKERS):
        raise RuntimeError("UCC runtime remained mapped after ablation")


def assert_ucc_installation_masked(name: str) -> None:
    """Require the live container's masked HPC-X UCC installation to be empty."""
    code = (
        "import os; "
        f"path={UCC_CONTAINER_PATH!r}; "
        "raise SystemExit(not os.path.isdir(path) or bool(os.listdir(path)))"
    )
    result = subprocess.run(
        ["docker", "exec", name, "python", "-c", code],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        raise RuntimeError("UCC installation ablation is not active")


def assert_ucc_installation_absent(name: str, python_executable: str) -> None:
    """Require the clean container to have no HPC-X UCC installation to mask."""
    code = f"import os; raise SystemExit(os.path.exists({UCC_CONTAINER_PATH!r}))"
    result = subprocess.run(
        ["docker", "exec", name, python_executable, "-c", code],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        raise RuntimeError("UCC installation is present in the clean runtime")


def capture_process_maps(
    name: str,
    destination: Path | None = None,
    reject_ucc: bool = False,
) -> None:
    """Validate and optionally persist one bounded native-library snapshot."""
    result = subprocess.run(
        ["docker", "exec", name, "cat", "/proc/1/maps"],
        check=False,
        capture_output=True,
    )
    if (
        result.returncode != 0
        or not result.stdout
        or len(result.stdout) > TRACE_FILE_LIMIT
    ):
        raise RuntimeError("worker process maps could not be captured")
    if reject_ucc:
        assert_no_ucc_process_maps(result.stdout)
    if destination is None:
        return
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(result.stdout)
