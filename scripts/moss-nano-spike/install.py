"""Explicitly provision optional Nano speech, then register native-only runtime paths for Bottie."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid
import venv

from assets import checked_path, download_file, load_lock, verify_assets

SCRIPT_DIR = Path(__file__).resolve().parent


def configured_python(root: Path) -> Path | None:
    """Reuse a compatible registered environment without mutating it or trusting a relative launcher."""
    try:
        path = Path(json.loads((root / "runtime.json").read_text())["pythonPath"])
        if not path.is_absolute() or not path.is_file():
            return None
        check_python(path)
        return path
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        return None


def create_environment(root: Path) -> Path:
    """Build at its final unique location, retaining standalone Python's interpreter links on Unix."""
    environment = root / "environments" / str(uuid.uuid4())
    try:
        venv.EnvBuilder(with_pip=True, symlinks=sys.platform != "win32").create(environment)
        python = environment / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        subprocess.run([str(python), "-m", "pip", "install", "-r", str(SCRIPT_DIR / "requirements.txt")], check=True)
        return python
    except BaseException:
        shutil.rmtree(environment, ignore_errors=True)
        raise


def check_python(python: Path) -> None:
    """Require the exact measured inference packages without importing Torch or accepting a shell command."""
    requirements = dict(
        line.split("==")
        for line in (SCRIPT_DIR / "requirements.txt").read_text().splitlines()
        if line and not line.startswith("#")
    )
    program = "import importlib.metadata,json; print(json.dumps({n:importlib.metadata.version(n) for n in "
    program += repr(list(requirements)) + "}))"
    result = subprocess.run([str(python), "-I", "-c", program], check=True, capture_output=True, text=True, timeout=30)
    if json.loads(result.stdout) != requirements:
        raise ValueError("Python inference package versions do not match the measured runtime")


def register_runtime(app_data: Path, python: Path, assets: Path) -> None:
    """Atomically publish configuration only after assets and the interpreter pass verification."""
    python = Path(os.path.abspath(python))  # Resolving this symlink would lose venv site-packages.
    assets = assets.resolve()
    if not python.is_file() or not assets.is_dir():
        raise ValueError("Runtime paths must exist")
    verify_assets(assets)
    check_python(python)
    root = app_data / "nano-speech"
    root.mkdir(parents=True, exist_ok=True)
    temporary = root / (str(uuid.uuid4()) + ".partial")
    try:
        temporary.write_text(json.dumps({"pythonPath": str(python), "assetDirectory": str(assets)}, indent=2) + "\n")
        temporary.replace(root / "runtime.json")
    finally:
        temporary.unlink(missing_ok=True)


def default_app_data() -> Path:
    """Match Tauri's per-user data location; --app-data supports sandbox-container installations."""
    if sys.platform == "darwin":
        return Path.home() / "Library/Application Support/com.bottie.app"
    if sys.platform == "win32":
        return Path(os.environ["APPDATA"]) / "com.bottie.app"
    return Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local/share"))) / "com.bottie.app"


def main() -> None:
    """Provision a dedicated environment/model cache only when this setup command is explicitly run."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-data", type=Path, default=default_app_data())
    parser.add_argument(
        "--reuse-python", type=Path, help="Reuse a verified existing inference venv instead of creating one"
    )
    parser.add_argument("--reuse-assets", type=Path, help="Register verified existing assets in place")
    parser.add_argument(
        "--source-cache", type=Path, help="Copy verified trial assets instead of downloading them again"
    )
    args = parser.parse_args()
    root = args.app_data.resolve() / "nano-speech"
    root.mkdir(parents=True, exist_ok=True)
    python = args.reuse_python or configured_python(root)
    if python is None:
        if sys.version_info[:2] != (3, 12):
            raise ValueError("Run setup with Python 3.12 or supply --reuse-python with the measured environment")
        python = create_environment(root)
    assets = args.reuse_assets or root / "assets"
    if args.source_cache:
        verify_assets(args.source_cache)
        for entry in load_lock()["files"]:
            destination = checked_path(assets, entry["path"])
            destination.parent.mkdir(parents=True, exist_ok=True)
            temporary = destination.with_name(destination.name + ".copy-partial")
            shutil.copyfile(checked_path(args.source_cache, entry["path"]), temporary)
            temporary.replace(destination)
    if args.reuse_assets is None:
        for entry in load_lock()["files"]:
            download_file(assets, entry)
    register_runtime(args.app_data.resolve(), python, assets)
    print("Nano installed. Restart Bottie, then choose MOSS Nano · Ava or MOSS Nano · Adam in Settings.")


if __name__ == "__main__":
    main()
