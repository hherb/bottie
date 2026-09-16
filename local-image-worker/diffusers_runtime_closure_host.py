#!/usr/bin/env python3
"""Run the Diffusers runtime-closure collector through a host-verified image ID."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path

from diffusers_bundle_candidate import BASE_IMAGE, BASE_IMAGE_DIGEST
from diffusers_bundle_environment import _inspect_derived_image
from diffusers_runtime_closure import ClosureEvidenceError, collect_runtime_closure


CONTAINER_SOURCE_ROOT = "/opt/bottie-runtime-closure"
CONTAINER_TRACE_ROOT = "/opt/bottie-runtime-trace"


def collect_verified_runtime_closure(image_reference: str, trace_root: Path) -> dict:
    """Run the unbound classifier in the exact image selected by the host Docker daemon."""
    image_id = _inspect_derived_image(image_reference)
    source_root = Path(__file__).resolve().parent
    trace_root = trace_root.resolve(strict=True)
    command = [
        "docker",
        "run",
        "--rm",
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--gpus",
        "all",
        "--user",
        f"{os.getuid()}:{os.getgid()}",
        "-e",
        "PYTHONDONTWRITEBYTECODE=1",
        "--entrypoint",
        "python",
        "--mount",
        f"type=bind,src={source_root},dst={CONTAINER_SOURCE_ROOT},readonly",
        "--mount",
        f"type=bind,src={trace_root},dst={CONTAINER_TRACE_ROOT},readonly",
        "-w",
        CONTAINER_SOURCE_ROOT,
        image_id,
        f"{CONTAINER_SOURCE_ROOT}/diffusers_runtime_closure_host.py",
        "--collect-unbound",
        CONTAINER_TRACE_ROOT,
    ]
    try:
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
    except OSError as error:
        raise ClosureEvidenceError("verified-image closure collection is unavailable") from error
    if completed.returncode != 0:
        detail = completed.stderr.strip().splitlines()[-1] if completed.stderr.strip() else "unknown error"
        raise ClosureEvidenceError(f"verified-image closure collection failed: {detail[:512]}")
    try:
        review = json.loads(completed.stdout)
    except (json.JSONDecodeError, TypeError) as error:
        raise ClosureEvidenceError("verified-image closure collection is malformed") from error
    if not isinstance(review, dict) or "derivedImageDigest" in review:
        raise ClosureEvidenceError("unbound runtime closure is malformed")
    return {
        **review,
        "baseImage": BASE_IMAGE,
        "baseImageDigest": BASE_IMAGE_DIGEST,
        "derivedImageDigest": image_id,
    }


def _write_review(review: dict, output: Path) -> None:
    """Atomically write one deterministic path-free runtime-closure record."""
    parent = output.parent.resolve(strict=True)
    destination = parent / output.name
    temporary = parent / f".{output.name}.tmp"
    temporary.write_text(f"{json.dumps(review, indent=2, sort_keys=True)}\n", encoding="utf-8")
    os.replace(temporary, destination)


def parse_arguments() -> argparse.Namespace:
    """Parse host orchestration or private in-container collection arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image_reference", nargs="?")
    parser.add_argument("trace_root", nargs="?", type=Path)
    parser.add_argument("output", nargs="?", type=Path)
    parser.add_argument("--collect-unbound", type=Path, help=argparse.SUPPRESS)
    return parser.parse_args()


def main() -> None:
    """Collect and persist one exact runtime-closure review."""
    arguments = parse_arguments()
    if arguments.collect_unbound is not None:
        if any(
            value is not None
            for value in (arguments.image_reference, arguments.trace_root, arguments.output)
        ):
            raise ClosureEvidenceError("unbound collection accepts only a trace root")
        print(json.dumps(collect_runtime_closure(arguments.collect_unbound), sort_keys=True))
        return
    if arguments.image_reference is None or arguments.trace_root is None or arguments.output is None:
        raise ClosureEvidenceError("image reference, trace root, and output are required")
    review = collect_verified_runtime_closure(arguments.image_reference, arguments.trace_root)
    _write_review(review, arguments.output)
    if not review["assemblyEligible"]:
        raise SystemExit(3)


if __name__ == "__main__":
    main()
