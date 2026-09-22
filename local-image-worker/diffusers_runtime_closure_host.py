#!/usr/bin/env python3
"""Run the Diffusers runtime-closure collector through a host-verified image ID."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path

from diffusers_bundle_environment import _inspect_derived_image
from diffusers_runtime_closure import (
    ClosureEvidenceError,
    _read_bounded_text,
    collect_runtime_closure,
)
from diffusers_runtime_closure_profiles import (
    NGC_RUNTIME_PROFILE_NAME,
    ClosureProfileError,
    runtime_closure_profile,
)


CONTAINER_SOURCE_ROOT = "/opt/bottie-runtime-closure"
CONTAINER_TRACE_ROOT = "/opt/bottie-runtime-trace"
CONTAINER_LICENSE_REVIEW = "/opt/bottie-license-review.json"
CONTAINER_LICENSE_SOURCES = "/opt/bottie-license-sources"
CONTAINER_CLEAN_RUNTIME_LOCK = "/opt/bottie-clean-runtime-lock.json"
IMAGE_ID_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


def collect_verified_runtime_closure(
    image_reference: str,
    trace_root: Path,
    license_review: Path | None = None,
    license_sources: Path | None = None,
    profile_name: str = NGC_RUNTIME_PROFILE_NAME,
    clean_runtime_lock: Path | None = None,
) -> dict:
    """Run the unbound classifier in the exact image selected by the host Docker daemon."""
    try:
        profile = runtime_closure_profile(profile_name)
    except ClosureProfileError as error:
        raise ClosureEvidenceError(str(error)) from error
    if license_review is not None and not profile.allow_license_review:
        raise ClosureEvidenceError(
            "runtime closure profile does not accept licence review"
        )
    if license_sources is not None and not profile.license_source_components:
        raise ClosureEvidenceError(
            "runtime closure profile does not accept licence sources"
        )
    if profile.requires_clean_runtime_lock and os.getuid() == 0:
        raise ClosureEvidenceError(
            "runtime closure collection requires a non-root host user"
        )
    if profile.require_all_license_sources and license_sources is None:
        raise ClosureEvidenceError("runtime closure profile requires licence sources")
    if profile.requires_clean_runtime_lock:
        if clean_runtime_lock is None:
            raise ClosureEvidenceError("clean-runtime lock is required")
        clean_runtime_lock = clean_runtime_lock.resolve(strict=True)
    elif clean_runtime_lock is not None:
        raise ClosureEvidenceError("clean-runtime lock is not accepted by this profile")
    image_id = (
        _inspect_derived_image(image_reference)
        if profile_name == NGC_RUNTIME_PROFILE_NAME
        else _inspect_exact_image(image_reference, profile.derived_image_digest)
    )
    source_root = Path(__file__).resolve().parent
    trace_root = trace_root.resolve(strict=True)
    review_arguments = []
    if license_review is not None:
        license_review = license_review.resolve(strict=True)
        review_arguments = [
            "--mount",
            f"type=bind,src={license_review},dst={CONTAINER_LICENSE_REVIEW},readonly",
        ]
    source_arguments = []
    if license_sources is not None:
        license_sources = license_sources.resolve(strict=True)
        if not license_sources.is_dir():
            raise ClosureEvidenceError("external licence source root is invalid")
        source_arguments = [
            "--mount",
            f"type=bind,src={license_sources},dst={CONTAINER_LICENSE_SOURCES},readonly",
        ]
    lock_arguments = []
    if clean_runtime_lock is not None:
        lock_arguments = [
            "--mount",
            f"type=bind,src={clean_runtime_lock},dst={CONTAINER_CLEAN_RUNTIME_LOCK},readonly",
        ]
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
        profile.python_executable,
        "--mount",
        f"type=bind,src={source_root},dst={CONTAINER_SOURCE_ROOT},readonly",
        "--mount",
        f"type=bind,src={trace_root},dst={CONTAINER_TRACE_ROOT},readonly",
        *review_arguments,
        *source_arguments,
        *lock_arguments,
        "-w",
        CONTAINER_SOURCE_ROOT,
        image_id,
        f"{CONTAINER_SOURCE_ROOT}/diffusers_runtime_closure_host.py",
        "--collect-unbound",
        CONTAINER_TRACE_ROOT,
    ]
    if license_review is not None:
        command.extend(["--license-review", CONTAINER_LICENSE_REVIEW])
    if license_sources is not None:
        command.extend(["--license-sources", CONTAINER_LICENSE_SOURCES])
    if profile_name != NGC_RUNTIME_PROFILE_NAME:
        command.extend(["--profile", profile_name])
    if clean_runtime_lock is not None:
        command.extend(["--clean-runtime-lock", CONTAINER_CLEAN_RUNTIME_LOCK])
    try:
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
    except OSError as error:
        raise ClosureEvidenceError(
            "verified-image closure collection is unavailable"
        ) from error
    if completed.returncode != 0:
        detail = (
            completed.stderr.strip().splitlines()[-1]
            if completed.stderr.strip()
            else "unknown error"
        )
        raise ClosureEvidenceError(
            f"verified-image closure collection failed: {detail[:512]}"
        )
    try:
        review = json.loads(completed.stdout)
    except (json.JSONDecodeError, TypeError) as error:
        raise ClosureEvidenceError(
            "verified-image closure collection is malformed"
        ) from error
    if not isinstance(review, dict) or "derivedImageDigest" in review:
        raise ClosureEvidenceError("unbound runtime closure is malformed")
    return {
        **review,
        "baseImage": profile.base_image,
        "baseImageDigest": profile.base_image_digest,
        "derivedImageDigest": image_id,
    }


def _inspect_exact_image(image_reference: str, expected_image_id: str) -> str:
    """Resolve one Docker reference and require an exact immutable Linux ARM64 image."""
    if (
        not isinstance(image_reference, str)
        or not image_reference
        or image_reference != image_reference.strip()
        or image_reference.startswith("-")
        or len(image_reference.encode()) > 512
        or any(ord(character) < 32 for character in image_reference)
        or not IMAGE_ID_PATTERN.fullmatch(expected_image_id)
    ):
        raise ClosureEvidenceError("runtime closure image reference is invalid")
    try:
        completed = subprocess.run(
            ["docker", "image", "inspect", image_reference],
            capture_output=True,
            text=True,
            check=False,
        )
        inspected = json.loads(completed.stdout)
    except (OSError, json.JSONDecodeError, TypeError) as error:
        raise ClosureEvidenceError("runtime closure image inspection failed") from error
    if (
        completed.returncode != 0
        or not isinstance(inspected, list)
        or len(inspected) != 1
        or not isinstance(inspected[0], dict)
    ):
        raise ClosureEvidenceError("runtime closure image inspection is malformed")
    image = inspected[0]
    if (
        image.get("Id") != expected_image_id
        or image.get("Os") != "linux"
        or image.get("Architecture") != "arm64"
    ):
        raise ClosureEvidenceError("runtime closure image identity has drifted")
    return expected_image_id


def _write_review(review: dict, output: Path) -> None:
    """Atomically write one deterministic path-free runtime-closure record."""
    parent = output.parent.resolve(strict=True)
    destination = parent / output.name
    temporary = parent / f".{output.name}.tmp"
    temporary.write_text(
        f"{json.dumps(review, indent=2, sort_keys=True)}\n", encoding="utf-8"
    )
    os.replace(temporary, destination)


def parse_arguments() -> argparse.Namespace:
    """Parse host orchestration or private in-container collection arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image_reference", nargs="?")
    parser.add_argument("trace_root", nargs="?", type=Path)
    parser.add_argument("output", nargs="?", type=Path)
    parser.add_argument("--collect-unbound", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--license-review", type=Path)
    parser.add_argument("--license-sources", type=Path)
    parser.add_argument("--profile", default=NGC_RUNTIME_PROFILE_NAME)
    parser.add_argument("--clean-runtime-lock", type=Path)
    return parser.parse_args()


def main() -> None:
    """Collect and persist one exact runtime-closure review."""
    arguments = parse_arguments()
    if arguments.collect_unbound is not None:
        if any(
            value is not None
            for value in (
                arguments.image_reference,
                arguments.trace_root,
                arguments.output,
            )
        ):
            raise ClosureEvidenceError("unbound collection accepts only a trace root")
        license_review = None
        if arguments.license_review is not None:
            try:
                license_review = json.loads(
                    _read_bounded_text(arguments.license_review)
                )
            except json.JSONDecodeError as error:
                raise ClosureEvidenceError(
                    "licence review manifest is malformed"
                ) from error
        print(
            json.dumps(
                collect_runtime_closure(
                    arguments.collect_unbound,
                    license_review,
                    arguments.license_sources,
                    arguments.profile,
                    arguments.clean_runtime_lock,
                ),
                sort_keys=True,
            )
        )
        return
    if (
        arguments.image_reference is None
        or arguments.trace_root is None
        or arguments.output is None
    ):
        raise ClosureEvidenceError(
            "image reference, trace root, and output are required"
        )
    review = collect_verified_runtime_closure(
        arguments.image_reference,
        arguments.trace_root,
        arguments.license_review,
        arguments.license_sources,
        arguments.profile,
        arguments.clean_runtime_lock,
    )
    _write_review(review, arguments.output)
    if not review["assemblyEligible"]:
        raise SystemExit(3)


if __name__ == "__main__":
    main()
