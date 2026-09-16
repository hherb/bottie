"""Record bounded file events when explicitly injected into the Diffusers proof worker."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path


TRACE_ENVIRONMENT_VARIABLE = "BOTTIE_RUNTIME_TRACE_FILE"
MAX_RECORDED_PATH_BYTES = 4_096


def _record(descriptor: int, kind: str, value: object, trace_path: str) -> None:
    """Append one bounded absolute path event without recursively using Python file I/O."""
    if not isinstance(value, (str, bytes, os.PathLike)):
        return
    path = os.fsdecode(value)
    if path == trace_path or not path.startswith("/") or len(path.encode()) > MAX_RECORDED_PATH_BYTES:
        return
    try:
        payload = json.dumps({"kind": kind, "path": path}, separators=(",", ":")).encode() + b"\n"
        os.write(descriptor, payload)
    except OSError:
        os._exit(70)


def install_runtime_trace() -> None:
    """Install one explicit append-only audit trace and snapshot already loaded module files."""
    trace_path = os.environ.get(TRACE_ENVIRONMENT_VARIABLE)
    if not trace_path or not trace_path.startswith("/"):
        raise RuntimeError("runtime trace destination is invalid")
    descriptor = os.open(trace_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)

    def audit(event: str, arguments: tuple) -> None:
        """Retain only file-open, import-file, and dynamic-library path events."""
        if event == "open" and arguments:
            _record(descriptor, "open", arguments[0], trace_path)
        elif event == "ctypes.dlopen" and arguments:
            _record(descriptor, "dlopen", arguments[0], trace_path)
        elif event == "import" and len(arguments) > 1 and arguments[1]:
            _record(descriptor, "module", arguments[1], trace_path)

    sys.addaudithook(audit)
    _record(descriptor, "module", sys.executable, trace_path)
    for module in tuple(sys.modules.values()):
        _record(descriptor, "module", getattr(module, "__file__", None), trace_path)


install_runtime_trace()
