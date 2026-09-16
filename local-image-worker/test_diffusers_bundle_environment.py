"""Tests for the proof-only Diffusers bundle environment inventory."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from diffusers_bundle_environment import (
    DERIVED_IMAGE_DIGEST,
    EnvironmentEvidenceError,
    _bind_verified_image,
    _collect_from_verified_image,
    _hash_optional_file,
    _inspect_derived_image,
    _looks_like_license_file,
    environment_review,
    normalize_component,
    portable_license_name,
)


SHA256 = "a" * 64


class DiffusersBundleEnvironmentTests(unittest.TestCase):
    """Protect the path-free, fail-closed environment review contract."""

    def test_normalizes_component_without_native_paths(self) -> None:
        """Only bounded identity and licence measurements survive collection."""
        component = normalize_component(
            ecosystem="python",
            name="Example",
            version="1.2.3",
            architecture=None,
            license_expression="MIT",
            license_files=[("dist-info/LICENSE", 12, SHA256)],
        )

        self.assertEqual(
            component,
            {
                "ecosystem": "python",
                "name": "example",
                "version": "1.2.3",
                "licenseExpression": "MIT",
                "licenseFiles": [
                    {"relativeName": "dist-info/LICENSE", "byteSize": 12, "sha256": SHA256}
                ],
            },
        )

    def test_rejects_paths_and_unbounded_or_invalid_component_fields(self) -> None:
        """Environment evidence cannot disclose host paths or accept vague identities."""
        invalid_values = ["", " example", "../example", "/example", "example\nname"]
        for value in invalid_values:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    normalize_component("python", value, "1", None, "MIT", [])

    def test_review_fails_closed_when_licence_bytes_are_missing(self) -> None:
        """A declared licence is insufficient without measured real licence bytes."""
        python_component = normalize_component("python", "tokenizers", "1", None, "Apache-2.0", [])
        native_component = normalize_component(
            "deb", "libc6", "1", "arm64", "Debian-copyright", [("copyright", 12, SHA256)]
        )

        review = environment_review([python_component], [native_component], SHA256, 99)

        self.assertFalse(review["assemblyEligible"])
        self.assertEqual(review["blockers"], ["python:tokenizers@1:missing-license-bytes"])
        self.assertNotIn("path", str(review).lower())

    def test_review_rejects_placeholder_license_declarations(self) -> None:
        """Literal placeholder metadata is normalized to an undeclared blocker."""
        for declaration in ["UNKNOWN", "NOASSERTION"]:
            with self.subTest(declaration=declaration):
                component = normalize_component(
                    "python", "example", "1", None, declaration, [("LICENSE", 12, SHA256)]
                )

                review = environment_review([component], [], SHA256, 99)

                self.assertEqual(component["licenseExpression"], "undeclared")
                self.assertEqual(review["blockers"], ["python:example@1:undeclared-license"])

    @patch("diffusers_bundle_environment.subprocess.run")
    def test_inspects_the_actual_derived_image_identity(self, run) -> None:
        """The host Docker daemon, not caller-controlled environment text, binds the image."""
        run.return_value.returncode = 0
        run.return_value.stdout = json.dumps(
            [{"Id": DERIVED_IMAGE_DIGEST, "Os": "linux", "Architecture": "arm64"}]
        )
        run.return_value.stderr = ""

        self.assertEqual(_inspect_derived_image("bottie-proof:review"), DERIVED_IMAGE_DIGEST)
        run.assert_called_once_with(
            ["docker", "image", "inspect", "bottie-proof:review"],
            capture_output=True,
            text=True,
            check=False,
        )

    @patch("diffusers_bundle_environment.subprocess.run")
    def test_rejects_a_different_inspected_image(self, run) -> None:
        """A tag resolving to rebuilt image bytes cannot be presented as the retained proof image."""
        run.return_value.returncode = 0
        run.return_value.stdout = json.dumps(
            [{"Id": f"sha256:{'b' * 64}", "Os": "linux", "Architecture": "arm64"}]
        )
        run.return_value.stderr = ""

        with self.assertRaisesRegex(EnvironmentEvidenceError, "derived-image identity has drifted"):
            _inspect_derived_image("bottie-proof:review")

    @patch("diffusers_bundle_environment.subprocess.run")
    def test_collects_by_verified_image_id_with_network_disabled(self, run) -> None:
        """Collection uses the inspected immutable ID rather than resolving the caller's tag again."""
        run.return_value.returncode = 0
        run.return_value.stdout = '{"blockers": []}'
        run.return_value.stderr = ""

        self.assertEqual(_collect_from_verified_image(DERIVED_IMAGE_DIGEST), {"blockers": []})

        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn(DERIVED_IMAGE_DIGEST, command)
        self.assertIn("--collect-unbound", command)

    def test_review_is_deterministic_and_requires_container_terms(self) -> None:
        """Component order is canonical and missing NVIDIA terms block assembly."""
        first = normalize_component(
            "python", "zeta", "2", None, "MIT", [("LICENSE", 12, SHA256)]
        )
        second = normalize_component(
            "python", "Alpha", "1", None, "MIT", [("LICENSE", 12, SHA256)]
        )

        review = environment_review([first, second], [], None, None)

        self.assertEqual([item["name"] for item in review["pythonComponents"]], ["alpha", "zeta"])
        self.assertEqual(review["blockers"], ["container-terms:missing-license-bytes"])

    def test_only_host_verified_binding_adds_image_identity(self) -> None:
        """The in-container measurement cannot present itself as image-bound evidence."""
        measurement = environment_review([], [], SHA256, 99)

        self.assertNotIn("baseImageDigest", measurement)
        self.assertNotIn("derivedImageDigest", measurement)

        review = _bind_verified_image(measurement, DERIVED_IMAGE_DIGEST)

        self.assertEqual(review["derivedImageDigest"], DERIVED_IMAGE_DIGEST)

    def test_canonicalizes_installation_relative_license_names(self) -> None:
        """Wheel traversal records become stable labels without losing the file identity."""
        self.assertEqual(
            portable_license_name("../../../share/doc/example/LICENSE.txt", 2),
            "external/0002-LICENSE.txt",
        )
        self.assertEqual(
            portable_license_name("example.dist-info/licenses/LICENSE", 0),
            "example.dist-info/licenses/LICENSE",
        )

    def test_broken_package_license_link_is_missing_evidence(self) -> None:
        """An absent symlink target becomes a blocker rather than a collection crash."""
        with TemporaryDirectory() as directory:
            path = Path(directory) / "copyright"
            path.symlink_to(Path(directory) / "missing")

            self.assertIsNone(_hash_optional_file(path))

    def test_recognizes_documents_without_treating_license_code_as_evidence(self) -> None:
        """Licence bytes must be documents, not helpers or tests whose names mention licences."""
        accepted = [
            "example.dist-info/LICENSE",
            "example.dist-info/licenses/Apache-2.0.txt",
            "notices/fontawesome.js.LICENSE.txt",
            "COPYING.BSD",
        ]
        rejected = ["example/test_license.py", "example/license_utils.py", "license-data.json"]

        self.assertTrue(all(_looks_like_license_file(value) for value in accepted))
        self.assertFalse(any(_looks_like_license_file(value) for value in rejected))

    def test_checked_in_dgx_summary_remains_fail_closed_and_path_free(self) -> None:
        """The retained review summary cannot silently become product acceptance."""
        summary_path = Path(__file__).parents[1] / "docs" / "local-image-linux-environment-review.json"
        summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(summary["pythonComponentCount"], 248)
        self.assertEqual(summary["nativeComponentCount"], 434)
        self.assertEqual(summary["blockers"], sorted(summary["blockers"], key=str.encode))
        self.assertEqual(len(summary["blockers"]), 76)
        self.assertFalse(summary["assemblyEligible"])
        self.assertFalse(summary["distributionReviewed"])
        self.assertNotIn("/home/", summary_path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
