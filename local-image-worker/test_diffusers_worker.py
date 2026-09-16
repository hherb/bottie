"""Focused dependency-free tests for Bottie's pinned Diffusers proof backend."""

from __future__ import annotations

import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock

from diffusers_worker import DiffusersBackend, main


class FakeImage:
    """Capture one requested PNG destination without producing model bytes."""

    def __init__(self) -> None:
        """Create an unsaved fake image."""
        self.saved: tuple[Path, str] | None = None

    def save(self, path: Path, format: str) -> None:
        """Record the exact native output contract."""
        self.saved = (path, format)


class FakePipeline:
    """Record exact local loading and generation arguments."""

    loaded: tuple[str, dict] | None = None

    def __init__(self) -> None:
        """Create a pipeline with no generation call."""
        self.call: dict | None = None
        self.image = FakeImage()

    @classmethod
    def from_pretrained(cls, path: str, **options):
        """Return one fake pipeline and retain load options."""
        cls.loaded = (path, options)
        return cls()

    def __call__(self, **options):
        """Return one fake image while retaining fixed profile options."""
        self.call = options
        return types.SimpleNamespace(images=[self.image])


class FakeGenerator:
    """Retain the CUDA generator device and deterministic seed."""

    def __init__(self, device: str) -> None:
        """Record the requested generator device."""
        self.device = device
        self.seed: int | None = None

    def manual_seed(self, seed: int):
        """Retain and return the deterministic seed."""
        self.seed = seed
        return self


class DiffusersBackendTests(unittest.TestCase):
    """Exercise the exact offline load and bounded CUDA generation profile."""

    def test_load_and_generate_use_only_the_local_pinned_profile(self) -> None:
        """The backend loads local bfloat16 bytes and emits one seeded 15-step PNG."""
        fake_torch = types.SimpleNamespace(bfloat16="bfloat16", Generator=FakeGenerator)
        fake_diffusers = types.SimpleNamespace(QwenImagePipeline=FakePipeline)
        with mock.patch.dict(sys.modules, {"torch": fake_torch, "diffusers": fake_diffusers}):
            backend = DiffusersBackend()
            with tempfile.TemporaryDirectory() as temporary:
                model = Path(temporary) / "model"
                model.mkdir()
                output = Path(temporary) / "generated.png"
                backend.load(model)
                progress = mock.Mock()
                backend.generate("A small blue circle", 512, 512, 42, output, progress)

        self.assertEqual(
            FakePipeline.loaded,
            (str(model), {"dtype": "bfloat16", "local_files_only": True, "device_map": "cuda"}),
        )
        pipeline = backend._pipeline
        self.assertEqual(pipeline.call["num_inference_steps"], 15)
        self.assertEqual(pipeline.call["width"], 512)
        self.assertEqual(pipeline.call["height"], 512)
        self.assertEqual(pipeline.call["generator"].device, "cuda")
        self.assertEqual(pipeline.call["generator"].seed, 42)
        pipeline.call["callback_on_step_end"](pipeline, 0, None, {})
        progress.assert_called_once_with(1, 15)
        self.assertEqual(pipeline.image.saved, (output, "PNG"))

    def test_clean_protocol_shutdown_skips_unbounded_cuda_destructors(self) -> None:
        """A completed worker loop exits immediately after every frame has flushed."""
        with tempfile.TemporaryDirectory() as temporary, mock.patch(
            "diffusers_worker.install_network_denial"
        ), mock.patch("diffusers_worker.Worker") as worker, mock.patch(
            "diffusers_worker.os._exit", side_effect=SystemExit
        ) as exit_process, mock.patch("sys.argv", ["worker", temporary]):
            with self.assertRaises(SystemExit):
                main()
        worker.return_value.run.assert_called_once_with()
        exit_process.assert_called_once_with(0)


if __name__ == "__main__":
    unittest.main()
