"""Pinned Nano assets: offline verification plus explicit opt-in model acquisition."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import urllib.request

LOCK_PATH = Path(__file__).with_name("assets.lock.json")
READ_BYTES = 1024 * 1024
MAX_ASSET_BYTES = 512 * 1024 * 1024


def checked_path(root: Path, relative: str) -> Path:
    """Resolve a regular relative destination without traversal or symlink escapes."""
    path = PurePosixPath(relative)
    if not relative or "\\" in relative or path.is_absolute() or ".." in path.parts or str(path) != relative:
        raise ValueError("Unsafe asset destination")
    resolved = (root / relative).resolve()
    if not resolved.is_relative_to(root.resolve()) or resolved == root.resolve():
        raise ValueError("Asset destination escaped its cache")
    return resolved


def validate_inventory(entries: list[dict]) -> None:
    """Reject incomplete, duplicate, or unbounded asset identities before I/O."""
    seen = set()
    for entry in entries:
        name = entry["path"]
        checked_path(Path("/tmp/bottie-nano-inventory-check"), name)
        if name.casefold() in seen:
            raise ValueError("Duplicate asset destination")
        seen.add(name.casefold())
        size = entry["size"]
        if type(size) is not int or not 0 < size <= MAX_ASSET_BYTES:
            raise ValueError("Invalid asset size")
        if not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"]):
            raise ValueError("Invalid asset digest")
        if not entry["url"].startswith("https://"):
            raise ValueError("Asset download requires HTTPS")


def verify_file(path: Path, entry: dict) -> bool:
    """Require exact size and SHA-256 before permitting inference or cache reuse."""
    if not path.is_file() or path.stat().st_size != entry["size"]:
        return False
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest() == entry["sha256"]


def load_lock() -> dict:
    """Read the committed inventory; this operation never contacts a server."""
    lock = json.loads(LOCK_PATH.read_text())
    validate_inventory(lock["files"])
    return lock


def verify_assets(root: Path) -> dict:
    """Fail closed on absent or changed code, metadata, tokenizer, or weight bytes."""
    lock = load_lock()
    for entry in lock["files"]:
        if not verify_file(checked_path(root, entry["path"]), entry):
            raise ValueError(f"Missing or changed trial asset: {entry['path']}; run assets.py --download explicitly")
    return lock


def download_file(root: Path, entry: dict) -> None:
    """Stream a size-bounded download and atomically promote only verified bytes."""
    destination = checked_path(root, entry["path"])
    if verify_file(destination, entry):
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = checked_path(root, entry["path"] + ".partial")
    try:
        with urllib.request.urlopen(entry["url"], timeout=60) as response, temporary.open("wb") as stream:
            remaining = entry["size"]
            while chunk := response.read(min(READ_BYTES, remaining + 1)):
                remaining -= len(chunk)
                if remaining < 0:
                    raise ValueError("Download exceeds pinned size")
                stream.write(chunk)
        if not verify_file(temporary, entry):
            raise ValueError("Downloaded asset failed size/SHA-256 verification")
        temporary.replace(destination)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> None:
    """Keep acquisition an explicit opt-in, with offline verification as the default."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--download", action="store_true")
    args = parser.parse_args()
    lock = load_lock()
    if args.download:
        for entry in lock["files"]:
            print(f"Verifying/downloading {entry['path']}", flush=True)
            download_file(args.directory, entry)
    verify_assets(args.directory)
    print(json.dumps({"verifiedFiles": len(lock["files"]), "bytes": sum(f["size"] for f in lock["files"])}))


if __name__ == "__main__":
    main()
