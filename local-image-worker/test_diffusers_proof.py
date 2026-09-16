"""Dependency-free contract tests for the Linux NVIDIA proof harness."""

from __future__ import annotations

import hashlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

from prove_diffusers_worker import (
    COMPLETE_SEED,
    MODEL_ID,
    PROOF_DIMENSIONS,
    decoded_rgb_sha256,
    generation_command,
    nvidia_process_bytes,
    read_frame,
    run_proof,
    write_frame,
)


class DiffusersProofTests(unittest.TestCase):
    """Exercise framing, exact request shape, and decoded output evidence."""

    def test_proof_frame_round_trip_is_protocol_compatible(self) -> None:
        """The proof client emits the same bounded big-endian JSON framing."""
        stream = io.BytesIO()
        message = generation_command("proof", COMPLETE_SEED)
        write_frame(stream, message)
        stream.seek(0)
        self.assertEqual(read_frame(stream), message)
        self.assertEqual(message["modelId"], MODEL_ID)
        self.assertEqual((message["width"], message["height"]), PROOF_DIMENSIONS)
        self.assertEqual(message["count"], 1)

    def test_decoded_png_evidence_hashes_rgb_pixels(self) -> None:
        """PNG encoding differences cannot change the accepted decoded-pixel digest."""
        pixels = bytes([37, 91, 143]) * (PROOF_DIMENSIONS[0] * PROOF_DIMENSIONS[1])
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "proof.png"
            Image.frombytes("RGB", PROOF_DIMENSIONS, pixels).save(path, format="PNG")
            self.assertEqual(decoded_rgb_sha256(path), hashlib.sha256(pixels).hexdigest())

    def test_proof_keeps_the_private_container_stdin_attached(self) -> None:
        """Docker must not replace Bottie's private input pipe with immediate EOF."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch("prove_diffusers_worker.subprocess.Popen", side_effect=RuntimeError("captured")) as popen:
                with self.assertRaisesRegex(RuntimeError, "captured"):
                    run_proof("proof-image", model, output)
        command = popen.call_args.args[0]
        self.assertIn("-i", command[: command.index("--name")])
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--read-only", command)
        self.assertEqual(command[command.index("--cap-drop") + 1], "ALL")
        self.assertEqual(command[command.index("--security-opt") + 1], "no-new-privileges")
        self.assertEqual(command[command.index("--gpus") + 1], "all")
        self.assertIn("--user", command)
        self.assertIn(f"{model}:/model:ro", command)
        self.assertIn(f"{output}:/output:rw", command)
        self.assertEqual(popen.call_args.kwargs["bufsize"], 0)

    def test_missing_nvidia_process_counter_is_not_reported_as_zero_use(self) -> None:
        """An unsupported or absent UMA counter remains explicitly unavailable."""
        completed = mock.Mock(stdout="914, 1024 MiB\n")
        with mock.patch("prove_diffusers_worker.subprocess.run", return_value=completed):
            self.assertIsNone(nvidia_process_bytes(915))
            self.assertEqual(nvidia_process_bytes(914), 1024 * 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
