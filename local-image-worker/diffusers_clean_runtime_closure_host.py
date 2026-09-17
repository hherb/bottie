#!/usr/bin/env python3
"""Collect two rebuilt clean-runtime closures and retain one agreeing summary."""

from __future__ import annotations

import argparse
from copy import deepcopy
from pathlib import Path

from diffusers_clean_runtime_closure import CleanRuntimeClosureError
from diffusers_runtime_closure_host import (
    _write_review,
    collect_verified_runtime_closure,
)
from diffusers_runtime_closure_profiles import (
    CLEAN_REBUILT_IMAGE_DIGEST,
    CLEAN_RUNTIME_PROFILE_NAME,
)


TRACE_FIELDS = {
    "traceContextSha256",
    "pythonTraceSha256",
    "processMapsSha256",
}


def agreed_closure_summary(closures: list[dict]) -> dict:
    """Return one path-free result only when two trace-derived closures agree exactly."""
    if len(closures) != 2 or any(not isinstance(closure, dict) for closure in closures):
        raise CleanRuntimeClosureError(
            "exactly two clean-runtime closures are required"
        )
    common_records = []
    traces = []
    for closure in closures:
        common = deepcopy(closure)
        trace = common.pop("trace", None)
        if not isinstance(trace, dict) or set(trace) != TRACE_FIELDS:
            raise CleanRuntimeClosureError("clean-runtime closure trace is malformed")
        traces.append(trace)
        common_records.append(common)
    if traces[0]["traceContextSha256"] == traces[1]["traceContextSha256"]:
        raise CleanRuntimeClosureError(
            "clean-runtime closure traces are not independent"
        )
    if common_records[0] != common_records[1]:
        raise CleanRuntimeClosureError("clean-runtime closures do not agree")
    summary = common_records[0]
    summary["traces"] = sorted(
        traces, key=lambda item: item["traceContextSha256"].encode()
    )
    summary["closuresAgree"] = True
    return summary


def collect_agreed_clean_runtime_closure(
    image_reference: str,
    first_trace_root: Path,
    second_trace_root: Path,
    clean_runtime_lock: Path,
) -> dict:
    """Collect both retained contexts independently through the exact clean profile."""
    first_trace_root = first_trace_root.resolve(strict=True)
    second_trace_root = second_trace_root.resolve(strict=True)
    if first_trace_root == second_trace_root:
        raise CleanRuntimeClosureError(
            "clean-runtime closure traces are not independent"
        )
    closures = [
        collect_verified_runtime_closure(
            image_reference,
            trace_root,
            profile_name=CLEAN_RUNTIME_PROFILE_NAME,
            clean_runtime_lock=clean_runtime_lock,
        )
        for trace_root in (first_trace_root, second_trace_root)
    ]
    return agreed_closure_summary(closures)


def parse_arguments() -> argparse.Namespace:
    """Parse the exact rebuilt-image, two-trace, lock, and output inputs."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image_reference")
    parser.add_argument("first_trace_root", type=Path)
    parser.add_argument("second_trace_root", type=Path)
    parser.add_argument("clean_runtime_lock", type=Path)
    parser.add_argument("output", type=Path)
    return parser.parse_args()


def main() -> None:
    """Collect, compare, and atomically retain one clean-runtime closure summary."""
    arguments = parse_arguments()
    review = collect_agreed_clean_runtime_closure(
        arguments.image_reference,
        arguments.first_trace_root,
        arguments.second_trace_root,
        arguments.clean_runtime_lock,
    )
    _write_review(review, arguments.output)
    if not review["assemblyEligible"]:
        raise SystemExit(3)


if __name__ == "__main__":
    main()
