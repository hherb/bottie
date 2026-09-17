#!/usr/bin/env python3
"""Run two exact offline clean-runtime builds and retain path-free comparison evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

from diffusers_clean_runtime_lock import (
    CleanRuntimeLockError,
    _load_manifest,
    validate_clean_runtime_lock,
)
from diffusers_clean_runtime_rebuild import (
    IMAGE_ID_PATTERN,
    REVIEWED_WORKER_SOURCES,
    CleanRuntimeRebuildError,
    _build_command,
    _collect_regular_files,
    _compare_rebuilds,
    _render_dockerfile,
    validate_rebuild_plan,
)


MAX_PLAN_BYTES = 64 * 1024
CONTAINER_ID_PATTERN = re.compile(r"[0-9a-f]{64}")
INSPECTION_SOURCES = (
    "diffusers_clean_runtime_inventory.py",
    "diffusers_clean_runtime_lock.py",
    "diffusers_clean_runtime_lock_host.py",
)


def _stage_context(
    source_root: Path, context: Path, filenames: tuple[str, ...]
) -> None:
    """Create one build context containing only the fixed recipe and reviewed worker bytes."""
    try:
        context.mkdir(mode=0o700)
        (context / "Dockerfile").write_text(_render_dockerfile(), encoding="utf-8")
        for filename in filenames:
            shutil.copyfile(source_root / filename, context / filename)
    except OSError as error:
        raise CleanRuntimeRebuildError("rebuild context cannot be staged") from error


def _stage_inspection(source_root: Path, inspection: Path) -> None:
    """Stage only the existing read-only inventory collector and its two imports."""
    try:
        inspection.mkdir(mode=0o700)
        for filename in INSPECTION_SOURCES:
            source = source_root / filename
            if source.is_symlink() or not source.is_file():
                raise CleanRuntimeRebuildError("inspection source is invalid")
            destination = inspection / filename
            shutil.copyfile(source, destination)
            destination.chmod(0o444)
        inspection.chmod(0o555)
    except OSError as error:
        raise CleanRuntimeRebuildError("inspection source cannot be staged") from error


def _collect_inventory_command(image_id: str, inspection: Path) -> list[str]:
    """Return the isolated command that measures installed identities by immutable ID."""
    if not IMAGE_ID_PATTERN.fullmatch(image_id) or not inspection.is_absolute():
        raise CleanRuntimeRebuildError("inventory collection identity is invalid")
    return [
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
        "--user",
        "65534:65534",
        "--entrypoint",
        "/opt/bottie/venv/bin/python",
        "--mount",
        f"type=bind,src={inspection},dst=/opt/bottie-inspection,readonly",
        image_id,
        "/opt/bottie-inspection/diffusers_clean_runtime_lock_host.py",
        "--collect-unbound",
    ]


def _inspect_image(image_reference: str) -> str:
    """Resolve one completed build name to an immutable Linux ARM64 image ID."""
    try:
        completed = subprocess.run(
            ["docker", "image", "inspect", image_reference],
            capture_output=True,
            text=True,
            check=False,
        )
        inspected = json.loads(completed.stdout)
    except (OSError, json.JSONDecodeError, TypeError) as error:
        raise CleanRuntimeRebuildError("rebuilt image inspection failed") from error
    if (
        completed.returncode != 0
        or not isinstance(inspected, list)
        or len(inspected) != 1
        or not isinstance(inspected[0], dict)
    ):
        raise CleanRuntimeRebuildError("rebuilt image inspection is malformed")
    image = inspected[0]
    image_id = image.get("Id")
    if image.get("Os") != "linux" or image.get("Architecture") != "arm64":
        raise CleanRuntimeRebuildError("rebuilt image target has drifted")
    if not isinstance(image_id, str) or not IMAGE_ID_PATTERN.fullmatch(image_id):
        raise CleanRuntimeRebuildError("rebuilt image identity is invalid")
    return image_id


def _expected_inventory(lock: dict) -> dict:
    """Project the frozen artifact lock into the installed-identity schema."""
    return {
        "schemaVersion": lock.get("schemaVersion"),
        "target": lock.get("target"),
        "pythonVersion": lock.get("pythonVersion"),
        "pythonDistributions": [
            {"name": record["name"], "version": record["version"]}
            for record in lock["pythonArtifacts"]
        ],
        "debianPackages": [
            {
                "name": record["name"],
                "version": record["version"],
                "architecture": record["architecture"],
            }
            for record in lock["debianArtifacts"]
        ],
    }


def _validate_rebuilt_inventory(inventory: dict, lock: dict) -> None:
    """Require the rebuilt installed identities to equal every frozen input identity."""
    if inventory != _expected_inventory(lock):
        raise CleanRuntimeRebuildError(
            "rebuilt inventory differs from the frozen input lock"
        )


def _collect_inventory(image_id: str, inspection: Path) -> dict:
    """Collect and decode one rebuilt image's installed identities offline."""
    try:
        completed = subprocess.run(
            _collect_inventory_command(image_id, inspection),
            capture_output=True,
            text=True,
            check=False,
        )
        inventory = json.loads(completed.stdout)
    except (OSError, json.JSONDecodeError, TypeError) as error:
        raise CleanRuntimeRebuildError("rebuilt inventory collection failed") from error
    if completed.returncode != 0 or not isinstance(inventory, dict):
        raise CleanRuntimeRebuildError("rebuilt inventory collection is malformed")
    return inventory


def _export_regular_files(image_id: str):
    """Export one stopped container and return its normalized regular-file measurement."""
    container_id = None
    process = None
    try:
        created = subprocess.run(
            ["docker", "container", "create", "--network", "none", image_id],
            capture_output=True,
            text=True,
            check=False,
        )
        container_id = created.stdout.strip()
        if created.returncode != 0 or not CONTAINER_ID_PATTERN.fullmatch(container_id):
            raise CleanRuntimeRebuildError("rebuilt rootfs container cannot be created")
        with tempfile.TemporaryFile() as errors:
            process = subprocess.Popen(
                ["docker", "container", "export", container_id],
                stdout=subprocess.PIPE,
                stderr=errors,
            )
            if process.stdout is None:
                raise CleanRuntimeRebuildError("rebuilt rootfs export is unavailable")
            records = _collect_regular_files(process.stdout)
            process.stdout.close()
            return_code = process.wait()
            process = None
            if return_code != 0:
                raise CleanRuntimeRebuildError("rebuilt rootfs export failed")
        return records
    except OSError as error:
        raise CleanRuntimeRebuildError(
            "rebuilt rootfs export is unavailable"
        ) from error
    finally:
        if process is not None:
            process.terminate()
            process.wait()
        if container_id:
            subprocess.run(
                ["docker", "container", "rm", "--force", container_id],
                capture_output=True,
                check=False,
            )


def _run_build(
    context: Path, inputs: Path, inspection: Path, build_name: str, lock: dict
) -> dict:
    """Build, inspect, inventory, and normalize one independently named image."""
    try:
        completed = subprocess.run(
            _build_command(context, inputs, build_name),
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise CleanRuntimeRebuildError("offline rebuild could not start") from error
    if completed.returncode != 0:
        raise CleanRuntimeRebuildError("offline rebuild failed")
    image_id = _inspect_image(build_name)
    inventory = _collect_inventory(image_id, inspection)
    _validate_rebuilt_inventory(inventory, lock)
    return {
        "buildName": build_name,
        "imageId": image_id,
        "inventory": inventory,
        "regularFiles": _export_regular_files(image_id),
    }


def run_rebuilds(
    plan: dict,
    lock: dict,
    artifact_root: Path,
    source_root: Path,
    first_name: str,
    second_name: str,
) -> dict:
    """Verify inputs, execute two offline builds, and return path-free agreement evidence."""
    try:
        verification = validate_clean_runtime_lock(lock, artifact_root)
    except CleanRuntimeLockError as error:
        raise CleanRuntimeRebuildError(str(error)) from error
    sources = validate_rebuild_plan(plan, source_root, verification["lockSha256"])
    if first_name == second_name:
        raise CleanRuntimeRebuildError("rebuild names are not independent")
    with tempfile.TemporaryDirectory(prefix="bottie-clean-rebuild-") as temporary:
        root = Path(temporary)
        context = root / "context"
        inspection = root / "inspection"
        _stage_context(source_root, context, sources)
        validate_rebuild_plan(plan, context, verification["lockSha256"])
        _stage_inspection(source_root, inspection)
        first = _run_build(context, artifact_root, inspection, first_name, lock)
        if validate_clean_runtime_lock(lock, artifact_root) != verification:
            raise CleanRuntimeRebuildError("rebuild inputs changed between builds")
        second = _run_build(context, artifact_root, inspection, second_name, lock)
        if validate_clean_runtime_lock(lock, artifact_root) != verification:
            raise CleanRuntimeRebuildError("rebuild inputs changed after builds")
    evidence = _compare_rebuilds(first, second, verification["lockSha256"])
    evidence["rebuildPlanSha256"] = hashlib.sha256(
        json.dumps(plan, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    evidence["inputArtifactByteSize"] = verification["totalArtifactByteSize"]
    return evidence


def _load_plan(path: Path) -> dict:
    """Load one bounded absolute rebuild plan."""
    try:
        if (
            not path.is_absolute()
            or path.is_symlink()
            or path.stat().st_size > MAX_PLAN_BYTES
        ):
            raise CleanRuntimeRebuildError("rebuild plan file is invalid")
        plan = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CleanRuntimeRebuildError("rebuild plan file cannot be read") from error
    if not isinstance(plan, dict):
        raise CleanRuntimeRebuildError("rebuild plan schema is not exact")
    return plan


def _write_evidence(evidence: dict, output: Path, artifact_root: Path) -> None:
    """Atomically retain path-free evidence outside the frozen input tree."""
    if not output.is_absolute() or output.is_symlink():
        raise CleanRuntimeRebuildError("rebuild evidence output is invalid")
    try:
        parent = output.parent.resolve(strict=True)
        inputs = artifact_root.resolve(strict=True)
    except OSError as error:
        raise CleanRuntimeRebuildError(
            "rebuild evidence destination is unavailable"
        ) from error
    destination = parent / output.name
    if destination == inputs or inputs in destination.parents:
        raise CleanRuntimeRebuildError(
            "rebuild evidence must be outside the input tree"
        )
    descriptor = None
    temporary_name = None
    try:
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{output.name}.", dir=parent
        )
        with os.fdopen(descriptor, "w", encoding="utf-8") as temporary:
            descriptor = None
            temporary.write(f"{json.dumps(evidence, indent=2, sort_keys=True)}\n")
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, destination)
        temporary_name = None
    except OSError as error:
        raise CleanRuntimeRebuildError("rebuild evidence cannot be written") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def main() -> None:
    """Parse one explicitly authorized host run and write its exact evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", type=Path)
    parser.add_argument("lock", type=Path)
    parser.add_argument("artifact_root", type=Path)
    parser.add_argument("source_root", type=Path)
    parser.add_argument("first_name")
    parser.add_argument("second_name")
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    try:
        evidence = run_rebuilds(
            _load_plan(arguments.plan),
            _load_manifest(arguments.lock),
            arguments.artifact_root,
            arguments.source_root,
            arguments.first_name,
            arguments.second_name,
        )
        _write_evidence(evidence, arguments.output, arguments.artifact_root)
    except (CleanRuntimeLockError, CleanRuntimeRebuildError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
