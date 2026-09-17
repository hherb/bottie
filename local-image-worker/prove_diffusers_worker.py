#!/usr/bin/env python3
"""Measure Bottie's pinned Diffusers worker on one named Linux NVIDIA host."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import select
import shutil
import struct
import subprocess
import tempfile
import threading
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO

from PIL import Image

from diffusers_pytorch_worker import PYTORCH_WORKER_IDENTITY
from diffusers_ucc_ablation import (
    UCC_CONTAINER_PATH,
    assert_no_ucc_process_maps,
    assert_ucc_installation_masked,
    capture_process_maps,
)
from diffusers_worker import DIFFUSERS_WORKER_IDENTITY, MODEL_ID, MODEL_REVISION
from mlx_worker import WorkerIdentity

FRAME_LIMIT = 1024 * 1024
PROOF_PROMPT = "A violet glass robot tending a tiny greenhouse, detailed botanical illustration"
PROOF_DIMENSIONS = (512, 512)
COMPLETE_SEED = 42
CANCEL_SEED = 43
NETWORK_PROBE_ADDRESS = ("1.1.1.1", 443)
MEMORY_SAMPLE_SECONDS = 0.1
HANDSHAKE_TIMEOUT_SECONDS = 30
EVENT_TIMEOUT_SECONDS = 30 * 60
CONTAINER_MEMORY_PATTERN = re.compile(r"^(\d+),\s*(\d+)\s*MiB$")
TRACE_WRAPPER = (
    "import sys; "
    "sys.path.insert(0, '/trace-source'); "
    "import diffusers_runtime_trace; "
    "sys.path.pop(0); "
    "import runpy; "
    "worker = sys.argv.pop(1); "
    "runpy.run_path(worker, run_name='__main__')"
)


@dataclass(frozen=True)
class RuntimeProfile:
    """Bind one proof selection to its exact worker identity and in-image script."""

    identity: WorkerIdentity
    worker_script: str


RUNTIME_PROFILES = {
    "ngc-25.11": RuntimeProfile(DIFFUSERS_WORKER_IDENTITY, "/opt/bottie/diffusers_worker.py"),
    "pytorch-2.10-cu130": RuntimeProfile(
        PYTORCH_WORKER_IDENTITY,
        "/opt/bottie/diffusers_pytorch_worker.py",
    ),
}
DEFAULT_RUNTIME_PROFILE = RUNTIME_PROFILES["ngc-25.11"]


class MemorySampler:
    """Sample one container process plus host and NVIDIA unified-memory views."""

    def __init__(self, process_id: int) -> None:
        """Bind the live container-init PID without starting a thread."""
        self._process_id = process_id
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="bottie-proof-memory", daemon=True)
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
            self.host_available_min = min(self.host_available_min, host_available_bytes())
            self.process_rss_peak = max(self.process_rss_peak, process_rss_bytes(self._process_id))
            nvidia_process_memory = nvidia_process_bytes(self._process_id)
            if nvidia_process_memory is not None:
                self.nvidia_process_peak = max(
                    self.nvidia_process_peak or 0, nvidia_process_memory
                )


def write_frame(stream: BinaryIO, message: dict) -> None:
    """Write one bounded private-protocol frame."""
    payload = json.dumps(message, separators=(",", ":")).encode()
    if not payload or len(payload) > FRAME_LIMIT:
        raise ValueError("invalid proof frame")
    stream.write(struct.pack(">I", len(payload)) + payload)
    stream.flush()


def read_frame(stream: BinaryIO) -> dict:
    """Read one bounded private-protocol frame."""
    prefix = read_exact(stream, 4)
    length = struct.unpack(">I", prefix)[0]
    if length == 0 or length > FRAME_LIMIT:
        raise ValueError("invalid worker frame")
    value = json.loads(read_exact(stream, length))
    if not isinstance(value, dict):
        raise ValueError("worker frame is not an object")
    return value


def read_frame_with_timeout(stream: BinaryIO, timeout_seconds: int) -> dict:
    """Read one frame only after its local pipe becomes readable within a fixed deadline."""
    readable, _, _ = select.select([stream], [], [], timeout_seconds)
    if not readable:
        raise TimeoutError("worker event deadline expired")
    return read_frame(stream)


def read_exact(stream: BinaryIO, length: int) -> bytes:
    """Read exactly one frame segment or fail on early worker exit."""
    chunks = bytearray()
    while len(chunks) < length:
        chunk = stream.read(length - len(chunks))
        if not chunk:
            raise EOFError("worker exited before a complete frame")
        chunks.extend(chunk)
    return bytes(chunks)


def wait_for_result(process: subprocess.Popen, request_id: str, cancel_after_step: int | None = None):
    """Wait for one correlated terminal result, optionally cancelling at a denoising boundary."""
    cancellation_started = None
    while True:
        event = read_frame_with_timeout(process.stdout, EVENT_TIMEOUT_SECONDS)
        if event.get("requestId") != request_id:
            raise RuntimeError("worker returned an uncorrelated event")
        if (
            cancel_after_step is not None
            and cancellation_started is None
            and event.get("type") == "progress"
            and event.get("stage") == "denoising"
            and event.get("completedSteps", 0) >= cancel_after_step
        ):
            cancellation_started = time.monotonic()
            write_frame(
                process.stdin,
                {"type": "cancel", "protocolVersion": 1, "requestId": request_id},
            )
        if event.get("type") == "result":
            return event, cancellation_started


def generation_command(request_id: str, seed: int) -> dict:
    """Build the fixed single-output proof request."""
    return {
        "type": "generate",
        "protocolVersion": 1,
        "requestId": request_id,
        "modelId": MODEL_ID,
        "prompt": PROOF_PROMPT,
        "width": PROOF_DIMENSIONS[0],
        "height": PROOF_DIMENSIONS[1],
        "count": 1,
        "seed": seed,
    }


def decoded_rgb_sha256(path: Path) -> str:
    """Hash exact decoded RGB pixels after checking the proof dimensions."""
    with Image.open(path) as image:
        image.load()
        if image.format != "PNG" or image.size != PROOF_DIMENSIONS:
            raise RuntimeError("worker output is not the exact proof PNG")
        return hashlib.sha256(image.convert("RGB").tobytes()).hexdigest()


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


def prove_network_denial(image: str) -> None:
    """Require Docker's network namespace to reject an external TCP connection."""
    code = (
        "import socket; "
        f"socket.create_connection(({NETWORK_PROBE_ADDRESS[0]!r}, {NETWORK_PROBE_ADDRESS[1]}), 1)"
    )
    result = subprocess.run(
        ["docker", "run", "--rm", "--network", "none", "--entrypoint", "python", image, "-c", code],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode == 0:
        raise RuntimeError("network-disabled container reached an external address")


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


def write_trace_context(image: str, trace_output: Path, identity: WorkerIdentity) -> None:
    """Bind exact proof identities and trace digests after clean worker shutdown."""
    python_trace = trace_output / "python-paths.jsonl"
    process_maps = trace_output / "process-maps.txt"
    context = {
        "imageId": image,
        "workerVersion": identity.worker_version,
        "runtimeId": identity.runtime_id,
        "modelId": MODEL_ID,
        "modelRevision": MODEL_REVISION,
        "pythonTraceSha256": hashlib.sha256(python_trace.read_bytes()).hexdigest(),
        "processMapsSha256": hashlib.sha256(process_maps.read_bytes()).hexdigest(),
    }
    destination = trace_output / "context.json"
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(context, stream, indent=2, sort_keys=True)
        stream.write("\n")


def run_proof(
    image: str,
    model: Path,
    output: Path,
    trace_source: Path | None = None,
    trace_output: Path | None = None,
    ablate_ucc: bool = False,
    profile: RuntimeProfile = DEFAULT_RUNTIME_PROFILE,
) -> dict:
    """Exercise load, cold/warm determinism, and active-step cancellation."""
    if (trace_source is None) != (trace_output is None):
        raise ValueError("runtime trace source and output must be supplied together")
    if profile.identity == PYTORCH_WORKER_IDENTITY and not ablate_ucc:
        raise ValueError("the PyTorch wheel profile requires UCC ablation")
    prove_network_denial(image)
    name = f"bottie-image-proof-{uuid.uuid4()}"
    ucc_mask = tempfile.TemporaryDirectory(prefix="bottie-ucc-mask-") if ablate_ucc else None
    command = [
        "docker", "run", "-i", "--name", name, "--network", "none", "--read-only", "--cap-drop", "ALL",
        "--security-opt", "no-new-privileges", "--gpus", "all", "--shm-size", "1g",
        "--user", f"{os.getuid()}:{os.getgid()}", "--tmpfs", "/tmp:rw,noexec,nosuid,size=1g",
        "-e", "HOME=/tmp", "-e", "CUDA_CACHE_PATH=/tmp/cuda-cache",
        "-e", "PYTHONDONTWRITEBYTECODE=1",
        "-v", f"{model}:/model:ro", "-v", f"{output}:/output:rw",
    ]
    if ucc_mask is not None:
        command.extend(["-v", f"{ucc_mask.name}:{UCC_CONTAINER_PATH}:ro"])
    if trace_source is not None and trace_output is not None:
        command.extend(
            [
                "-e",
                "BOTTIE_RUNTIME_TRACE_FILE=/runtime-trace/python-paths.jsonl",
                "-v",
                f"{trace_source}:/trace-source:ro",
                "-v",
                f"{trace_output}:/runtime-trace:rw",
                "--entrypoint",
                "python",
            ]
        )
    command.append(image)
    if trace_source is not None:
        command.extend(["-c", TRACE_WRAPPER, profile.worker_script, "/output"])
    else:
        command.append("/output")
    host_available_start = host_available_bytes()
    stderr_file = tempfile.TemporaryFile()
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=stderr_file,
            bufsize=0,
        )
    except BaseException:
        stderr_file.close()
        if ucc_mask is not None:
            ucc_mask.cleanup()
        raise
    sampler = None
    try:
        write_frame(process.stdin, {"type": "hello", "protocolVersion": 1, "clientVersion": "bottie-proof"})
        hello = read_frame_with_timeout(process.stdout, HANDSHAKE_TIMEOUT_SECONDS)
        capabilities = read_frame_with_timeout(process.stdout, HANDSHAKE_TIMEOUT_SECONDS)
        if hello.get("workerVersion") != profile.identity.worker_version:
            raise RuntimeError("worker version mismatch")
        if capabilities.get("capabilities", {}).get("runtimeId") != profile.identity.runtime_id:
            raise RuntimeError("worker runtime mismatch")
        if ablate_ucc:
            assert_ucc_installation_masked(name)
            capture_process_maps(name, reject_ucc=True)
        sampler = MemorySampler(container_process_id(name))
        sampler.start()

        load_started = time.monotonic()
        write_frame(process.stdin, {
            "type": "load", "protocolVersion": 1, "requestId": "proof-load",
            "model": {"modelId": MODEL_ID, "modelRevision": MODEL_REVISION, "modelDirectory": "/model"},
        })
        load_result, _ = wait_for_result(process, "proof-load")
        load_seconds = time.monotonic() - load_started
        if load_result["result"]["status"] != "completed":
            raise RuntimeError("worker model load failed")

        durations, hashes = [], []
        for label in ("cold", "warm"):
            started = time.monotonic()
            write_frame(process.stdin, generation_command(f"proof-{label}", COMPLETE_SEED))
            result, _ = wait_for_result(process, f"proof-{label}")
            durations.append(time.monotonic() - started)
            if result["result"]["status"] != "completed":
                raise RuntimeError(f"{label} generation failed")
            generated = output / "generated.png"
            hashes.append(decoded_rgb_sha256(generated))
            shutil.move(generated, output / f"{label}.png")
        if hashes[0] != hashes[1]:
            raise RuntimeError("same-seed cold and warm pixels differ")
        if trace_output is not None or ablate_ucc:
            destination = trace_output / "process-maps.txt" if trace_output is not None else None
            capture_process_maps(name, destination, reject_ucc=ablate_ucc)

        write_frame(process.stdin, generation_command("proof-cancel", CANCEL_SEED))
        cancelled, cancellation_started = wait_for_result(process, "proof-cancel", cancel_after_step=2)
        if cancelled["result"]["status"] != "cancelled" or cancellation_started is None:
            raise RuntimeError("active generation did not cancel cooperatively")
        if (output / "generated.png").exists():
            raise RuntimeError("cancelled generation retained an output")
        cancellation_ms = max(1, round((time.monotonic() - cancellation_started) * 1000))
        sampler.finish()
        write_frame(process.stdin, {"type": "shutdown", "protocolVersion": 1})
        if process.wait(timeout=30) != 0:
            raise RuntimeError("worker did not shut down cleanly")
        stderr_file.seek(0)
        stderr = stderr_file.read(FRAME_LIMIT + 1)
        if len(stderr) > FRAME_LIMIT:
            raise RuntimeError("worker stderr exceeded the proof bound")
        if trace_output is not None:
            write_trace_context(image, trace_output, profile.identity)
        return {
            "imageId": image,
            "modelId": MODEL_ID,
            "modelRevision": MODEL_REVISION,
            "runtimeId": profile.identity.runtime_id,
            "loadSeconds": round(load_seconds, 3),
            "coldGenerationSeconds": round(durations[0], 3),
            "warmGenerationSeconds": round(durations[1], 3),
            "generatedRgbSha256": hashes[0],
            "deterministicSameSeed": True,
            "cancellationLatencyMs": cancellation_ms,
            "processRssPeakBytes": sampler.process_rss_peak,
            "nvidiaProcessUnifiedMemoryPeakBytes": sampler.nvidia_process_peak,
            "hostAvailableStartBytes": host_available_start,
            "hostAvailableMinimumBytes": sampler.host_available_min,
            "networkNamespaceDeniedExternalConnection": True,
            **(
                {"uccInstallationMasked": True, "uccMappedRuntimeFileCount": 0}
                if ablate_ucc
                else {}
            ),
            "visualReviewed": False,
        }
    except BaseException as error:
        stderr_file.flush()
        stderr_file.seek(0)
        stderr = stderr_file.read(FRAME_LIMIT + 1)
        if len(stderr) > FRAME_LIMIT:
            raise RuntimeError(f"{error}; worker stderr exceeded the proof bound") from error
        diagnostics = [
            line.decode(errors="replace").strip()
            for line in stderr.splitlines()
            if line.startswith(b"bottie-worker-")
        ]
        if diagnostics:
            raise RuntimeError(f"{error}; {diagnostics[-1]}") from error
        raise
    finally:
        if sampler is not None and sampler._thread.is_alive():
            sampler.finish()
        if process.poll() is None:
            process.kill()
            process.wait()
        subprocess.run(["docker", "rm", "-f", name], check=False, stdout=subprocess.DEVNULL)
        stderr_file.close()
        if ucc_mask is not None:
            ucc_mask.cleanup()


def main() -> None:
    """Validate arguments, run the proof, and atomically write measurements."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--measurements", type=Path, required=True)
    parser.add_argument("--runtime-trace-source", type=Path)
    parser.add_argument("--runtime-trace-output", type=Path)
    parser.add_argument("--runtime-profile", choices=RUNTIME_PROFILES, default="ngc-25.11")
    parser.add_argument("--ablate-ucc", action="store_true")
    arguments = parser.parse_args()
    optional_paths = [
        path
        for path in (arguments.runtime_trace_source, arguments.runtime_trace_output)
        if path is not None
    ]
    for path in (arguments.model, arguments.output, arguments.measurements.parent, *optional_paths):
        if not path.is_absolute() or not path.exists():
            raise SystemExit("proof paths must be existing absolute paths")
    if any(arguments.output.iterdir()):
        raise SystemExit("proof output directory must be empty")
    if (arguments.runtime_trace_source is None) != (arguments.runtime_trace_output is None):
        raise SystemExit("both runtime trace paths are required together")
    if arguments.runtime_trace_output is not None and any(arguments.runtime_trace_output.iterdir()):
        raise SystemExit("runtime trace output directory must be empty")
    measurements = run_proof(
        arguments.image,
        arguments.model,
        arguments.output,
        trace_source=arguments.runtime_trace_source,
        trace_output=arguments.runtime_trace_output,
        ablate_ucc=arguments.ablate_ucc,
        profile=RUNTIME_PROFILES[arguments.runtime_profile],
    )
    temporary = arguments.measurements.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(measurements, indent=2) + "\n")
    os.replace(temporary, arguments.measurements)
    print("Diffusers worker measurements complete; visual review remains required.")


if __name__ == "__main__":
    main()
