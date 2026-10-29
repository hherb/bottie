"""Build/provision the platform MOSS worker, verify its model, and atomically register native paths."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import uuid
import venv

from assets import download_assets, selected_model, verify_assets

ROOT = Path(__file__).resolve().parent
CUDA_WHEEL_VARIANT = "cu130"
CUDA_WHEEL_INDEX = "https://download.pytorch.org/whl/" + CUDA_WHEEL_VARIANT


def app_data():
    """Match Tauri's per-user data path; callers can override sandbox-container locations."""
    if sys.platform == "darwin":
        return Path.home() / "Library/Application Support/com.bottie.app"
    if sys.platform == "win32":
        return Path(os.environ["APPDATA"]) / "com.bottie.app"
    return Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local/share"))) / "com.bottie.app"


def artifact(path, root):
    """Describe exact executable/resource bytes without reading a full native binary into memory."""
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return dict(path=path.relative_to(root).as_posix(), size=path.stat().st_size, sha256=digest)


def validate_mlx_host():
    """Fail before a large model download when the Apple runtime cannot be built on this host."""
    if platform.machine() != "arm64" or int(platform.mac_ver()[0].split(".")[0]) < 26:
        raise ValueError("MOSS MLX requires Apple Silicon and macOS 26 or later")
    compiler = subprocess.run(["xcrun", "metal", "--version"], capture_output=True)
    if compiler.returncode:
        raise ValueError("Install Xcode's Metal compiler: xcodebuild -downloadComponent MetalToolchain")


def build_mlx(root):
    """Build the pinned Swift port and retain its Metal resource bundles beside the executable."""
    validate_mlx_host()
    package = ROOT / "swift"
    subprocess.run(
        [
            "swift",
            "build",
            "--package-path",
            str(package),
            "-c",
            "release",
            "--product",
            "BottieMossImage",
            "--force-resolved-versions",
        ],
        check=True,
    )
    build = Path(
        subprocess.check_output(
            ["swift", "build", "--package-path", str(package), "-c", "release", "--show-bin-path"], text=True
        ).strip()
    )
    destination = root / "runtimes" / str(uuid.uuid4())
    destination.mkdir(parents=True)
    shutil.copy2(build / "BottieMossImage", destination)
    for source in build.iterdir():
        if source.suffix == ".bundle" and source.is_dir():
            shutil.copytree(source, destination / source.name)
        elif source.suffix in {".metallib", ".dylib"} and source.is_file():
            shutil.copy2(source, destination / source.name)
    executable = destination / "BottieMossImage"
    subprocess.run([str(executable), "--probe"], check=True, cwd=destination, capture_output=True, timeout=60)
    return executable


def cuda_requirements():
    """Read the one committed direct-version inventory for acquisition and environment verification."""
    return dict(
        line.split("==")
        for line in (ROOT / "requirements-cuda.txt").read_text().splitlines()
        if line and not line.startswith("#")
    )


def compatible_cuda_versions(versions):
    """Accept the pinned release including official PyTorch +cu wheel tags, never another base release."""
    requirements = cuda_requirements()
    return all(
        (versions.get(name, "").split("+", 1)[0] if name == "torch" else versions.get(name)) == version
        for name, version in requirements.items()
    )


def cuda_python(root, reuse):
    """Use an explicit dedicated venv and require actual CUDA rather than falling back to CPU/MPS."""
    if sys.platform not in {"linux", "win32"}:
        raise ValueError("CUDA MOSS is supported on Linux/Windows NVIDIA hosts")
    requirements = cuda_requirements()
    if reuse:
        python = Path(os.path.abspath(reuse))
    else:
        environment = root / "runtimes" / str(uuid.uuid4())
        venv.EnvBuilder(with_pip=True, symlinks=sys.platform != "win32").create(environment)
        python = environment / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        subprocess.run(
            [
                str(python),
                "-m",
                "pip",
                "install",
                f"torch=={requirements['torch']}+{CUDA_WHEEL_VARIANT}",
                "--index-url",
                CUDA_WHEEL_INDEX,
            ],
            check=True,
        )
        subprocess.run([str(python), "-m", "pip", "install", "-r", str(ROOT / "requirements-cuda.txt")], check=True)
    program = "import importlib.metadata,json,torch; names=" + repr(list(requirements))
    program += "; print(json.dumps({'versions':{n:importlib.metadata.version(n) for n in names},"
    program += "'cuda':torch.cuda.is_available(),'bf16':torch.cuda.is_bf16_supported()}))"
    result = subprocess.run([str(python), "-I", "-c", program], check=True, timeout=60, capture_output=True, text=True)
    status = json.loads(result.stdout)
    if not compatible_cuda_versions(status["versions"]) or not status["cuda"] or not status["bf16"]:
        raise ValueError("The pinned Python releases and a CUDA bfloat16 device are required")
    return python


def register(root, model, directory, executable, runtime):
    """Publish native registration only after exact assets and all worker artifacts are verified."""
    verify_assets(directory, model)
    executable = Path(os.path.abspath(executable))  # Preserve a venv launcher's symlink identity.
    if not executable.is_file():
        raise ValueError("worker executable is missing")
    artifacts = []
    if runtime == "mlx":
        for entry in executable.parent.iterdir():
            if entry.is_dir() and entry.suffix == ".bundle":
                artifacts.extend(artifact(p, executable.parent) for p in entry.rglob("*") if p.is_file())
            elif entry.is_file() and entry.suffix in {".metallib", ".dylib"}:
                artifacts.append(artifact(entry, executable.parent))
    config = dict(
        runtime=runtime,
        modelId=model["id"],
        revision=model["revision"],
        executable=str(executable),
        modelDirectory=str(directory.resolve()),
        executableSha256=artifact(executable, executable.parent)["sha256"],
        artifacts=artifacts,
    )
    root.mkdir(parents=True, exist_ok=True)
    candidate = root / (str(uuid.uuid4()) + ".partial")
    try:
        candidate.write_text(json.dumps(config, indent=2) + "\n")
        candidate.replace(root / "runtime.json")
    finally:
        candidate.unlink(missing_ok=True)


def main():
    """Downloads/builds run only through this deliberately invoked setup command, never app startup."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-data", type=Path, default=app_data())
    parser.add_argument("--variant", choices=["SFT", "Base"], default="SFT")
    parser.add_argument("--model-directory", type=Path)
    parser.add_argument("--download", action="store_true")
    parser.add_argument("--reuse-executable", type=Path)
    parser.add_argument("--reuse-python", type=Path)
    args = parser.parse_args()
    runtime = "mlx" if sys.platform == "darwin" and platform.machine() == "arm64" else "cuda"
    if runtime == "mlx" and args.reuse_executable is None:
        validate_mlx_host()
    model = selected_model(runtime, args.variant)
    root = args.app_data.resolve() / "moss-image"
    directory = args.model_directory or root / "models" / model["revision"]
    if not args.download:
        verify_assets(directory, model)
    executable = (
        (args.reuse_executable or build_mlx(root)) if runtime == "mlx" else cuda_python(root, args.reuse_python)
    )
    if runtime == "mlx" and args.reuse_executable:
        subprocess.run(
            [str(executable.resolve()), "--probe"],
            check=True,
            cwd=executable.resolve().parent,
            capture_output=True,
            timeout=60,
        )
    if args.download:
        download_assets(directory, model)
    register(root, model, directory, executable, runtime)
    print("MOSS-Image registered. Restart Bottie and choose Image > MOSS Image SFT/Base. No automatic cloud fallback.")


if __name__ == "__main__":
    main()
