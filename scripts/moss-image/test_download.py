"""Incomplete HTTP responses resume verified acquisition without discarding valid prefix bytes."""

import hashlib
import io
from http.client import IncompleteRead
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from assets import download_file


class DownloadRecoveryTests(unittest.TestCase):
    """A short transfer may resume, but a complete wrong digest must restart before publication."""

    def test_short_or_interrupted_response_resumes_its_existing_prefix(self):
        """Both ordinary short EOF and HTTP framing failure retain bytes for the next exact range."""
        for interrupted in [False, True]:
            with self.subTest(interrupted=interrupted), tempfile.TemporaryDirectory() as directory:
                first = io.BytesIO(b"abc")
                first.status, first.headers = 200, {}
                if interrupted:
                    first.read = Mock(side_effect=[b"abc", IncompleteRead(b"d", 2)])
                second = io.BytesIO(b"def")
                second.status, second.headers = 206, {"Content-Range": "bytes 3-5/6"}
                entry = dict(path="model", size=6, sha256=hashlib.sha256(b"abcdef").hexdigest())
                with patch("assets.urllib.request.urlopen", side_effect=[first, second]) as opened:
                    download_file(Path(directory), dict(id="approved/model", revision="pinned"), entry)
                self.assertEqual(opened.call_args_list[1].args[0].get_header("Range"), "bytes=3-")
                self.assertEqual((Path(directory) / "model").read_bytes(), b"abcdef")
                self.assertFalse((Path(directory) / "model.partial").exists())

    def test_complete_wrong_digest_restarts_without_promoting_changed_bytes(self):
        """Size alone never validates an asset; a full checksum mismatch discards the bad candidate."""
        with tempfile.TemporaryDirectory() as directory:
            first, second = io.BytesIO(b"abcdef"), io.BytesIO(b"uvwxyz")
            for response in [first, second]:
                response.status, response.headers = 200, {}
            entry = dict(path="model", size=6, sha256=hashlib.sha256(b"uvwxyz").hexdigest())
            with patch("assets.urllib.request.urlopen", side_effect=[first, second]) as opened:
                download_file(Path(directory), dict(id="approved/model", revision="pinned"), entry)
            self.assertIsNone(opened.call_args_list[1].args[0].get_header("Range"))
            self.assertEqual((Path(directory) / "model").read_bytes(), b"uvwxyz")


if __name__ == "__main__":
    unittest.main()
