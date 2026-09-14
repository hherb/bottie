"""Focused standard-library tests for Bottie's private MLX worker policy."""

from __future__ import annotations

import io
import tempfile
import unittest
from unittest import mock
from pathlib import Path

from mlx_worker import (
    MAX_PROMPT_BYTES,
    MODEL_ID,
    MODEL_REVISION,
    CANCELLATION_POLL_WINDOW_SECONDS,
    CancellationController,
    ProgressCallback,
    Worker,
    read_frame,
    report_backend_failure,
    require_generate,
    require_hello,
    write_frame,
)


class FakeBackend:
    """Deterministic no-runtime backend for the private-loop contract test."""

    def load(self, model_directory: Path) -> None:
        """Accept the test's existing model directory."""
        if not model_directory.is_dir():
            raise RuntimeError("missing fixture model")

    def generate(self, prompt, width, height, seed, output_path, progress) -> None:
        """Emit one progress step and one inert output fixture."""
        progress(1, 15)
        output_path.write_bytes(b"fixture")


class WorkerPolicyTests(unittest.TestCase):
    """Exercise closed frames, fixed generation, and exact cancellation correlation."""

    def test_frame_round_trip_and_closed_hello(self) -> None:
        """One compact frame round-trips while future hello fields fail closed."""
        message = {"type": "hello", "protocolVersion": 1, "clientVersion": "bottie-test"}
        stream = io.BytesIO()
        write_frame(stream, message)
        stream.seek(0)
        decoded = read_frame(stream)
        self.assertEqual(decoded, message)
        require_hello(decoded)
        decoded["future"] = True
        with self.assertRaises(ValueError):
            require_hello(decoded)

    def test_generation_is_fixed_to_one_seeded_512_square_output(self) -> None:
        """The proof worker rejects alternate dimensions, counts, and oversized prompts."""
        command = {
            "type": "generate",
            "protocolVersion": 1,
            "requestId": "proof-generate",
            "modelId": MODEL_ID,
            "prompt": "A violet glass robot tending a tiny greenhouse",
            "width": 512,
            "height": 512,
            "count": 1,
            "seed": 42,
        }
        values = require_generate(command, True)
        self.assertEqual(values[2:], (512, 512, 42))
        command["prompt"] = "x" * (MAX_PROMPT_BYTES + 1)
        with self.assertRaises(ValueError):
            require_generate(command, True)

    def test_cancellation_matches_only_the_active_request(self) -> None:
        """Unknown and duplicate cancellation cannot interrupt unrelated work."""
        controller = CancellationController()
        controller.begin("active")
        self.assertFalse(controller.request("other"))
        with mock.patch("_thread.interrupt_main") as interrupt:
            self.assertTrue(controller.request("active"))
            self.assertFalse(controller.request("active"))
            interrupt.assert_called_once_with()
        self.assertTrue(controller.finish())

    def test_denoising_progress_yields_one_bounded_cancellation_window(self) -> None:
        """Every completed step gives the command reader one fixed cancellation window."""
        callback = ProgressCallback()
        callback.reporter = mock.Mock()
        config = mock.Mock(num_inference_steps=15)
        with mock.patch("time.sleep") as sleep:
            callback.call_in_loop(0, config)
        callback.reporter.assert_called_once_with(1, 15)
        sleep.assert_called_once_with(CANCELLATION_POLL_WINDOW_SECONDS)

    def test_output_directory_must_already_exist(self) -> None:
        """The worker never creates or follows an untrusted output root."""
        from mlx_worker import validate_output_directory

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.assertEqual(validate_output_directory(root), root.resolve())
            with self.assertRaises(FileNotFoundError):
                validate_output_directory(root / "missing")

    def test_backend_diagnostics_are_fixed_and_path_free(self) -> None:
        """Backend failures disclose only one stable category on stderr."""
        with mock.patch("sys.stderr", new_callable=io.StringIO) as stderr:
            report_backend_failure("load", ModuleNotFoundError("/private/model/secret.py"))
        self.assertEqual(stderr.getvalue(), "bottie-worker-load-dependency-missing\n")

    def test_private_loop_loads_generates_and_shuts_down_in_order(self) -> None:
        """The real loop emits correlated handshake, progress, and terminal frames."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model = root / "model"
            output = root / "output"
            model.mkdir()
            output.mkdir()
            commands = io.BytesIO()
            write_frame(commands, {"type": "hello", "protocolVersion": 1, "clientVersion": "bottie-test"})
            write_frame(
                commands,
                {
                    "type": "load",
                    "protocolVersion": 1,
                    "requestId": "load-proof",
                    "model": {
                        "modelId": MODEL_ID,
                        "modelRevision": MODEL_REVISION,
                        "modelDirectory": str(model),
                    },
                },
            )
            write_frame(
                commands,
                {
                    "type": "generate",
                    "protocolVersion": 1,
                    "requestId": "generate-proof",
                    "modelId": MODEL_ID,
                    "prompt": "A violet glass robot tending a tiny greenhouse",
                    "width": 512,
                    "height": 512,
                    "count": 1,
                    "seed": 42,
                },
            )
            write_frame(commands, {"type": "shutdown", "protocolVersion": 1})
            commands.seek(0)
            events = io.BytesIO()

            Worker(commands, events, output, FakeBackend).run()

            events.seek(0)
            decoded = []
            while events.tell() < len(events.getbuffer()):
                decoded.append(read_frame(events))
            self.assertEqual([event["type"] for event in decoded[:2]], ["hello", "capabilities"])
            terminals = [event for event in decoded if event["type"] == "result"]
            self.assertEqual([event["operation"] for event in terminals], ["load", "generate"])
            self.assertEqual(terminals[-1]["result"]["outputs"][0]["outputName"], "generated.png")


if __name__ == "__main__":
    unittest.main()
