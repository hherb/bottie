"""Immutable MOSS snapshots with explicit, resumable, size/hash-verified acquisition."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import urllib.request

LOCK = Path(__file__).with_name("models.lock.json")
READ_BYTES = 4 * 1024 * 1024
DOWNLOAD_WORKERS = 8


def checked_path(root, relative):
    """Inventories may name regular descendants only, with no traversal or symlink escape."""
    name = PurePosixPath(relative)
    if not relative or "\\" in relative or name.is_absolute() or ".." in name.parts or str(name) != relative:
        raise ValueError("unsafe model path")
    path = root / relative
    if not path.resolve().is_relative_to(root.resolve()) or path.resolve() == root.resolve():
        raise ValueError("model path escapes its registered directory")
    return path


def selected_model(runtime, variant):
    """Use a closed compiled inventory, never a mutable Hub alias supplied by a prompt."""
    suffix = f"MOSS-Image-{variant}" + ("-bf16" if runtime == "mlx" else "")
    return next(
        m for m in json.loads(LOCK.read_text())["models"] if m["runtime"] == runtime and m["id"].endswith(suffix)
    )


def verify_file(path, entry):
    """Hash streaming files so model verification never allocates a safetensors-sized buffer."""
    if not path.is_file() or path.stat().st_size != entry["size"]:
        return False
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest() == entry["sha256"]


def verify_assets(directory, model):
    """A partially downloaded or altered model cannot be registered or executed."""
    for entry in model["files"]:
        if not verify_file(checked_path(directory, entry["path"]), entry):
            raise ValueError("missing or changed model asset: " + entry["path"])


def download_file(directory, model, entry):
    """Resume exact revision bytes and atomically promote only the verified complete file."""
    destination = checked_path(directory, entry["path"])
    if verify_file(destination, entry):
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = checked_path(directory, entry["path"] + ".partial")
    url = f"https://huggingface.co/{model['id']}/resolve/{model['revision']}/{entry['path']}?download=true"
    for attempt in range(3):
        start = temporary.stat().st_size if temporary.exists() else 0
        if start >= entry["size"]:
            if verify_file(temporary, entry):
                temporary.replace(destination)
                return
            temporary.unlink()
            start = 0
        try:
            request = urllib.request.Request(url, headers={"Range": f"bytes={start}-"} if start else {})
            with urllib.request.urlopen(request, timeout=120) as response:
                if start and response.status != 206:
                    start = 0
                if start and not response.headers.get("Content-Range", "").startswith(f"bytes {start}-"):
                    raise ValueError("incorrect resume range")
                with temporary.open("ab" if start else "wb") as stream:
                    remaining = entry["size"] - start
                    while chunk := response.read(min(READ_BYTES, remaining + 1)):
                        remaining -= len(chunk)
                        if remaining < 0:
                            raise ValueError("asset exceeds pinned byte size")
                        stream.write(chunk)
            if not verify_file(temporary, entry):
                temporary.unlink(missing_ok=True)
                raise ValueError("asset checksum mismatch")
            temporary.replace(destination)
            print("Verified " + entry["path"], flush=True)
            return
        except (OSError, ValueError):
            if attempt == 2:
                raise


def download_assets(directory, model):
    """Limit concurrency to eight files and report the immutable download size before acquisition."""
    print(f"{model['id']}@{model['revision']}: {sum(f['size'] for f in model['files']):,} bytes", flush=True)
    with ThreadPoolExecutor(max_workers=DOWNLOAD_WORKERS) as executor:
        list(executor.map(lambda entry: download_file(directory, model, entry), model["files"]))
    verify_assets(directory, model)


def main():
    """Offline verification is the default; network acquisition requires --download."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument(
        "--runtime", choices=["mlx", "cuda"], default="mlx" if platform.system() == "Darwin" else "cuda"
    )
    parser.add_argument("--variant", choices=["SFT", "Base"], default="SFT")
    parser.add_argument("--download", action="store_true")
    args = parser.parse_args()
    model = selected_model(args.runtime, args.variant)
    if args.download:
        download_assets(args.directory, model)
    else:
        verify_assets(args.directory, model)


if __name__ == "__main__":
    main()
