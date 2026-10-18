"""Pure tests for explicit MOSS asset installation and CUDA worker request guards."""

import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from cuda_worker import request
from assets import checked_path, verify_file


class MossTests(unittest.TestCase):
    """Reject damaged assets and malformed requests without importing GPU frameworks."""

    def test_inventory_rejects_escape_and_modified_assets(self):
        """Only exact approved bytes can enter a model installation."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(ValueError):
                checked_path(root, "../escape")
            file = root / "file"
            file.write_bytes(b"changed")
            self.assertFalse(verify_file(file, {"size": 7, "sha256": "0" * 64}))
            self.assertFalse(verify_file(file, {"size": 8, "sha256": "0" * 64}))

    def test_worker_rejects_invalid_sizes_seeds_steps_and_prompt_bounds(self):
        """A JSON message cannot request an unsupported image or unbounded GPU work."""
        data = dict(prompt='Layout: "Bottie"', width=512, height=512, seed=42, steps=50)
        encode = lambda d: io.BytesIO((json.dumps(d) + "\n").encode())
        self.assertEqual(request(encode(data)), data)
        for key, bad in [
            ("prompt", "x" * 32769),
            ("width", 0),
            ("height", 99999),
            ("seed", -1),
            ("seed", True),
            ("steps", 999),
        ]:
            with self.subTest(key=key, bad=str(bad)[:20]), self.assertRaises(ValueError):
                request(encode(dict(data, **{key: bad})))
        with self.assertRaises(ValueError):
            request(io.BytesIO(b"{}"))

    def test_registration_failure_preserves_the_previous_configuration(self):
        """Verify weights and runtime artifacts before atomically replacing a working registration."""
        from install import register

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "runtime.json").write_text("old configuration")
            with patch("install.verify_assets", side_effect=ValueError("damaged")):
                with self.assertRaises(ValueError):
                    register(root, {"id": "model", "revision": "rev", "files": []}, root, root / "python", "cuda")
            self.assertEqual((root / "runtime.json").read_text(), "old configuration")


class CudaVersionTests(unittest.TestCase):
    """Official CUDA wheel local-version tags do not change the pinned PyTorch release."""

    def test_accepts_cuda_wheel_suffix_but_rejects_a_different_release(self):
        """A valid torch +cu tag must not make reusable CUDA setup reject its own supported wheel."""
        from install import compatible_cuda_versions

        versions = {
            "torch": "2.14.1+cu130",
            "diffusers": "0.41.0",
            "transformers": "5.20.1",
            "accelerate": "1.15.0",
            "safetensors": "0.8.0",
            "Pillow": "13.0.0",
        }
        self.assertTrue(compatible_cuda_versions(versions))
        self.assertFalse(compatible_cuda_versions(dict(versions, torch="2.13.0+cu130")))


class RuntimeArtifactTests(unittest.TestCase):
    """Registration includes Metal resources while excluding unrelated build products."""

    def test_registers_only_the_executable_and_adjacent_runtime_resources(self):
        """Reusing a Swift build directory must not serialize all object files into native configuration."""
        from install import register

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build = root / "build"
            build.mkdir()
            executable = build / "BottieMossImage"
            executable.write_bytes(b"compiled worker")
            (build / "unrelated.o").write_bytes(b"not a runtime input")
            bundle = build / "mlx.bundle/Contents/Resources"
            bundle.mkdir(parents=True)
            (bundle / "default.metallib").write_bytes(b"metal")
            model = {"id": "mlx-community/MOSS-Image-SFT-bf16", "revision": "pinned", "files": []}
            with patch("install.verify_assets"):
                register(root, model, root, executable, "mlx")
            config = json.loads((root / "runtime.json").read_text())
            self.assertEqual(config["modelId"], model["id"])
            self.assertEqual(
                [f["path"] for f in config["artifacts"]], ["mlx.bundle/Contents/Resources/default.metallib"]
            )
            self.assertNotIn("unrelated.o", json.dumps(config))


class CudaWheelTests(unittest.TestCase):
    """ARM64 PyPI's CPU wheel must not be mistaken for a working CUDA installation."""

    def test_new_environment_installs_official_cuda_wheel_before_the_other_packages(self):
        """Use the CUDA index explicitly on both supported CUDA host platforms."""
        from install import cuda_python

        versions = {
            "torch": "2.14.1+cu130",
            "diffusers": "0.41.0",
            "transformers": "5.20.1",
            "accelerate": "1.15.0",
            "safetensors": "0.8.0",
            "Pillow": "13.0.0",
        }
        with tempfile.TemporaryDirectory() as directory, patch("sys.platform", "linux"), patch(
            "install.venv.EnvBuilder"
        ), patch("install.subprocess.run") as run:
            run.return_value.stdout = json.dumps({"versions": versions, "cuda": True, "bf16": True})
            cuda_python(Path(directory), None)
            first = run.call_args_list[0].args[0]
            self.assertIn("--index-url", first)
            self.assertIn("https://download.pytorch.org/whl/cu130", first)


if __name__ == "__main__":
    unittest.main()
