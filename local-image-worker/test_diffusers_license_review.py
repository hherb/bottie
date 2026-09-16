"""Tests for exact Diffusers runtime licence-review evidence."""

from __future__ import annotations

import base64
import hashlib
import unittest

from diffusers_license_review import LicenseReviewError, validate_license_review


LICENSE_BYTES = b"MIT licence\n"
REVIEW_BYTES = b"Reviewed against exact component and source bytes.\n"
SHA256 = hashlib.sha256(LICENSE_BYTES).hexdigest()
OTHER_SHA256 = hashlib.sha256(REVIEW_BYTES).hexdigest()
IMAGE_DIGEST = f"sha256:{'c' * 64}"
TRACE = {
    "pythonTraceSha256": "d" * 64,
    "processMapsSha256": "e" * 64,
}


def manifest() -> dict:
    """Return one complete minimal review manifest fixture."""
    return {
        "schemaVersion": 1,
        "derivedImageDigest": IMAGE_DIGEST,
        "trace": TRACE,
        "components": [
            {
                "identity": "python:example@1.0",
                "reviewedLicenseExpression": "MIT",
                "licenseFiles": [
                    {
                        "relativeName": "example-1.0/LICENSE",
                        "byteSize": len(LICENSE_BYTES),
                        "sha256": SHA256,
                        "contentsBase64": base64.b64encode(LICENSE_BYTES).decode("ascii"),
                    }
                ],
                "reviewEvidence": {
                    "byteSize": len(REVIEW_BYTES),
                    "sha256": OTHER_SHA256,
                    "contentsBase64": base64.b64encode(REVIEW_BYTES).decode("ascii"),
                },
            }
        ],
    }


class DiffusersLicenseReviewTests(unittest.TestCase):
    """Protect exact identity, byte, and independent-review bindings."""

    def test_validates_path_free_review_evidence_for_exact_components(self) -> None:
        """A review retains only measured bytes and normalized component-scoped conclusions."""
        reviewed = validate_license_review(
            manifest(),
            {"python:example@1.0"},
            IMAGE_DIGEST,
            TRACE,
        )

        self.assertEqual(reviewed["python:example@1.0"]["reviewedLicenseExpression"], "MIT")
        self.assertEqual(reviewed["python:example@1.0"]["licenseFiles"][0]["sha256"], SHA256)
        self.assertNotIn("contentsBase64", str(reviewed))
        self.assertNotIn("path", str(reviewed).lower())

    def test_rejects_image_trace_or_component_drift(self) -> None:
        """Review evidence cannot migrate to a rebuilt image, another trace, or an unknown component."""
        cases = []
        wrong_image = manifest()
        wrong_image["derivedImageDigest"] = f"sha256:{'f' * 64}"
        cases.append(wrong_image)
        wrong_trace = manifest()
        wrong_trace["trace"] = {**TRACE, "pythonTraceSha256": "f" * 64}
        cases.append(wrong_trace)
        wrong_component = manifest()
        wrong_component["components"][0]["identity"] = "python:other@1.0"
        cases.append(wrong_component)

        for value in cases:
            with self.subTest(value=value):
                with self.assertRaises(LicenseReviewError):
                    validate_license_review(
                        value,
                        {"python:example@1.0"},
                        IMAGE_DIGEST,
                        TRACE,
                    )

    def test_rejects_partial_duplicate_or_unsorted_component_reviews(self) -> None:
        """A completed review must cover every component exactly once in byte order."""
        second = {
            **manifest()["components"][0],
            "identity": "python:zeta@2.0",
        }
        expected = {"python:example@1.0", "python:zeta@2.0"}

        for components in [
            manifest()["components"],
            [manifest()["components"][0], manifest()["components"][0]],
            [second, manifest()["components"][0]],
        ]:
            value = manifest()
            value["components"] = components
            with self.subTest(components=components):
                with self.assertRaises(LicenseReviewError):
                    validate_license_review(value, expected, IMAGE_DIGEST, TRACE)

    def test_rejects_placeholder_expression_or_unmeasured_evidence(self) -> None:
        """Labels alone cannot replace real licence bytes or an independent review record."""
        cases = []
        placeholder = manifest()
        placeholder["components"][0]["reviewedLicenseExpression"] = "NOASSERTION"
        cases.append(placeholder)
        no_license_bytes = manifest()
        no_license_bytes["components"][0]["licenseFiles"] = []
        cases.append(no_license_bytes)
        empty_review = manifest()
        empty_review["components"][0]["reviewEvidence"]["byteSize"] = 0
        cases.append(empty_review)
        changed_bytes = manifest()
        changed_bytes["components"][0]["licenseFiles"][0]["contentsBase64"] = base64.b64encode(
            b"other bytes"
        ).decode("ascii")
        cases.append(changed_bytes)
        absolute_name = manifest()
        absolute_name["components"][0]["licenseFiles"][0]["relativeName"] = "/tmp/LICENSE"
        cases.append(absolute_name)
        unsorted_files = manifest()
        first_file = unsorted_files["components"][0]["licenseFiles"][0]
        unsorted_files["components"][0]["licenseFiles"] = [
            {**first_file, "relativeName": "z/LICENSE"},
            {**first_file, "relativeName": "a/LICENSE"},
        ]
        cases.append(unsorted_files)

        for value in cases:
            with self.subTest(value=value):
                with self.assertRaises(LicenseReviewError):
                    validate_license_review(
                        value,
                        {"python:example@1.0"},
                        IMAGE_DIGEST,
                        TRACE,
                    )


if __name__ == "__main__":
    unittest.main()
