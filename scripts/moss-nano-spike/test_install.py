"""Exercise explicit native registration without provisioning models or Python packages."""

from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

from install import configured_python, create_environment, register_runtime


class InstallationTests(unittest.TestCase):
    """A failed setup must not replace a working runtime configuration."""

    def test_repeated_setup_reuses_a_compatible_registered_environment(self):
        """An unchanged optional installation must not accumulate duplicate Python environments."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            python = root / "python"
            python.touch()
            (root / "runtime.json").write_text(json.dumps({"pythonPath": str(python)}))
            with patch("install.check_python") as check:
                self.assertEqual(configured_python(root), python)
                check.assert_called_once_with(python)
            with patch("install.check_python", side_effect=ValueError("incompatible")):
                self.assertIsNone(configured_python(root))

    def test_environment_creation_preserves_interpreter_links_and_cleans_only_its_candidate(self):
        """Standalone macOS Python cannot be relocated by copying its executable into a venv."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            retained = root / "retained-environment"
            retained.mkdir()
            with patch("install.venv.EnvBuilder") as builder, patch("install.subprocess.run"):
                environment = create_environment(root)
                self.assertTrue(environment.is_relative_to(root))
                self.assertEqual(builder.call_args.kwargs["symlinks"], sys.platform != "win32")
            with patch("install.venv.EnvBuilder") as builder, patch("install.subprocess.run"):
                builder.return_value.create.side_effect = RuntimeError("setup failed")
                with self.assertRaises(RuntimeError):
                    create_environment(root)
            self.assertTrue(retained.is_dir())

    def test_registration_requires_verified_assets_and_compatible_python(self):
        """Keep paths native and publish configuration only after all checks pass."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            python = root / "python"
            python.touch()
            assets = root / "assets"
            assets.mkdir()
            with patch("install.verify_assets", return_value={}), patch("install.check_python"):
                register_runtime(root, python, assets)
            path = root / "nano-speech/runtime.json"
            original = path.read_bytes()
            self.assertEqual(json.loads(original)["pythonPath"], str(python))
            with patch("install.verify_assets", side_effect=ValueError("corrupt")), patch("install.check_python"):
                with self.assertRaises(ValueError):
                    register_runtime(root, python, assets)
            self.assertEqual(path.read_bytes(), original)
            with patch("install.verify_assets", return_value={}), patch("install.check_python", side_effect=ValueError):
                with self.assertRaises(ValueError):
                    register_runtime(root, python, assets)
            self.assertEqual(path.read_bytes(), original)

    def test_interpreter_symlink_is_not_resolved_out_of_its_virtual_environment(self):
        """The venv launcher path must be retained so its own site-packages remain available."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "base-python"
            target.touch()
            launcher = root / "venv-python"
            launcher.symlink_to(target)
            with patch("install.verify_assets", return_value={}), patch("install.check_python"):
                register_runtime(root, launcher, root)
            config = json.loads((root / "nano-speech/runtime.json").read_text())
            self.assertEqual(config["pythonPath"], str(launcher))


if __name__ == "__main__":
    unittest.main()
