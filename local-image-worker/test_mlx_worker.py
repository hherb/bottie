"""Focused standard-library tests for Bottie's private MLX worker policy."""

from __future__ import annotations

import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from mlx_worker import (
    CANCELLATION_POLL_WINDOW_SECONDS,
    MAX_PROMPT_BYTES,
    MODEL_ID,
    MODEL_REVISION,
    NetworkDeniedError,
    CancellationController,
    ProgressCallback,
    Worker,
    WorkerIdentity,
    read_frame,
    report_backend_failure,
    require_generate,
    require_hello,
    require_load,
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

    def test_generation_identity_is_explicit_per_worker_backend(self) -> None:
        """A second runtime cannot inherit the MLX model revision accidentally."""
        identity = WorkerIdentity(
            worker_version="diffusers-proof",
            runtime_id="diffusers@proof",
            model_id=MODEL_ID,
            model_revision="linux-proof-revision",
        )
        command = {
            "type": "load",
            "protocolVersion": 1,
            "requestId": "load-proof",
            "model": {
                "modelId": MODEL_ID,
                "modelRevision": "linux-proof-revision",
                "modelDirectory": "/missing",
            },
        }
        with self.assertRaises(FileNotFoundError):
            require_load(command, identity)
        command["model"]["modelRevision"] = MODEL_REVISION
        with self.assertRaises(ValueError):
            require_load(command, identity)

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

    def test_cancel_that_arrives_before_its_request_begins_is_honored(self) -> None:
        """A queued request whose cancel already arrived stops as soon as it begins."""
        controller = CancellationController()
        controller.begin("running")
        self.assertFalse(controller.request("queued"))
        controller.finish()
        controller.begin("unrelated")
        controller.finish()
        with self.assertRaises(KeyboardInterrupt):
            controller.begin("queued")
        self.assertTrue(controller.finish())
        controller.begin("queued-again")
        self.assertFalse(controller.finish())

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
        with mock.patch("sys.stderr", new_callable=io.StringIO) as stderr:
            report_backend_failure("generate", NetworkDeniedError("https://private.example"))
        self.assertEqual(stderr.getvalue(), "bottie-worker-generate-network-attempt-denied\n")

    def test_private_loop_loads_generates_and_shuts_down_in_order(self) -> None:
        """The real loop emits correlated handshake, progress, and terminal frames."""
        decoded = self._run_private_loop(cancel_id=None)
        self.assertEqual([event["type"] for event in decoded[:2]], ["hello", "capabilities"])
        terminals = [event for event in decoded if event["type"] == "result"]
        self.assertEqual([event["operation"] for event in terminals], ["load", "generate"])
        self.assertEqual(terminals[-1]["result"]["outputs"][0]["outputName"], "generated.png")

    def test_late_cancel_for_a_finished_request_leaves_the_worker_alive(self) -> None:
        """A cancel that matches no active request is dropped and later generation still works."""
        decoded = self._run_private_loop(cancel_id="finished-request")
        terminals = [event for event in decoded if event["type"] == "result"]
        self.assertEqual([event["operation"] for event in terminals], ["load", "generate"])
        self.assertEqual(terminals[-1]["result"]["status"], "completed")

    def test_cancel_read_before_generate_begins_ends_it_cancelled(self) -> None:
        """A cancel the reader handles before the main thread starts the request still applies."""
        decoded = self._run_private_loop(cancel_id="generate-proof")
        terminals = [event for event in decoded if event["type"] == "result"]
        self.assertEqual([event["result"]["status"] for event in terminals], ["completed", "cancelled"])

    def test_interrupt_escaping_a_failure_handler_still_reports_cancelled(self) -> None:
        """A cancel landing while a backend failure is handled cannot kill the worker."""

        def failing_generate(*args, **kwargs):
            raise RuntimeError("backend failed")

        with (
            mock.patch.object(FakeBackend, "generate", failing_generate),
            mock.patch("mlx_worker.report_backend_failure", side_effect=KeyboardInterrupt),
        ):
            decoded = self._run_private_loop(cancel_id=None)
        terminals = [event for event in decoded if event["type"] == "result"]
        self.assertEqual([event["result"]["status"] for event in terminals], ["completed", "cancelled"])

    def test_stray_interrupt_while_idle_does_not_stop_the_worker(self) -> None:
        """A KeyboardInterrupt that lands while waiting for a command is ignored."""
        original_get = __import__("queue").Queue.get
        calls = []

        def interrupted_once(queue_self, *args, **kwargs):
            calls.append(None)
            if len(calls) == 1:
                raise KeyboardInterrupt
            return original_get(queue_self, *args, **kwargs)

        with mock.patch("queue.Queue.get", interrupted_once):
            decoded = self._run_private_loop(cancel_id=None)
        terminals = [event for event in decoded if event["type"] == "result"]
        self.assertEqual([event["operation"] for event in terminals], ["load", "generate"])

    def _run_private_loop(self, cancel_id: str | None) -> list[dict]:
        """Run the real loop over load, an optional cancel, generate, and shutdown frames."""
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
            if cancel_id is not None:
                write_frame(commands, {"type": "cancel", "protocolVersion": 1, "requestId": cancel_id})
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
            return decoded


if __name__ == "__main__":
    unittest.main()
