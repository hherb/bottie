#!/usr/bin/env python3
"""Private Diffusers worker identity for the conventional PyTorch CUDA wheel proof."""

from __future__ import annotations

import os
import sys
from pathlib import Path

from diffusers_worker import DiffusersBackend, MODEL_ID, MODEL_REVISION
from mlx_worker import Worker, WorkerIdentity, install_network_denial

WORKER_VERSION = "diffusers-0.40.0-pytorch-2.10.0-cu130-proof-1"
RUNTIME_ID = "diffusers@0.40.0+pytorch-2.10.0-cu130-ubuntu24.04-arm64"
PYTORCH_WORKER_IDENTITY = WorkerIdentity(WORKER_VERSION, RUNTIME_ID, MODEL_ID, MODEL_REVISION)


def main() -> None:
    """Start the conventional-wheel proof with its distinct runtime identity."""
    if len(sys.argv) != 2:
        raise SystemExit(2)
    install_network_denial()
    Worker(
        sys.stdin.buffer,
        sys.stdout.buffer,
        Path(sys.argv[1]),
        DiffusersBackend,
        PYTORCH_WORKER_IDENTITY,
    ).run()
    os._exit(0)


if __name__ == "__main__":
    main()
