"""Dependency-free contract tests for the Linux NVIDIA proof harness."""

from __future__ import annotations

import hashlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

from diffusers_worker import DIFFUSERS_WORKER_IDENTITY
from prove_diffusers_worker import (
    COMPLETE_SEED,
    MODEL_ID,
    PROOF_DIMENSIONS,
    RUNTIME_PROFILES,
    assert_no_ucc_process_maps,
    assert_ucc_installation_absent,
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
            self.assertEqual(
                decoded_rgb_sha256(path), hashlib.sha256(pixels).hexdigest()
            )

    def test_proof_keeps_the_private_container_stdin_attached(self) -> None:
        """Docker must not replace Bottie's private input pipe with immediate EOF."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen",
                side_effect=RuntimeError("captured"),
            ) as popen:
                with self.assertRaisesRegex(RuntimeError, "captured"):
                    run_proof("proof-image", model, output)
        command = popen.call_args.args[0]
        self.assertIn("-i", command[: command.index("--name")])
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--read-only", command)
        self.assertEqual(command[command.index("--cap-drop") + 1], "ALL")
        self.assertEqual(
            command[command.index("--security-opt") + 1], "no-new-privileges"
        )
        self.assertEqual(command[command.index("--gpus") + 1], "all")
        self.assertIn("--user", command)
        self.assertIn(f"{model}:/model:ro", command)
        self.assertIn(f"{output}:/output:rw", command)
        self.assertEqual(popen.call_args.kwargs["bufsize"], 0)

    def test_explicit_runtime_trace_is_read_only_and_does_not_replace_proof_isolation(
        self,
    ) -> None:
        """Tracing adds only bounded source/output mounts and an injected Python recorder."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            trace_source, trace_output = root / "source", root / "trace"
            for directory in (model, output, trace_source, trace_output):
                directory.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen",
                side_effect=RuntimeError("captured"),
            ) as popen:
                with self.assertRaisesRegex(RuntimeError, "captured"):
                    run_proof(
                        "proof-image",
                        model,
                        output,
                        trace_source=trace_source,
                        trace_output=trace_output,
                    )

        command = popen.call_args.args[0]
        self.assertIn(f"{trace_source}:/trace-source:ro", command)
        self.assertIn(f"{trace_output}:/runtime-trace:rw", command)
        self.assertEqual(command[command.index("--entrypoint") + 1], "python")
        self.assertIn(
            "BOTTIE_RUNTIME_TRACE_FILE=/runtime-trace/python-paths.jsonl", command
        )
        self.assertTrue(
            any("import diffusers_runtime_trace" in argument for argument in command)
        )
        self.assertEqual(command[command.index("--network") + 1], "none")

    def test_ucc_ablation_masks_the_complete_installation_read_only(self) -> None:
        """The opt-in proof hides all UCC bytes without weakening container isolation."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen",
                side_effect=RuntimeError("captured"),
            ) as popen:
                with self.assertRaisesRegex(RuntimeError, "captured"):
                    run_proof("proof-image", model, output, ablate_ucc=True)

        command = popen.call_args.args[0]
        ucc_mounts = [
            argument for argument in command if argument.endswith(":/opt/hpcx/ucc:ro")
        ]
        self.assertEqual(len(ucc_mounts), 1)
        self.assertEqual(command[command.index("--network") + 1], "none")
        self.assertIn("--read-only", command)
        self.assertEqual(command[command.index("--cap-drop") + 1], "ALL")

    def test_runtime_profiles_bind_the_expected_worker_script_and_identity(
        self,
    ) -> None:
        """Each selectable proof profile names one immutable in-image entrypoint."""
        ngc = RUNTIME_PROFILES["ngc-25.11"]
        pytorch = RUNTIME_PROFILES["pytorch-2.10-cu130"]
        clean = RUNTIME_PROFILES["pytorch-2.10-cu130-clean"]

        self.assertEqual(ngc.identity, DIFFUSERS_WORKER_IDENTITY)
        self.assertEqual(ngc.worker_script, "/opt/bottie/diffusers_worker.py")
        self.assertEqual(
            pytorch.identity.runtime_id,
            "diffusers@0.40.0+pytorch-2.10.0-cu130-ubuntu24.04-arm64",
        )
        self.assertEqual(
            pytorch.worker_script, "/opt/bottie/diffusers_pytorch_worker.py"
        )
        self.assertEqual(clean.identity, pytorch.identity)
        self.assertEqual(clean.python_executable, "/opt/bottie/venv/bin/python")
        self.assertEqual(clean.ucc_policy, "absent")

    def test_pytorch_wheel_profile_requires_the_ucc_ablation_gate(self) -> None:
        """The alternative profile cannot produce evidence without proving UCC absence."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with self.assertRaisesRegex(ValueError, "requires UCC ablation"):
                run_proof(
                    "proof-image",
                    model,
                    output,
                    profile=RUNTIME_PROFILES["pytorch-2.10-cu130"],
                )

    def test_clean_profile_uses_venv_python_and_forbids_ucc_masking(self) -> None:
        """The rebuilt runtime traces its exact interpreter without hiding UCC bytes."""
        clean = RUNTIME_PROFILES["pytorch-2.10-cu130-clean"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            trace_source, trace_output = root / "source", root / "trace"
            for directory in (model, output, trace_source, trace_output):
                directory.mkdir()
            with self.assertRaisesRegex(ValueError, "must not use UCC ablation"):
                run_proof("proof-image", model, output, ablate_ucc=True, profile=clean)
            with mock.patch(
                "prove_diffusers_worker.prove_network_denial"
            ) as denial, mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen",
                side_effect=RuntimeError("captured"),
            ) as popen:
                with self.assertRaisesRegex(RuntimeError, "captured"):
                    run_proof(
                        "proof-image",
                        model,
                        output,
                        trace_source=trace_source,
                        trace_output=trace_output,
                        profile=clean,
                    )

        denial.assert_called_once_with("proof-image", clean.python_executable)
        command = popen.call_args.args[0]
        self.assertEqual(
            command[command.index("--entrypoint") + 1], clean.python_executable
        )

    def test_clean_profile_checks_that_the_ucc_installation_is_absent(self) -> None:
        """The clean proof distinguishes a missing installation from a masked one."""
        completed = mock.Mock(returncode=0)
        with mock.patch(
            "diffusers_ucc_ablation.subprocess.run", return_value=completed
        ) as run:
            assert_ucc_installation_absent("proof-container", "/venv/python")

        self.assertEqual(
            run.call_args.args[0][:4],
            ["docker", "exec", "proof-container", "/venv/python"],
        )
        completed.returncode = 1
        with mock.patch(
            "diffusers_ucc_ablation.subprocess.run", return_value=completed
        ):
            with self.assertRaisesRegex(RuntimeError, "UCC installation is present"):
                assert_ucc_installation_absent("proof-container", "/venv/python")

    def test_ucc_ablation_rejects_a_surviving_runtime_mapping(self) -> None:
        """A masked installation is insufficient if a UCC file remains mapped."""
        assert_no_ucc_process_maps(
            b"7f00-7f10 r-xp 0000 00:00 0 /usr/lib/libcuda.so.1\n"
        )
        with self.assertRaisesRegex(RuntimeError, "UCC runtime remained mapped"):
            assert_no_ucc_process_maps(
                b"7f00-7f10 r-xp 0000 00:00 0 /opt/hpcx/ucc/lib/libucc.so.1.0.0\n"
            )
        with self.assertRaisesRegex(RuntimeError, "UCC runtime remained mapped"):
            assert_no_ucc_process_maps(
                b"7f00-7f10 r-xp 0000 00:00 0 /usr/lib/aarch64-linux-gnu/libucc.so.1\n"
            )

    def test_ucc_mask_is_checked_after_the_worker_handshake(self) -> None:
        """Docker must finish registering the live container before an exec-based mask check."""
        events = []
        process = mock.Mock(stdin=io.BytesIO(), stdout=io.BytesIO())
        process.poll.return_value = 0

        def read_event(*_arguments):
            events.append("read")
            if len(events) == 1:
                return {"workerVersion": DIFFUSERS_WORKER_IDENTITY.worker_version}
            return {"capabilities": {"runtimeId": DIFFUSERS_WORKER_IDENTITY.runtime_id}}

        def check_mask(*_arguments):
            events.append("mask")
            raise RuntimeError("mask checked")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen", return_value=process
            ), mock.patch(
                "prove_diffusers_worker.read_frame_with_timeout", side_effect=read_event
            ), mock.patch(
                "prove_diffusers_worker.assert_ucc_installation_masked",
                side_effect=check_mask,
            ), mock.patch(
                "prove_diffusers_worker.subprocess.run"
            ):
                with self.assertRaisesRegex(RuntimeError, "mask checked"):
                    run_proof("proof-image", model, output, ablate_ucc=True)

        self.assertEqual(events, ["read", "read", "mask"])

    def test_clean_ucc_absence_is_checked_after_the_worker_handshake(self) -> None:
        """The clean profile checks its live container before importing the model stack."""
        clean = RUNTIME_PROFILES["pytorch-2.10-cu130-clean"]
        events = []
        process = mock.Mock(stdin=io.BytesIO(), stdout=io.BytesIO())
        process.poll.return_value = 0

        def read_event(*_arguments):
            events.append("read")
            if len(events) == 1:
                return {"workerVersion": clean.identity.worker_version}
            return {"capabilities": {"runtimeId": clean.identity.runtime_id}}

        def check_absence(*_arguments):
            events.append("absence")
            raise RuntimeError("absence checked")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen", return_value=process
            ), mock.patch(
                "prove_diffusers_worker.read_frame_with_timeout", side_effect=read_event
            ), mock.patch(
                "prove_diffusers_worker.assert_ucc_installation_absent",
                side_effect=check_absence,
            ), mock.patch(
                "prove_diffusers_worker.subprocess.run"
            ):
                with self.assertRaisesRegex(RuntimeError, "absence checked"):
                    run_proof("proof-image", model, output, profile=clean)

        self.assertEqual(events, ["read", "read", "absence"])

    def test_ucc_maps_are_checked_before_model_load(self) -> None:
        """The ablation proof observes live mappings before importing the model stack."""
        process = mock.Mock(stdin=io.BytesIO(), stdout=io.BytesIO())
        process.poll.return_value = 0
        frames = [
            {"workerVersion": DIFFUSERS_WORKER_IDENTITY.worker_version},
            {"capabilities": {"runtimeId": DIFFUSERS_WORKER_IDENTITY.runtime_id}},
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model, output = root / "model", root / "output"
            model.mkdir()
            output.mkdir()
            with mock.patch("prove_diffusers_worker.prove_network_denial"), mock.patch(
                "prove_diffusers_worker.host_available_bytes", return_value=1
            ), mock.patch(
                "prove_diffusers_worker.subprocess.Popen", return_value=process
            ), mock.patch(
                "prove_diffusers_worker.read_frame_with_timeout", side_effect=frames
            ), mock.patch(
                "prove_diffusers_worker.assert_ucc_installation_masked"
            ), mock.patch(
                "prove_diffusers_worker.capture_process_maps",
                side_effect=RuntimeError("maps checked"),
            ) as capture, mock.patch(
                "prove_diffusers_worker.subprocess.run"
            ):
                with self.assertRaisesRegex(RuntimeError, "maps checked"):
                    run_proof("proof-image", model, output, ablate_ucc=True)

        capture.assert_called_once()
        self.assertEqual(capture.call_args.kwargs, {"reject_ucc": True})

    def test_missing_nvidia_process_counter_is_not_reported_as_zero_use(self) -> None:
        """An unsupported or absent UMA counter remains explicitly unavailable."""
        completed = mock.Mock(stdout="914, 1024 MiB\n")
        with mock.patch(
            "diffusers_proof_memory.subprocess.run", return_value=completed
        ):
            self.assertIsNone(nvidia_process_bytes(915))
            self.assertEqual(nvidia_process_bytes(914), 1024 * 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
