#!/usr/bin/env bash
# Reproducible build of the audio.cpp C ABI (`libaudiocpp`) with only the model
# families Bottie would plausibly link, for the evaluation in
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

# OpenMP is upstream's `find_package(OpenMP REQUIRED)` whenever ENGINE_ENABLE_OPENMP
# is ON, and ON is upstream's default. Apple clang ships no OpenMP runtime, so on
# macOS that default aborts configure before anything compiles. Upstream's own
# scripts/build_metal.sh defaults --openmp OFF for the same reason, so this script
# follows it: OFF on Darwin, ON elsewhere, overridable.
#
# Homebrew's libomp does satisfy the REQUIRED — AUDIOCPP_SPIKE_OPENMP=ON with
# OpenMP_ROOT="$(brew --prefix libomp)" in the environment builds fine — but the
# result links /opt/homebrew/opt/libomp/lib/libomp.dylib by absolute path, which is
# not present on an end user's Mac. OFF is the only configuration a shipped macOS
# build could use without vendoring libomp.
if [ -n "${AUDIOCPP_SPIKE_OPENMP:-}" ]; then
    OPENMP="${AUDIOCPP_SPIKE_OPENMP}"
elif [ "$(uname -s)" = "Darwin" ]; then
    OPENMP=OFF
else
    OPENMP=ON
fi

# AUDIOCPP_STATIC_ESPEAK stays OFF. ON statically links GPL-3.0-or-later
# eSpeak-ng, which Bottie's dependency licence review does not permit.
# ENGINE_ENABLE_NATIVE_CPU stays OFF so the kernels are portable across the
# machines a released build is shipped to, not tuned to the builder's host.
#
# Nothing here pins a compute backend, so CMake enables whatever the host offers:
# on macOS that silently adds Metal and Accelerate, which a Linux CPU-only build
# does not have. The summary below reports what was actually enabled so two hosts
# are not compared as though they built the same thing.
cmake -S "${SOURCE_DIR}" -B "${BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DAUDIOCPP_VERSION="${VERSION}" \
    -DAUDIOCPP_BUILD_C_API=ON \
    -DAUDIOCPP_MODEL_SET=custom \
    -DAUDIOCPP_MODELS="${MODELS}" \
    -DENGINE_ENABLE_NATIVE_CPU=OFF \
    -DENGINE_ENABLE_OPENMP="${OPENMP}" \
    -DGGML_OPENMP="${OPENMP}" \
    -DAUDIOCPP_STATIC_ESPEAK=OFF \
    -DENGINE_BUILD_EXAMPLES=OFF \
    -DENGINE_BUILD_TESTS=OFF |
    tee "${BUILD_DIR}-configure.log"

started=$(date +%s)
cmake --build "${BUILD_DIR}" --target audiocpp -j"$(nproc 2>/dev/null || sysctl -n hw.logicalcpu)"
elapsed=$(( $(date +%s) - started ))

# `nm -D --defined-only` is GNU binutils. Apple's nm spells the same query
# `nm -gU` and prefixes C symbols with an underscore, so counting portably needs
# both spellings rather than one that silently degrades to "n/a" on macOS.
count_exported_audiocpp() {
    local lib="$1"
    if nm -D --defined-only "${lib}" >/dev/null 2>&1; then
        nm -D --defined-only "${lib}" 2>/dev/null | grep -c ' audiocpp_' || true
    elif nm -gU "${lib}" >/dev/null 2>&1; then
        nm -gU "${lib}" 2>/dev/null | grep -c ' _\{0,1\}audiocpp_' || true
    else
        echo 'n/a'
    fi
}

echo "audio.cpp commit:  $(git -C "${SOURCE_DIR}" rev-parse HEAD)"
echo "reported version:  ${VERSION}"
echo "linked models:     ${MODELS}"
echo "host:              $(uname -s) $(uname -m)"
echo "openmp:            ${OPENMP}"
echo "backends enabled:  $(grep -o 'Including [A-Za-z]* backend' "${BUILD_DIR}-configure.log" |
    sed 's/Including //; s/ backend//' | paste -sd, - || echo 'n/a')"
echo "build wall seconds: ${elapsed}"
# CMake puts the library under the build tree's bin/ on this project.
find "${BUILD_DIR}" -type f \( -name 'libaudiocpp.so.*' -o -name 'libaudiocpp.*.dylib' -o -name 'audiocpp.dll' \) |
    while read -r lib; do
        echo "library:           ${lib} ($(wc -c < "${lib}") bytes)"
        echo "exported symbols:  $(count_exported_audiocpp "${lib}")"
    done
