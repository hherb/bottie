#!/usr/bin/env bash
# Reproducible CPU-only build of the audio.cpp C ABI (`libaudiocpp`) with only the
# model families Bottie would plausibly link, for the evaluation in
# `docs/audio-cpp-evaluation.md`.
#
# This builds an external project. It is spike tooling and is not part of Bottie's
# release build, packaging, or CI.
#
# Usage: build-libaudiocpp.sh <audio.cpp checkout> [build dir]
set -euo pipefail

SOURCE_DIR="${1:?usage: build-libaudiocpp.sh <audio.cpp checkout> [build dir]}"
BUILD_DIR="${2:-${SOURCE_DIR}/build-spike}"

# Only the two families Bottie would use: Supertonic 3 for text-to-speech and
# Nemotron 3.5 for streaming speech recognition. `full` links all 80+ families.
MODELS="${AUDIOCPP_SPIKE_MODELS:-supertonic,nemotron_asr}"

# AUDIOCPP_VERSION defaults to the literal "dev", which is what
# audiocpp_build_version() then reports at runtime. Derive it from the checkout so
# a build is identifiable; Bottie would pin a released tag rather than a branch.
VERSION="${AUDIOCPP_SPIKE_VERSION:-$(git -C "${SOURCE_DIR}" describe --tags --always 2>/dev/null || echo unknown)}"

# AUDIOCPP_STATIC_ESPEAK stays OFF. ON statically links GPL-3.0-or-later
# eSpeak-ng, which Bottie's dependency licence review does not permit.
# ENGINE_ENABLE_NATIVE_CPU stays OFF so the kernels are portable across the
# machines a released build is shipped to, not tuned to the builder's host.
cmake -S "${SOURCE_DIR}" -B "${BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DAUDIOCPP_VERSION="${VERSION}" \
    -DAUDIOCPP_BUILD_C_API=ON \
    -DAUDIOCPP_MODEL_SET=custom \
    -DAUDIOCPP_MODELS="${MODELS}" \
    -DENGINE_ENABLE_NATIVE_CPU=OFF \
    -DAUDIOCPP_STATIC_ESPEAK=OFF \
    -DENGINE_BUILD_EXAMPLES=OFF \
    -DENGINE_BUILD_TESTS=OFF

started=$(date +%s)
cmake --build "${BUILD_DIR}" --target audiocpp -j"$(nproc 2>/dev/null || sysctl -n hw.logicalcpu)"
elapsed=$(( $(date +%s) - started ))

echo "audio.cpp commit:  $(git -C "${SOURCE_DIR}" rev-parse HEAD)"
echo "reported version:  ${VERSION}"
echo "linked models:     ${MODELS}"
echo "build wall seconds: ${elapsed}"
# CMake puts the library under the build tree's bin/ on this project.
find "${BUILD_DIR}" -type f \( -name 'libaudiocpp.so.*' -o -name 'libaudiocpp.*.dylib' -o -name 'audiocpp.dll' \) |
    while read -r lib; do
        echo "library:           ${lib} ($(wc -c < "${lib}") bytes)"
        echo "exported symbols:  $(nm -D --defined-only "${lib}" 2>/dev/null | grep -c ' audiocpp_' || echo 'n/a')"
    done
