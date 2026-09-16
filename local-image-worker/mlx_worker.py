#!/usr/bin/env python3
"""Private framed MLX-Gen worker for Bottie's pinned Qwen Image 2512 package."""

from __future__ import annotations

import _thread
import json
import os
import queue
import struct
import sys
import threading
import time
from pathlib import Path
from typing import BinaryIO, Callable, NamedTuple, Protocol

PROTOCOL_VERSION = 1
WORKER_VERSION = "mlx-gen-0.18.2-proof-1"
RUNTIME_ID = "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c"
MODEL_ID = "Qwen/Qwen-Image-2512"
MODEL_REVISION = "423f1f5bf708c6e11eb78881ef9738422cea0814"
MAX_FRAME_BYTES = 1024 * 1024
MAX_PROMPT_BYTES = 32 * 1024
MAX_PIXELS = 512 * 512
OUTPUT_NAME = "generated.png"
CANCELLATION_POLL_WINDOW_SECONDS = 0.1
NETWORK_AUDIT_EVENTS = frozenset({"socket.bind", "socket.connect", "socket.getaddrinfo"})
OFFLINE_ENVIRONMENT = {
    "HF_HUB_OFFLINE": "1",
    "HF_HUB_DISABLE_TELEMETRY": "1",
    "TRANSFORMERS_OFFLINE": "1",
}


class WorkerIdentity(NamedTuple):
    """Exact backend identity negotiated and validated by one worker process."""

    worker_version: str
    runtime_id: str
    model_id: str
    model_revision: str


MLX_WORKER_IDENTITY = WorkerIdentity(WORKER_VERSION, RUNTIME_ID, MODEL_ID, MODEL_REVISION)


class NetworkDeniedError(PermissionError):
    """Stable internal marker for an attempted audited network operation."""


class Backend(Protocol):
    """Minimal stateful image backend consumed by the private protocol loop."""

    def load(self, model_directory: Path) -> None:
        """Load one previously verified local model directory."""

    def generate(
        self,
        prompt: str,
        width: int,
        height: int,
        seed: int,
        output_path: Path,
        progress: Callable[[int, int], None],
    ) -> None:
        """Generate one PNG while reporting completed denoising steps."""


class CancellationController:
    """Correlate one cancellation request and interrupt Python's active main thread."""

    def __init__(self) -> None:
        """Create an idle cancellation controller."""
        self._lock = threading.Lock()
        self._active_request_id: str | None = None
        self._cancelled = False

    def begin(self, request_id: str) -> None:
        """Register the only operation that may currently be cancelled."""
        with self._lock:
            self._active_request_id = request_id
            self._cancelled = False

    def finish(self) -> bool:
        """Clear the active operation and return whether it was cancelled."""
        with self._lock:
            cancelled = self._cancelled
            self._active_request_id = None
            self._cancelled = False
            return cancelled

    def request(self, request_id: str) -> bool:
        """Interrupt the main thread only for the exact active request."""
        with self._lock:
            if request_id != self._active_request_id or self._cancelled:
                return False
            self._cancelled = True
        _thread.interrupt_main()
        return True


class ProgressCallback:
    """Translate MLX-Gen's Qwen in-loop callback into worker progress."""

    def __init__(self) -> None:
        """Create a callback without an active protocol reporter."""
        self.reporter: Callable[[int, int], None] | None = None

    def call_in_loop(self, t: int, config, **_unused) -> None:
        """Report one step and briefly yield so the reader can deliver boundary cancellation."""
        if self.reporter is not None:
            self.reporter(t + 1, config.num_inference_steps)
            time.sleep(CANCELLATION_POLL_WINDOW_SECONDS)


class MlxBackend:
    """Pinned MLX-Gen adapter that never resolves model artifacts over the network."""

    def __init__(self) -> None:
        """Create an unloaded adapter without importing the heavyweight runtime."""
        self._model = None
        self._progress = ProgressCallback()

    def load(self, model_directory: Path) -> None:
        """Load the exact local mixed-q4 package with the pinned Qwen implementation."""
        from mflux.models.qwen.variants.txt2img.qwen_image import QwenImage

        model = QwenImage(model_path=str(model_directory))
        model.callbacks.register(self._progress)
        self._model = model

    def generate(
        self,
        prompt: str,
        width: int,
        height: int,
        seed: int,
        output_path: Path,
        progress: Callable[[int, int], None],
    ) -> None:
        """Run the fixed 15-step profile and write exactly one decoded PNG."""
        if self._model is None:
            raise RuntimeError("model is not loaded")
        self._progress.reporter = progress
        try:
            image = self._model.generate_image(
                seed=seed,
                prompt=prompt,
                width=width,
                height=height,
                num_inference_steps=15,
            )
            image.save(path=str(output_path), export_json_metadata=False)
        finally:
            self._progress.reporter = None


class Worker:
    """Long-lived, single-operation private worker with a dedicated command reader."""

    def __init__(
        self,
        input_stream: BinaryIO,
        output_stream: BinaryIO,
        output_directory: Path,
        backend_factory: Callable[[], Backend] = MlxBackend,
        identity: WorkerIdentity = MLX_WORKER_IDENTITY,
    ) -> None:
        """Bind private pipes, one trusted output directory, and one backend factory."""
        self._input = input_stream
        self._output = output_stream
        self._output_lock = threading.Lock()
        self._output_directory = validate_output_directory(output_directory)
        self._backend = backend_factory()
        self._identity = identity
        self._model_loaded = False
        self._commands: queue.Queue[dict | None] = queue.Queue()
        self._cancellation = CancellationController()

    def run(self) -> None:
        """Negotiate the protocol and serve commands until clean shutdown or EOF."""
        hello = read_frame(self._input)
        require_hello(hello)
        self._write(
            {
                "type": "hello",
                "protocolVersion": PROTOCOL_VERSION,
                "workerVersion": self._identity.worker_version,
            }
        )
        self._write(
            {
                "type": "capabilities",
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "runtimeId": self._identity.runtime_id,
                    "generation": True,
                    "supportsSeed": True,
                    "maxOutputs": 1,
                    "maxPixels": MAX_PIXELS,
                },
            }
        )
        threading.Thread(target=self._read_commands, name="bottie-worker-commands", daemon=True).start()
        while True:
            command = self._commands.get()
            if command is None:
                return
            command_type = command.get("type")
            if command_type == "load":
                self._load(command)
            elif command_type == "generate":
                self._generate(command)
            elif command_type == "shutdown":
                require_shutdown(command)
                return
            else:
                raise ValueError("unsupported command")

    def _read_commands(self) -> None:
        """Read framed commands while the main thread may be executing MLX kernels."""
        try:
            while True:
                command = read_frame(self._input)
                if command.get("type") == "cancel":
                    require_cancel(command)
                    if not self._cancellation.request(command["requestId"]):
                        self._commands.put(command)
                else:
                    self._commands.put(command)
        except EOFError:
            self._commands.put(None)
        except BaseException:
            self._commands.put({"type": "invalid"})

    def _load(self, command: dict) -> None:
        """Load one exact verified package and retain it for later generation."""
        request_id, model_directory = require_load(command, self._identity)
        self._cancellation.begin(request_id)
        self._progress(request_id, "loading", 0, 1)
        try:
            self._backend.load(model_directory)
            if self._cancellation.finish():
                self._result(request_id, "load", "cancelled")
                return
            self._model_loaded = True
            self._progress(request_id, "loading", 1, 1)
            self._result(request_id, "load", "completed", outputs=[])
        except KeyboardInterrupt:
            self._cancellation.finish()
            self._result(request_id, "load", "cancelled")
        except BaseException as error:
            report_backend_failure("load", error)
            if self._cancellation.finish():
                self._result(request_id, "load", "cancelled")
            else:
                self._result(request_id, "load", "failed", code="model_load_failed")

    def _generate(self, command: dict) -> None:
        """Generate one fixed-profile PNG or return one correlated terminal failure."""
        request_id, prompt, width, height, seed = require_generate(
            command, self._model_loaded, self._identity
        )
        output_path = self._output_directory / OUTPUT_NAME
        output_path.unlink(missing_ok=True)
        self._cancellation.begin(request_id)
        self._progress(request_id, "preparing", 0, 1)
        try:
            self._backend.generate(
                prompt,
                width,
                height,
                seed,
                output_path,
                lambda completed, total: self._progress(request_id, "denoising", completed, total),
            )
            if self._cancellation.finish():
                output_path.unlink(missing_ok=True)
                self._result(request_id, "generate", "cancelled")
                return
            self._progress(request_id, "decoding", 1, 1)
            self._result(
                request_id,
                "generate",
                "completed",
                outputs=[{"outputName": OUTPUT_NAME, "width": width, "height": height, "seed": seed}],
            )
        except KeyboardInterrupt:
            self._cancellation.finish()
            output_path.unlink(missing_ok=True)
            self._result(request_id, "generate", "cancelled")
        except BaseException as error:
            report_backend_failure("generate", error)
            output_path.unlink(missing_ok=True)
            if self._cancellation.finish():
                self._result(request_id, "generate", "cancelled")
            else:
                self._result(request_id, "generate", "failed", code="generation_failed")

    def _progress(self, request_id: str, stage: str, completed: int, total: int) -> None:
        """Write one correlated bounded progress event."""
        self._write(
            {
                "type": "progress",
                "protocolVersion": PROTOCOL_VERSION,
                "requestId": request_id,
                "stage": stage,
                "completedSteps": completed,
                "totalSteps": total,
            }
        )

    def _result(self, request_id: str, operation: str, status: str, **fields) -> None:
        """Write one terminal result without backend exception text."""
        self._write(
            {
                "type": "result",
                "protocolVersion": PROTOCOL_VERSION,
                "requestId": request_id,
                "operation": operation,
                "result": {"status": status, **fields},
            }
        )

    def _write(self, message: dict) -> None:
        """Serialize one complete frame under the single stdout lock."""
        with self._output_lock:
            write_frame(self._output, message)


def read_frame(stream: BinaryIO) -> dict:
    """Read and decode one bounded big-endian length-prefixed JSON object."""
    prefix = read_exact(stream, 4)
    length = struct.unpack(">I", prefix)[0]
    if length == 0 or length > MAX_FRAME_BYTES:
        raise ValueError("invalid frame length")
    payload = json.loads(read_exact(stream, length))
    if not isinstance(payload, dict):
        raise ValueError("frame must contain an object")
    return payload


def read_exact(stream: BinaryIO, length: int) -> bytes:
    """Read an exact byte count or report a clean private-pipe EOF."""
    chunks = bytearray()
    while len(chunks) < length:
        chunk = stream.read(length - len(chunks))
        if not chunk:
            raise EOFError
        chunks.extend(chunk)
    return bytes(chunks)


def write_frame(stream: BinaryIO, message: dict) -> None:
    """Encode and flush one compact bounded JSON frame."""
    payload = json.dumps(message, separators=(",", ":"), ensure_ascii=False).encode()
    if not payload or len(payload) > MAX_FRAME_BYTES:
        raise ValueError("invalid frame length")
    stream.write(struct.pack(">I", len(payload)))
    stream.write(payload)
    stream.flush()


def require_hello(command: dict) -> None:
    """Accept only the exact closed initial hello shape."""
    require_keys(command, {"type", "protocolVersion", "clientVersion"})
    if command["type"] != "hello" or command["protocolVersion"] != PROTOCOL_VERSION:
        raise ValueError("invalid hello")
    require_text(command["clientVersion"], 256)


def require_load(command: dict, identity: WorkerIdentity = MLX_WORKER_IDENTITY) -> tuple[str, Path]:
    """Validate one exact model load command and return its native values."""
    require_keys(command, {"type", "protocolVersion", "requestId", "model"})
    require_protocol(command, "load")
    request_id = require_request_id(command["requestId"])
    model = command["model"]
    if not isinstance(model, dict):
        raise ValueError("invalid model")
    require_keys(model, {"modelId", "modelRevision", "modelDirectory"})
    if model["modelId"] != identity.model_id or model["modelRevision"] != identity.model_revision:
        raise ValueError("unexpected model")
    model_directory = Path(model["modelDirectory"])
    resolved = model_directory.resolve(strict=True)
    if not model_directory.is_absolute() or not resolved.is_dir() or model_directory.is_symlink():
        raise ValueError("invalid model directory")
    return request_id, resolved


def require_generate(
    command: dict,
    model_loaded: bool,
    identity: WorkerIdentity = MLX_WORKER_IDENTITY,
) -> tuple[str, str, int, int, int]:
    """Validate the fixed single-output 512-square generation profile."""
    require_keys(
        command,
        {"type", "protocolVersion", "requestId", "modelId", "prompt", "width", "height", "count", "seed"},
    )
    require_protocol(command, "generate")
    if not model_loaded or command["modelId"] != identity.model_id:
        raise ValueError("model not loaded")
    request_id = require_request_id(command["requestId"])
    prompt = require_text(command["prompt"], MAX_PROMPT_BYTES)
    width, height, count, seed = command["width"], command["height"], command["count"], command["seed"]
    if width != 512 or height != 512 or count != 1 or not isinstance(seed, int) or seed < 0 or seed >= 2**64:
        raise ValueError("invalid generation profile")
    return request_id, prompt, width, height, seed


def require_cancel(command: dict) -> None:
    """Validate one exact correlated cancellation command."""
    require_keys(command, {"type", "protocolVersion", "requestId"})
    require_protocol(command, "cancel")
    require_request_id(command["requestId"])


def require_shutdown(command: dict) -> None:
    """Validate one exact clean shutdown command."""
    require_keys(command, {"type", "protocolVersion"})
    require_protocol(command, "shutdown")


def require_protocol(command: dict, command_type: str) -> None:
    """Require the current protocol version and expected command tag."""
    if command.get("type") != command_type or command.get("protocolVersion") != PROTOCOL_VERSION:
        raise ValueError("invalid protocol command")


def require_request_id(value) -> str:
    """Return one bounded non-control correlation identifier."""
    return require_text(value, 256)


def require_text(value, maximum_bytes: int) -> str:
    """Return bounded non-empty text without control characters."""
    if not isinstance(value, str) or not value or len(value.encode()) > maximum_bytes:
        raise ValueError("invalid text")
    if value != value.strip() or any(ord(character) < 32 for character in value):
        raise ValueError("invalid text")
    return value


def require_keys(value: dict, expected: set[str]) -> None:
    """Reject missing and unknown fields in one protocol object."""
    if set(value) != expected:
        raise ValueError("invalid fields")


def validate_output_directory(path: Path) -> Path:
    """Return one exact absolute existing non-symlink output directory."""
    resolved = path.resolve(strict=True)
    if not path.is_absolute() or not resolved.is_dir() or path.is_symlink():
        raise ValueError("invalid output directory")
    return resolved


def report_backend_failure(operation: str, error: BaseException) -> None:
    """Emit one bounded path-free diagnostic category without exception text."""
    if isinstance(error, NetworkDeniedError):
        category = "network-attempt-denied"
    elif isinstance(error, ModuleNotFoundError):
        category = "dependency-missing"
    elif isinstance(error, ImportError):
        category = "dependency-invalid"
    elif isinstance(error, FileNotFoundError):
        category = "resource-missing"
    elif isinstance(error, OSError):
        category = "runtime-io"
    elif isinstance(error, (KeyError, TypeError, ValueError)):
        category = "runtime-contract"
    else:
        category = "runtime-failed"
    sys.stderr.write(f"bottie-worker-{operation}-{category}\n")
    sys.stderr.flush()


def install_network_denial() -> None:
    """Deny Python networking and force supported libraries into local-only mode."""
    os.environ.clear()
    os.environ.update(OFFLINE_ENVIRONMENT)

    def deny_network(event: str, _arguments) -> None:
        """Reject every audited socket connection, bind, and resolution attempt."""
        if event in NETWORK_AUDIT_EVENTS:
            raise NetworkDeniedError("worker networking is disabled")

    sys.addaudithook(deny_network)


def main() -> None:
    """Start one worker with private standard streams and a trusted output root."""
    if len(sys.argv) != 2:
        raise SystemExit(2)
    install_network_denial()
    Worker(sys.stdin.buffer, sys.stdout.buffer, Path(sys.argv[1])).run()


if __name__ == "__main__":
    main()
