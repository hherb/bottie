"""Dependency-free contracts shared by Nano's offline native speech worker and trial."""

import base64
import math
import struct
from typing import Callable

MAX_TEXT_BYTES = 2000
MAX_REQUEST_BYTES = 16 * 1024
MAX_EVENT_BYTES = 600 * 1024
MAX_SEED = 2**32 - 1
MAX_TEXT_TOKENS = 75
MAX_NEW_FRAMES = 375
SAMPLE_RATE = 48000
CHANNELS = 2
SAMPLE_BYTES = 4
MAX_PCM_BYTES = SAMPLE_RATE * CHANNELS * SAMPLE_BYTES


def validate_request(request: dict, voices: set[str]) -> dict:
    """Accept only bounded local preset synthesis; paths and reference audio are excluded."""
    if not isinstance(request, dict) or set(request) != {"text", "voice", "seed"}:
        raise ValueError("Expected exactly text, voice, and seed")
    text = request["text"]
    if not isinstance(text, str) or not text.strip() or len(text.encode("utf-8")) > MAX_TEXT_BYTES:
        raise ValueError("Invalid text length")
    if not isinstance(request["voice"], str) or request["voice"] not in voices:
        raise ValueError("Unknown voice preset")
    if type(request["seed"]) is not int or not 0 <= request["seed"] <= MAX_SEED:
        raise ValueError("Invalid seed")
    return request


def split_text(text: str, token_count: Callable[[str], int], budget: int = MAX_TEXT_TOKENS) -> list[str]:
    """Split at sentence/word boundaries while preserving text and limiting encoded tokens."""
    chunks = []
    remaining = text.strip()
    while remaining:
        if token_count(remaining) <= budget:
            chunks.append(remaining)
            break
        end = 0
        for index in range(1, len(remaining) + 1):
            if token_count(remaining[:index]) > budget:
                break
            end = index
        if not end:
            raise ValueError("A single character exceeds the text token budget")
        prefix = remaining[:end]
        sentence_end = max((i + 1 for i, char in enumerate(prefix) if char in ".!?。！？"), default=0)
        word_end = max((i + 1 for i, char in enumerate(prefix) if char.isspace()), default=0)
        end = sentence_end or word_end or end
        chunk = remaining[:end].strip()
        if chunk:
            chunks.append(chunk)
        remaining = remaining[end:].lstrip()
    return chunks


def pcm_event(payload: bytes, sequence: int) -> dict:
    """Frame one finite, interleaved little-endian float32 stereo PCM chunk."""
    if not payload or len(payload) > MAX_PCM_BYTES or len(payload) % (CHANNELS * SAMPLE_BYTES):
        raise ValueError("Invalid PCM length")
    if any(not math.isfinite(value[0]) for value in struct.iter_unpack("<f", payload)):
        raise ValueError("Nonfinite PCM sample")
    return {
        "event": "audio",
        "sequence": sequence,
        "sampleRate": SAMPLE_RATE,
        "channels": CHANNELS,
        "samplesPerChannel": len(payload) // (CHANNELS * SAMPLE_BYTES),
        "pcm": base64.b64encode(payload).decode("ascii"),
    }


def completion_status(frame_count: int, frame_limit: int) -> str:
    """Distinguish model termination from empty output and hard generation truncation."""
    if not frame_count:
        return "empty"
    return "frame_limit" if frame_count >= frame_limit else "complete"
