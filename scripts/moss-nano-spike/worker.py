"""Offline private-stdio Nano inference shared by the trial and Bottie native speech backend."""

import argparse
import json
from pathlib import Path

try:
    import resource
except ImportError:  # Windows does not expose resource; native playback does not depend on RSS metrics.
    resource = None
import sys
import time

from assets import verify_assets
from contract import (
    CHANNELS,
    MAX_NEW_FRAMES,
    MAX_REQUEST_BYTES,
    MAX_TEXT_TOKENS,
    SAMPLE_RATE,
    completion_status,
    pcm_event,
    split_text,
    validate_request,
)


def emit(event: dict) -> None:
    """Publish a flushed protocol event, keeping stdout free of progress logging."""
    print(json.dumps(event, separators=(",", ":")), flush=True)


def peak_rss_bytes() -> int:
    """Normalize process-lifetime peak RSS across macOS and Linux resource conventions."""
    if resource is None:
        return 0
    value = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return int(value if sys.platform == "darwin" else value * 1024)


def deny_network(event: str, _arguments: tuple) -> None:
    """Make accidental inference-time network access fail visibly rather than download."""
    if event in {"socket.connect", "socket.getaddrinfo", "socket.bind"}:
        raise PermissionError("Nano inference is offline")


class NanoWorker:
    """Reuse pinned CPU sessions while resetting sampling and codec state per utterance."""

    def __init__(self, directory: Path, threads: int):
        """Verify all assets before importing the exact pinned upstream ONNX core."""
        started = time.perf_counter()
        lock = verify_assets(directory)
        sys.path.insert(0, str((directory / "runtime").resolve()))
        import numpy as np
        import onnxruntime
        import sentencepiece
        from ort_cpu_runtime import OrtCpuRuntime

        onnxruntime.disable_telemetry_events()
        sys.addaudithook(deny_network)
        self.np = np
        self.runtime = OrtCpuRuntime(
            directory,
            thread_count=threads,
            max_new_frames=MAX_NEW_FRAMES,
            sample_mode="fixed",
            execution_provider="cpu",
        )
        tokenizer = self.runtime.resolve_manifest_relative_path(self.runtime.manifest["model_files"]["tokenizer_model"])
        self.tokenizer = sentencepiece.SentencePieceProcessor(model_file=str(tokenizer))
        self.voices = {voice["voice"]: voice for voice in self.runtime.list_builtin_voices()}
        config = self.runtime.codec_meta["codec_config"]
        if config["sample_rate"] != SAMPLE_RATE or config["channels"] != CHANNELS:
            raise ValueError("Unsupported codec format")
        emit(
            {
                "event": "ready",
                "protocolVersion": 1,
                "loadSeconds": time.perf_counter() - started,
                "peakRssBytes": peak_rss_bytes(),
                "sourceRevision": lock["sourceRevision"],
                "modelRevision": lock["modelRevision"],
                "codecRevision": lock["codecRevision"],
                "voices": sorted(self.voices),
                "threads": threads,
                "numpy": np.__version__,
                "onnxruntime": onnxruntime.__version__,
                "sentencepiece": sentencepiece.__version__,
                "torchImported": "torch" in sys.modules,
                "telemetryDisabled": True,
                "executionProviders": sorted(
                    {provider for session in self.runtime.sessions.values() for provider in session.get_providers()}
                ),
            }
        )

    def synthesize(self, request: dict) -> None:
        """Emit PCM as soon as decoded; report truncation instead of a false completion."""
        validate_request(request, set(self.voices))
        self.runtime.rng = self.np.random.default_rng(request["seed"])
        started = time.perf_counter()

        def token_count(text: str) -> int:
            """Count the exact tokenizer output used by this pinned model."""
            return len(self.tokenizer.encode(text, out_type=int))

        chunks = split_text(request["text"], token_count, MAX_TEXT_TOKENS)
        sequence = 0
        sample_count = 0
        first_pcm_seconds = None
        frame_count = 0
        for chunk in chunks:
            rows = self.runtime.build_voice_clone_request_rows(
                self.voices[request["voice"]]["prompt_audio_codes"], self.tokenizer.encode(chunk, out_type=int)
            )
            self.runtime.codec_streaming_session.reset()

            def on_frame(_frames: list, _index: int, frame: list) -> None:
                """Decode each 80 ms frame and immediately expose its interleaved PCM."""
                nonlocal sequence, sample_count, first_pcm_seconds
                decoded = self.runtime.codec_streaming_session.run_frames([frame])
                if decoded is None:
                    return
                audio, length = decoded
                if length <= 0:
                    return
                pcm = audio[0, :, :length].T.astype("<f4", copy=False).tobytes()
                event = pcm_event(pcm, sequence)
                if first_pcm_seconds is None:
                    first_pcm_seconds = time.perf_counter() - started
                sample_count += event["samplesPerChannel"]
                sequence += 1
                emit(event)

            try:
                frames = self.runtime.generate_audio_frames(rows, on_frame=on_frame)
            finally:
                self.runtime.codec_streaming_session.reset()
            frame_count += len(frames)
            status = completion_status(len(frames), MAX_NEW_FRAMES)
            if status != "complete":
                emit({"event": "failed", "reason": status})
                return
        if not sample_count:
            emit({"event": "failed", "reason": "empty_pcm"})
            return
        elapsed = time.perf_counter() - started
        duration = sample_count / SAMPLE_RATE
        emit(
            {
                "event": "complete",
                "firstPcmSeconds": first_pcm_seconds,
                "generationSeconds": elapsed,
                "audioSeconds": duration,
                "realTimeFactor": elapsed / duration,
                "frames": frame_count,
                "audioChunks": sequence,
                "textChunks": len(chunks),
                "peakRssBytes": peak_rss_bytes(),
            }
        )


def main() -> None:
    """Read bounded serial requests; EOF or process termination ends this trial worker."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--threads", type=int, choices=range(1, 9), default=4)
    args = parser.parse_args()
    try:
        worker = NanoWorker(args.directory, args.threads)
        while line := sys.stdin.buffer.readline(MAX_REQUEST_BYTES + 1):
            if len(line) > MAX_REQUEST_BYTES or not line.endswith(b"\n"):
                raise ValueError("Invalid request framing")
            worker.synthesize(json.loads(line))
    except Exception as error:
        emit({"event": "failed", "reason": type(error).__name__})
        print(str(error), file=sys.stderr, flush=True)
        raise SystemExit(1) from None


if __name__ == "__main__":
    main()
