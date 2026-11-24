"""Installs the pinned ONNX Runtime prebuilt where ort-sys looks, so builds never stream it from the pyke CDN.

`plan` writes the archive's cache key and paths for the Actions cache. `install` verifies the archive against the
SHA-256 pinned in runtime-assets.json (downloading it only when it is missing or wrong, from this repository's mirror
release first), then unpacks it into ORT_CACHE_DIR/dfbin/<target>/<sha256>: the directory ort-sys treats as an
already verified download.
"""

import hashlib
import json
import lzma
import os
import platform
import re
import shutil
import sys
import tarfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TARGETS = {
    ("linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("darwin", "arm64"): "aarch64-apple-darwin",
    ("win32", "amd64"): "x86_64-pc-windows-msvc",
}
MIRROR = "https://github.com/hherb/bottie/releases/download/onnxruntime-{version}-pyke/"
# The Windows archive bundles DirectML.dll, whose licence does not clearly allow re-hosting it outside an app.
MIRRORED_TARGETS = {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}
# ort-sys decodes the archive as raw LZMA2 with a 64 MiB dictionary.
LZMA2_FILTERS = [{"id": lzma.FILTER_LZMA2, "dict_size": 1 << 26}]


def pinned_archive():
    runtime = json.loads((ROOT / "runtime-assets.json").read_text())["onnxRuntime"]
    # A different ort-sys pins a different archive hash; it would then ignore this directory and download silently.
    lock = (ROOT / "src-tauri" / "Cargo.lock").read_text()
    locked = re.search(r'name = "ort-sys"\nversion = "([^"]+)"', lock)
    if locked is None or runtime["selectedBy"] != f"ort-sys {locked.group(1)}":
        sys.exit(f"runtime-assets.json pins {runtime['selectedBy']}, but Cargo.lock has {locked and locked.group(1)}.")
    target = TARGETS.get((sys.platform, platform.machine().lower()))
    archive = next((a for a in runtime["archives"] if a["target"] == target), None)
    if archive is None:
        sys.exit(f"No pinned ONNX Runtime archive for {sys.platform}/{platform.machine()}.")
    archive["sources"] = [archive["url"]]
    if target in MIRRORED_TARGETS:
        mirror = MIRROR.format(version=runtime["version"]) + archive["url"].rsplit("/", 1)[1]
        archive["sources"].insert(0, mirror)
    return archive


def paths(archive):
    base = Path(os.environ["RUNNER_TEMP"])
    return base / "onnx-runtime-archive" / f"{archive['sha256']}.tar.lzma2", base / "ort.pyke.io"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def download(archive, destination):
    partial = destination.with_suffix(".partial")
    for attempt in range(1, 4):
        for url in archive["sources"]:
            try:
                with urllib.request.urlopen(url, timeout=60) as response, partial.open("wb") as file:
                    shutil.copyfileobj(response, file)
                if sha256(partial) == archive["sha256"]:
                    partial.replace(destination)
                    return
                print(f"Attempt {attempt}: {url} has the wrong SHA-256.", file=sys.stderr)
            except OSError as error:
                print(f"Attempt {attempt}: {url} failed: {error}", file=sys.stderr)
        time.sleep(10 * attempt)
    sys.exit(f"Could not fetch a verified ONNX Runtime archive from {archive['sources']}.")


def extract(archive_path, destination):
    staging = destination.with_name(destination.name + ".partial")
    shutil.rmtree(staging, ignore_errors=True)
    with archive_path.open("rb") as raw, lzma.open(raw, format=lzma.FORMAT_RAW, filters=LZMA2_FILTERS) as stream:
        with tarfile.open(fileobj=stream, mode="r|") as tar:
            for member in tar:
                # ort-sys keeps regular files only; the data filter rejects absolute or escaping paths.
                if member.isreg():
                    tar.extract(member, staging, filter="data")
    staging.replace(destination)


def main(command):
    archive = pinned_archive()
    archive_path, cache_dir = paths(archive)
    if command == "plan":
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"key=onnx-runtime-{archive['target']}-{archive['sha256']}\n")
            output.write(f"archive-dir={archive_path.parent}\n")
        with open(os.environ["GITHUB_ENV"], "a") as env:
            env.write(f"ORT_CACHE_DIR={cache_dir}\n")
        return
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    # A restored cache entry is checked like a download: only the pinned bytes are ever linked.
    if not archive_path.exists() or sha256(archive_path) != archive["sha256"]:
        archive_path.unlink(missing_ok=True)
        download(archive, archive_path)
    destination = cache_dir / "dfbin" / archive["target"] / archive["sha256"]
    if not destination.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        extract(archive_path, destination)
    print(f"ONNX Runtime {archive['target']} verified at {destination}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "install")
