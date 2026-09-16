#!/usr/bin/env python3
"""Private framed Diffusers worker for Bottie's Linux NVIDIA Qwen Image proof."""

from __future__ import annotations

import os
import sys
import time
from pathlib import Path
from typing import Callable

from mlx_worker import Worker, WorkerIdentity, install_network_denial

WORKER_VERSION = "diffusers-0.40.0-ngc-25.11-proof-1"
RUNTIME_ID = "diffusers@0.40.0+ngc-25.11-arm64"
MODEL_ID = "Qwen/Qwen-Image-2512"
MODEL_REVISION = "25468b98e3276ca6700de15c6628e51b7de54a26"
INFERENCE_STEPS = 15
CANCELLATION_POLL_WINDOW_SECONDS = 0.1
DIFFUSERS_WORKER_IDENTITY = WorkerIdentity(WORKER_VERSION, RUNTIME_ID, MODEL_ID, MODEL_REVISION)


class DiffusersBackend:
    """Pinned CUDA adapter that loads only an already verified local model tree."""

    def __init__(self) -> None:
        """Create an unloaded adapter without importing the heavyweight runtime."""
        self._pipeline = None

    def load(self, model_directory: Path) -> None:
        """Load the exact local Qwen Image package onto the single CUDA device."""
        import torch
        from diffusers import QwenImagePipeline

        self._pipeline = QwenImagePipeline.from_pretrained(
            str(model_directory),
            dtype=torch.bfloat16,
            local_files_only=True,
            device_map="cuda",
        )

    def generate(
        self,
        prompt: str,
        width: int,
        height: int,
        seed: int,
        output_path: Path,
        progress: Callable[[int, int], None],
    ) -> None:
        """Run the fixed seeded 15-step CUDA profile and write one decoded PNG."""
        if self._pipeline is None:
            raise RuntimeError("model is not loaded")
        import torch

        def report_step(_pipeline, step: int, _timestep, callback_kwargs: dict) -> dict:
            """Expose one denoising boundary and yield to exact-request cancellation."""
            progress(step + 1, INFERENCE_STEPS)
            time.sleep(CANCELLATION_POLL_WINDOW_SECONDS)
            return callback_kwargs

        generator = torch.Generator(device="cuda").manual_seed(seed)
        image = self._pipeline(
            prompt=prompt,
            width=width,
            height=height,
            num_inference_steps=INFERENCE_STEPS,
            true_cfg_scale=4.0,
            generator=generator,
            callback_on_step_end=report_step,
            callback_on_step_end_tensor_inputs=[],
        ).images[0]
        image.save(output_path, format="PNG")


def main() -> None:
    """Start one offline Diffusers worker with private standard streams."""
    if len(sys.argv) != 2:
        raise SystemExit(2)
    install_network_denial()
    Worker(
        sys.stdin.buffer,
        sys.stdout.buffer,
        Path(sys.argv[1]),
        DiffusersBackend,
        DIFFUSERS_WORKER_IDENTITY,
    ).run()
    os._exit(0)


if __name__ == "__main__":
    main()
