#!/usr/bin/env python3
"""Bounded host and container memory sampling for Diffusers proof runs."""

from __future__ import annotations

import re
import subprocess
import threading
from pathlib import Path

MEMORY_SAMPLE_SECONDS = 0.1
CONTAINER_MEMORY_PATTERN = re.compile(r"^(\d+),\s*(\d+)\s*MiB$")


class MemorySampler:
    """Sample one container process plus host and NVIDIA unified-memory views."""

    def __init__(self, process_id: int) -> None:
        """Bind the live container-init PID without starting a thread."""
        self._process_id = process_id
        self._stop = threading.Event()
        self._thread = threading.Thread(
            target=self._run, name="bottie-proof-memory", daemon=True
        )
        self.host_available_min = host_available_bytes()
        self.process_rss_peak = 0
        self.nvidia_process_peak: int | None = None

    def start(self) -> None:
        """Start bounded background sampling."""
        self._thread.start()

    def finish(self) -> None:
        """Stop sampling and wait for the sampler to finish."""
        self._stop.set()
        self._thread.join()

    def _run(self) -> None:
        """Retain only peak and minimum numeric measurements."""
        while not self._stop.wait(MEMORY_SAMPLE_SECONDS):
            self.host_available_min = min(
                self.host_available_min, host_available_bytes()
            )
            self.process_rss_peak = max(
                self.process_rss_peak, process_rss_bytes(self._process_id)
            )
            nvidia_process_memory = nvidia_process_bytes(self._process_id)
            if nvidia_process_memory is not None:
                self.nvidia_process_peak = max(
                    self.nvidia_process_peak or 0, nvidia_process_memory
                )


def host_available_bytes() -> int:
    """Read Linux MemAvailable without depending on a locale-sensitive command."""
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1]) * 1024
    raise RuntimeError("host memory availability is missing")


def process_rss_bytes(process_id: int) -> int:
    """Read the worker process's resident high-water mark when still live."""
    try:
        lines = Path(f"/proc/{process_id}/status").read_text().splitlines()
    except FileNotFoundError:
        return 0
    for line in lines:
        if line.startswith("VmHWM:"):
            return int(line.split()[1]) * 1024
    return 0


def nvidia_process_bytes(process_id: int) -> int | None:
    """Read NVIDIA's per-process unified-memory accounting when available."""
    result = subprocess.run(
        [
            "nvidia-smi",
            "--query-compute-apps=pid,used_memory",
            "--format=csv,noheader,nounits",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    for line in result.stdout.splitlines():
        match = CONTAINER_MEMORY_PATTERN.match(line.strip())
        if match and int(match.group(1)) == process_id:
            return int(match.group(2)) * 1024 * 1024
    return None


def container_process_id(name: str) -> int:
    """Return the live container init PID."""
    result = subprocess.run(
        ["docker", "inspect", "--format", "{{.State.Pid}}", name],
        check=True,
        capture_output=True,
        text=True,
    )
    process_id = int(result.stdout.strip())
    if process_id <= 0:
        raise RuntimeError("container process identity is unavailable")
    return process_id
