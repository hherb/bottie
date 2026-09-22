"""Tests for exact source evidence on the rebuilt clean-runtime profile."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import diffusers_clean_runtime_closure_host as clean_closure_host
import diffusers_runtime_closure_host as closure_host
from diffusers_runtime_closure_profiles import CLEAN_RUNTIME_PROFILE_NAME


def _closure(trace: dict) -> dict:
    """Return one agreeing path-free result for source-root orchestration tests."""
    return {
        "schemaVersion": 1,
        "trace": trace,
        "closure": {},
        "blockers": ["python:example@1:unreviewed-license-expression"],
        "closureComplete": True,
        "licenseReviewed": False,
        "assemblyEligible": False,
        "distributionReviewed": False,
    }


class DiffusersCleanRuntimeLicenseSourceTests(unittest.TestCase):
    """Protect clean-only source policy and two-trace source agreement."""

    def test_clean_profile_requires_a_source_root(self) -> None:
        """A generic clean-profile caller cannot omit the complete source set."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = root / "trace"
            lock = root / "lock.json"
            trace.mkdir()
            lock.write_text("{}", encoding="utf-8")

            with self.assertRaisesRegex(
                closure_host.ClosureEvidenceError, "requires licence sources"
            ):
                closure_host.collect_verified_runtime_closure(
                    clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
                    trace,
                    profile_name=CLEAN_RUNTIME_PROFILE_NAME,
                    clean_runtime_lock=lock,
                )

    def test_clean_profile_rejects_review_but_mounts_exact_sources(self) -> None:
        """The clean route admits source archives without admitting a review manifest."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = root / "trace"
            sources = root / "sources"
            lock = root / "lock.json"
            review = root / "review.json"
            trace.mkdir()
            sources.mkdir()
            lock.write_text("{}", encoding="utf-8")
            review.write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(
                closure_host.ClosureEvidenceError, "licence review"
            ):
                closure_host.collect_verified_runtime_closure(
                    clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
                    trace,
                    license_review=review,
                    profile_name=CLEAN_RUNTIME_PROFILE_NAME,
                    clean_runtime_lock=lock,
                )

            completed = SimpleNamespace(
                returncode=0, stdout='{"closure": {}}', stderr=""
            )
            with (
                patch.object(
                    closure_host,
                    "_inspect_exact_image",
                    return_value=clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
                ),
                patch.object(
                    closure_host.subprocess, "run", return_value=completed
                ) as run,
            ):
                closure_host.collect_verified_runtime_closure(
                    clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
                    trace,
                    license_sources=sources,
                    profile_name=CLEAN_RUNTIME_PROFILE_NAME,
                    clean_runtime_lock=lock,
                )

        command = run.call_args.args[0]
        source_mount = next(
            value for value in command if "bottie-license-sources" in value
        )
        self.assertIn(str(sources.resolve()), source_mount)
        self.assertIn("readonly", source_mount)

    def test_two_trace_collection_applies_the_same_exact_source_root(self) -> None:
        """Both retained traces independently bind the same authoritative source bytes."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = root / "first"
            second = root / "second"
            sources = root / "sources"
            lock = root / "lock.json"
            for path in (first, second, sources):
                path.mkdir()
            lock.write_text("{}", encoding="utf-8")
            closures = [
                _closure(
                    {
                        "traceContextSha256": "1" * 64,
                        "pythonTraceSha256": "2" * 64,
                        "processMapsSha256": "3" * 64,
                    }
                ),
                _closure(
                    {
                        "traceContextSha256": "4" * 64,
                        "pythonTraceSha256": "5" * 64,
                        "processMapsSha256": "6" * 64,
                    }
                ),
            ]
            with patch.object(
                clean_closure_host,
                "collect_verified_runtime_closure",
                side_effect=closures,
            ) as collect:
                clean_closure_host.collect_agreed_clean_runtime_closure(
                    clean_closure_host.CLEAN_REBUILT_IMAGE_DIGEST,
                    first,
                    second,
                    lock,
                    sources,
                )

        self.assertEqual(collect.call_count, 2)
        for call in collect.call_args_list:
            self.assertEqual(call.kwargs["license_sources"], sources.resolve())


if __name__ == "__main__":
    unittest.main()
