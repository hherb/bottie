"""Measure a real offline Nano worker and retain synthetic audition WAVs outside the repository."""

import argparse
from array import array
import base64
import hashlib
import json
from pathlib import Path
import platform
import struct
import sys
import time
import wave

from client import WorkerClient
from contract import CHANNELS, SAMPLE_BYTES, SAMPLE_RATE, pcm_event

STARTUP_TIMEOUT_SECONDS = 120
GENERATION_TIMEOUT_SECONDS = 300
MAX_AUDIO_SECONDS = 120
PCM16_SAMPLE_BYTES = 2
PCM16_MAX = 32767
SCRIPT_DIR = Path(__file__).resolve().parent


def start_worker(directory: Path, output: Path, threads: int, label: str) -> tuple[WorkerClient, dict]:
    """Wait for a fresh child to verify/load its assets and advertise a CPU-only runtime."""
    started = time.perf_counter()
    child = WorkerClient(
        [sys.executable, str(SCRIPT_DIR / "worker.py"), "--directory", str(directory), "--threads", str(threads)],
        output / f"{label}-stderr.log",
    )
    try:
        ready = child.read_event(STARTUP_TIMEOUT_SECONDS)
        if (
            ready.get("event") != "ready"
            or ready.get("protocolVersion") != 1
            or ready.get("torchImported") is not False
            or ready.get("executionProviders") != ["CPUExecutionProvider"]
        ):
            raise ValueError(f"Unexpected worker startup: {ready}")
        ready["readySeconds"] = time.perf_counter() - started
        return child, ready
    except BaseException:
        child.stop()
        raise


def decode_event(event: dict, sequence: int) -> bytes:
    """Validate ordering, format, exact sample counts, and finite PCM in each received frame."""
    payload = base64.b64decode(event["pcm"], validate=True)
    if event != pcm_event(payload, sequence):
        raise ValueError("Invalid audio event metadata or sequence")
    return payload


def write_wav(destination: Path, pcm: bytes) -> dict:
    """Write a PCM16 audition copy and retain measurable level/clipping evidence."""
    samples = array("h")
    peak = 0.0
    energy = 0.0
    clipped = 0
    for (value,) in struct.iter_unpack("<f", pcm):
        peak = max(peak, abs(value))
        energy += value * value
        clipped += int(abs(value) > 1.0)
        samples.append(round(max(-1.0, min(1.0, value)) * PCM16_MAX))
    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(destination), "wb") as stream:
        stream.setnchannels(CHANNELS)
        stream.setsampwidth(PCM16_SAMPLE_BYTES)
        stream.setframerate(SAMPLE_RATE)
        stream.writeframes(samples.tobytes())
    return {
        "wav": destination.name,
        "wavSha256": hashlib.sha256(destination.read_bytes()).hexdigest(),
        "peak": peak,
        "rms": (energy / len(samples)) ** 0.5,
        "clippedSamples": clipped,
    }


def run_case(child: WorkerClient, case: dict, output: Path) -> dict:
    """Require a terminal completion before saving audio; never accept a partial recording as success."""
    started = time.perf_counter()
    child.send({key: case[key] for key in ("text", "voice", "seed")})
    pcm = bytearray()
    sequence = 0
    first_received = None
    while True:
        event = child.read_event(GENERATION_TIMEOUT_SECONDS - (time.perf_counter() - started))
        if event["event"] == "audio":
            payload = decode_event(event, sequence)
            if first_received is None:
                first_received = time.perf_counter() - started
            sequence += 1
            pcm.extend(payload)
            if len(pcm) > MAX_AUDIO_SECONDS * SAMPLE_RATE * CHANNELS * SAMPLE_BYTES:
                raise ValueError("Utterance exceeds audio budget")
        elif event["event"] == "complete":
            duration = len(pcm) / (SAMPLE_RATE * CHANNELS * SAMPLE_BYTES)
            if not pcm or event["audioChunks"] != sequence or abs(event["audioSeconds"] - duration) > 1e-6:
                raise ValueError("Inconsistent completion")
            return {
                **case,
                **event,
                "firstReceivedSeconds": first_received,
                **write_wav(output / (case["id"] + ".wav"), pcm),
            }
        else:
            raise RuntimeError(f"Trial failed: {event}")


def cancel_after_audio(child: WorkerClient, case: dict) -> dict:
    """Cancel an actual live generation after PCM arrives and synchronously reap its process."""
    child.send({key: case[key] for key in ("text", "voice", "seed")})
    event = child.read_event(GENERATION_TIMEOUT_SECONDS)
    if event.get("event") != "audio":
        raise ValueError("Cancellation fixture did not receive live PCM")
    decode_event(event, 0)
    elapsed = child.stop()
    return {
        "afterFirstPcm": True,
        "stopSeconds": elapsed,
        "reaped": child.process.returncode is not None,
        "exitCode": child.process.returncode,
        "audioRetained": False,
    }


def main() -> None:
    """Run cold/warm synthesis, active cancellation, and a fresh-worker recovery check."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--threads", type=int, choices=range(1, 9), default=4)
    parser.add_argument(
        "--quick", action="store_true", help="Only the short English/German cases plus cancellation/recovery"
    )
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    cases = json.loads((SCRIPT_DIR / "cases.json").read_text())
    if args.quick:
        cases = cases[:2]
    report = {
        "formatVersion": 1,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "threads": args.threads,
        "cases": [],
        "auditionStatus": "not_listened",
        "applicationIntegrated": False,
    }
    child, report["startup"] = start_worker(args.directory, args.output, args.threads, "trial")
    try:
        for index, case in enumerate(cases):
            result = run_case(child, case, args.output)
            result["coldGeneration"] = index == 0
            report["cases"].append(result)
            print(
                json.dumps(
                    {
                        "id": case["id"],
                        "firstPcmSeconds": result["firstPcmSeconds"],
                        "realTimeFactor": result["realTimeFactor"],
                    }
                ),
                flush=True,
            )
        report["cancellation"] = cancel_after_audio(child, cases[-1])
    finally:
        child.stop()
    child, report["recoveryStartup"] = start_worker(args.directory, args.output, args.threads, "recovery")
    try:
        case = {**cases[0], "id": "english-recovery"}
        report["recovery"] = run_case(child, case, args.output)
    finally:
        child.stop()
    report_path = args.output / "measurements.json"
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(str(report_path), flush=True)


if __name__ == "__main__":
    main()
