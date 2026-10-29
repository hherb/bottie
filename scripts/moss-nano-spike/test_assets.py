"""Exercise asset integrity and offline failure without downloading model weights."""

import hashlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from assets import checked_path, download_file, verify_file, validate_inventory


class AssetTests(unittest.TestCase):
    """Keep model acquisition bounded to the exact reviewed inventory."""

    def test_rejects_unsafe_and_duplicate_destinations(self):
        """Remote inventories cannot escape their cache or replace duplicate paths."""
        for name in ("../outside", "/absolute", "a/../../outside", "a\\b"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                checked_path(Path("/tmp/cache"), name)
        entry = {"path": "model.onnx", "size": 3, "sha256": "a" * 64, "url": "https://example.com/a"}
        with self.assertRaises(ValueError):
            validate_inventory([entry, entry])

    def test_rejects_symlink_escape(self):
        """An existing symlink must not redirect a verified download outside its cache."""
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            root = Path(directory)
            (root / "link").symlink_to(outside, target_is_directory=True)
            with self.assertRaises(ValueError):
                checked_path(root, "link/weights")

    def test_size_and_digest_both_required(self):
        """Equal-size corruption and truncated assets both fail before inference."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "weights"
            entry = {"size": 3, "sha256": hashlib.sha256(b"abc").hexdigest()}
            self.assertFalse(verify_file(path, entry))
            for payload, expected in ((b"abc", True), (b"abd", False), (b"ab", False)):
                path.write_bytes(payload)
                self.assertEqual(verify_file(path, entry), expected)

    def test_rejects_unbounded_or_incomplete_identity(self):
        """Every asset needs a positive size, a digest, and an HTTPS origin."""
        base = {"path": "weights", "size": 3, "sha256": "a" * 64, "url": "https://example.com/a"}
        for field, value in (("size", -1), ("size", True), ("sha256", "bad"), ("url", "http://example.com/a")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_inventory([{**base, field: value}])

    def test_download_only_promotes_verified_complete_bytes(self):
        """Interrupted, oversized, or corrupt responses cannot become reusable model files."""
        entry = {
            "path": "weights",
            "size": 3,
            "sha256": hashlib.sha256(b"abc").hexdigest(),
            "url": "https://example.com/weights",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for bad in (b"ab", b"abd", b"abcd"):
                with patch("assets.urllib.request.urlopen", return_value=io.BytesIO(bad)):
                    with self.assertRaises(ValueError):
                        download_file(root, entry)
                self.assertFalse((root / "weights").exists())
                self.assertFalse((root / "weights.partial").exists())
            with patch("assets.urllib.request.urlopen", return_value=io.BytesIO(b"abc")):
                download_file(root, entry)
            with patch("assets.urllib.request.urlopen", side_effect=AssertionError("Unexpected network access")):
                download_file(root, entry)
            self.assertTrue(verify_file(root / "weights", entry))


if __name__ == "__main__":
    unittest.main()
