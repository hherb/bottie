"""Test bounded requests, text preservation, PCM framing, and terminal outcomes."""

import base64
import struct
import unittest

from contract import completion_status, pcm_event, split_text, validate_request


class ContractTests(unittest.TestCase):
    """Exercise the trial boundary independently of model weights and ONNX Runtime."""

    def test_rejects_paths_cloning_and_unbounded_requests(self):
        """The trial only accepts bounded text and an advertised preset voice."""
        base = {"text": "Hallo Welt.", "voice": "Ava", "seed": 42}
        self.assertEqual(validate_request(base, {"Ava"}), base)
        for change in (
            {"path": "/private/audio"},
            {"text": ""},
            {"text": "x" * 2001},
            {"voice": "unknown"},
            {"seed": True},
            {"seed": -1},
        ):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request({**base, **change}, {"Ava"})

    def test_chunking_preserves_exact_text_and_respects_budget(self):
        """Chunking must retain accents, punctuation, and all non-whitespace content."""
        for text in ("Grüße aus Köln. Bitte höre genau zu!", "甲乙丙丁戊己庚辛", "oneaverylongwordwithoutspaces"):
            with self.subTest(text=text):
                chunks = split_text(text, len, 8)
                self.assertEqual("".join(chunks).replace(" ", ""), text.replace(" ", ""))
                self.assertTrue(all(0 < len(chunk) <= 8 for chunk in chunks))

    def test_pcm_is_finite_aligned_and_bounded(self):
        """Reject invalid or oversized float PCM before publishing it to a consumer."""
        payload = struct.pack("<ffff", 0.0, 0.5, -0.5, 1.0)
        event = pcm_event(payload, 0)
        self.assertEqual(base64.b64decode(event["pcm"]), payload)
        self.assertEqual(event["samplesPerChannel"], 2)
        for bad in (
            b"",
            b"x",
            struct.pack("<ff", float("nan"), 0),
            struct.pack("<ff", float("inf"), 0),
            b"\0" * (48000 * 8 + 8),
        ):
            with self.subTest(length=len(bad)), self.assertRaises(ValueError):
                pcm_event(bad, 0)

    def test_frame_limit_never_claims_completion(self):
        """Reaching the generation ceiling is reported as truncation, not success."""
        self.assertEqual(completion_status(12, 375), "complete")
        self.assertEqual(completion_status(375, 375), "frame_limit")
        self.assertEqual(completion_status(0, 375), "empty")


if __name__ == "__main__":
    unittest.main()
