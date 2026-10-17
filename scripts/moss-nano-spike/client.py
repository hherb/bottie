"""Bounded stdio supervisor for the optional Nano trial worker."""

import json
import os
from pathlib import Path
import selectors
import subprocess
import time

from contract import MAX_EVENT_BYTES, MAX_REQUEST_BYTES

READ_CHUNK_BYTES = 64 * 1024
TERMINATION_GRACE_SECONDS = 2


class WorkerClient:
    """Own exactly one child, its bounded output buffer, and its cancellation lifecycle."""

    def __init__(self, command: list[str], stderr_path: Path):
        """Start a private-pipe child with diagnostics retained only in the trial directory."""
        self.stderr = stderr_path.open("wb")
        try:
            self.process = subprocess.Popen(
                command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr, bufsize=0
            )
        except BaseException:
            self.stderr.close()
            raise
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.buffer = bytearray()
        self.closed = False

    def send(self, request: dict) -> None:
        """Write one bounded request without shell interpolation."""
        payload = (json.dumps(request, ensure_ascii=False) + "\n").encode()
        if len(payload) > MAX_REQUEST_BYTES:
            raise ValueError("Request exceeds protocol limit")
        self.process.stdin.write(payload)
        self.process.stdin.flush()

    def read_event(self, timeout: float) -> dict:
        """Apply an absolute deadline even when a worker writes only a partial line."""
        if timeout <= 0:
            raise TimeoutError("Nano trial worker exceeded its deadline")
        deadline = time.monotonic() + timeout
        while True:
            newline = self.buffer.find(b"\n")
            if newline >= 0:
                if newline > MAX_EVENT_BYTES:
                    raise ValueError("Event exceeds protocol limit")
                line = bytes(self.buffer[:newline])
                del self.buffer[: newline + 1]
                result = json.loads(line)
                if not isinstance(result, dict):
                    raise ValueError("Expected an event object")
                return result
            if len(self.buffer) > MAX_EVENT_BYTES:
                raise ValueError("Event exceeds protocol limit")
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.selector.select(remaining):
                raise TimeoutError("Nano trial worker exceeded its deadline")
            chunk = os.read(self.process.stdout.fileno(), READ_CHUNK_BYTES)
            if not chunk:
                raise RuntimeError("Nano trial worker exited without a terminal event")
            self.buffer.extend(chunk)

    def stop(self) -> float:
        """Terminate, escalate only if needed, and reap this exact worker before returning."""
        started = time.perf_counter()
        if self.closed:
            return 0.0
        try:
            if self.process.poll() is None:
                self.process.terminate()
            try:
                self.process.wait(timeout=TERMINATION_GRACE_SECONDS)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=TERMINATION_GRACE_SECONDS)
        finally:
            self.process.stdin.close()
            self.process.stdout.close()
            self.stderr.close()
            self.selector.close()
            self.closed = True
        return time.perf_counter() - started
