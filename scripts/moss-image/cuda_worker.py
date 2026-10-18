"""One offline CUDA MOSS request on private stdio; Rust verifies all pipeline and weight bytes first."""

import argparse
import base64
import contextlib
import io
import json
import os
import sys
import threading
import time
import unicodedata

MAX_PROMPT_BYTES = 32 * 1024
MAX_REQUEST_BYTES = MAX_PROMPT_BYTES * 2 + 4096  # JSON escaping plus fixed metadata allowance.
MAX_TOKENS = 4096
MAX_PNG_BYTES = 25 * 1024 * 1024
STEPS = 50


def emit(event, stream=None):
    """Reserve stdout solely for bounded protocol frames, never dependency diagnostics."""
    target = sys.stdout if stream is None else stream
    target.write(json.dumps(event, separators=(",", ":")) + "\n")
    target.flush()


def request(stream):
    """Reject unsupported request shapes before loading multi-gigabyte GPU components."""
    raw = stream.readline(MAX_REQUEST_BYTES + 1)
    if len(raw) > MAX_REQUEST_BYTES or not raw.endswith(b"\n"):
        raise ValueError("invalid request framing")
    data = json.loads(raw)
    if set(data) != {"prompt", "width", "height", "seed", "steps"}:
        raise ValueError("invalid request fields")
    prompt = data["prompt"]
    if not isinstance(prompt, str) or not prompt.strip() or len(prompt.encode()) > MAX_PROMPT_BYTES:
        raise ValueError("invalid prompt")
    if any(unicodedata.category(c) == "Cc" and c not in "\n\t" for c in prompt):
        raise ValueError("unsupported control")
    width, height = data["width"], data["height"]
    if any(type(x) is not int or x < 256 or x % 16 for x in [width, height]) or width * height > 1456**2:
        raise ValueError("invalid dimensions")
    if type(data["seed"]) is not int or not 0 <= data["seed"] < 2**63 or data["steps"] != STEPS:
        raise ValueError("invalid seed or steps")
    return data


def watch_parent(parent):
    """Stop owned GPU work after parent death; Windows requires a handle rather than cached getppid()."""
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        synchronize = 0x00100000
        wait_timeout = 0x102
        poll_milliseconds = 1000
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        kernel.WaitForSingleObject.restype = wintypes.DWORD
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        kernel.CloseHandle.restype = wintypes.BOOL
        handle = kernel.OpenProcess(synchronize, False, parent)
        if not handle:
            os._exit(1)
        try:
            while kernel.WaitForSingleObject(handle, poll_milliseconds) == wait_timeout:
                pass
        finally:
            kernel.CloseHandle(handle)
    else:
        poll_seconds = 1
        while os.getppid() == parent:
            time.sleep(poll_seconds)
    os._exit(1)


def main():
    """Load only the SHA-verified local snapshot and require actual CUDA; never fall back to CPU/MPS."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-directory", required=True)
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    protocol_output = sys.stdout
    threading.Thread(target=watch_parent, args=(os.getppid(),), daemon=True).start()
    os.environ.update(HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", HF_HUB_DISABLE_TELEMETRY="1")

    def offline(event, _args):
        """Block Python network operations; this is defense in depth, not an OS network sandbox."""
        if event.startswith("socket."):
            raise PermissionError("MOSS inference is offline")

    sys.addaudithook(offline)
    with contextlib.redirect_stdout(sys.stderr):
        import torch
        from diffusers import DiffusionPipeline

        if not torch.cuda.is_available() or not torch.cuda.is_bf16_supported():
            raise RuntimeError("CUDA with bfloat16 support required")
    emit(dict(event="ready", protocolVersion=1, modelId=args.model_id, revision=args.revision, runtime="cuda"))
    data = request(sys.stdin.buffer)
    with contextlib.redirect_stdout(sys.stderr):
        pipe = DiffusionPipeline.from_pretrained(
            args.model_directory,
            custom_pipeline=os.path.join(args.model_directory, "pipeline_moss_image.py"),
            trust_remote_code=True,
            local_files_only=True,
            torch_dtype={"default": torch.bfloat16, "vae": torch.float32},
        )
        ids = pipe.tokenizer.apply_chat_template(
            [{"role": "user", "content": data["prompt"]}],
            tokenize=True,
            add_generation_prompt=True,
            enable_thinking=True,
        )
        if len(ids) > MAX_TOKENS:
            raise ValueError("layout exceeds encoder token limit")
        pipe.scheduler = type(pipe.scheduler).from_config(pipe.scheduler.config, shift=5)
        if torch.cuda.get_device_properties(0).total_memory >= 40 * 1024**3:
            pipe.to("cuda")
        elif torch.cuda.get_device_properties(0).total_memory >= 24 * 1024**3:
            pipe.enable_model_cpu_offload(device="cuda")
        else:
            pipe.enable_sequential_cpu_offload(device="cuda")
        pipe.set_progress_bar_config(disable=True)
        progress_bar = pipe.progress_bar

        def denoise_progress(items):
            """Report actual completed denoise steps on the protocol stream, not redirected library output."""
            for index, timestep in enumerate(progress_bar(items)):
                yield timestep
                emit(dict(event="progress", step=index + 1), stream=protocol_output)

        pipe.progress_bar = denoise_progress
        image = pipe(
            data["prompt"],
            width=data["width"],
            height=data["height"],
            num_inference_steps=STEPS,
            guidance_scale=3.0,
            uncond_skip_layers=[22],
            generator=torch.Generator("cpu").manual_seed(data["seed"]),
        ).images[0]
        output = io.BytesIO()
        image.save(output, format="PNG")
        png = output.getvalue()
        if image.size != (data["width"], data["height"]) or len(png) > MAX_PNG_BYTES:
            raise ValueError("invalid output")
    emit(
        dict(
            event="complete",
            modelId=args.model_id,
            revision=args.revision,
            width=data["width"],
            height=data["height"],
            seed=data["seed"],
            png=base64.b64encode(png).decode(),
        )
    )


if __name__ == "__main__":
    try:
        main()
    except Exception:
        emit(dict(event="error"))
        sys.exit(1)
