"""Closed identities for Bottie's independently collected Linux runtime closures."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from diffusers_bundle_candidate import BASE_IMAGE, BASE_IMAGE_DIGEST
from diffusers_bundle_environment import (
    DERIVED_IMAGE_DIGEST,
    TARGET_ARCHITECTURE,
    TARGET_PYTHON_VERSION,
)
from diffusers_pytorch_worker import PYTORCH_WORKER_IDENTITY
from diffusers_worker import DIFFUSERS_WORKER_IDENTITY
from mlx_worker import WorkerIdentity


NGC_RUNTIME_PROFILE_NAME = "ngc-25.11"
CLEAN_RUNTIME_PROFILE_NAME = "pytorch-2.10-cu130-clean-rebuild"
NGC_LICENSE_SOURCE_COMPONENTS = frozenset(
    {
        "native:nvidia-nvpl-blas@0.2.0",
        "native:nvidia-nvpl-lapack@0.2.2",
        "python:sentencepiece@0.2.2",
        "python:tokenizers@0.23.2",
    }
)
CLEAN_LICENSE_SOURCE_COMPONENTS = frozenset(
    {
        "python:sentencepiece@0.2.2",
        "python:tokenizers@0.23.2",
    }
)
CLEAN_REBUILT_IMAGE_DIGEST = (
    "sha256:740816cb8f348aa26e3d32f73b15b86b7f5a7228cff7b24d6910ee7aa0ee12a4"
)
CLEAN_BASE_IMAGE = "ubuntu:24.04"
CLEAN_BASE_IMAGE_DIGEST = (
    "sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90"
)
CLEAN_TRACE_CONTEXT_SHA256S = frozenset(
    {
        "41c9859cafd3e9290febf6844e81a0bd995daf02292815149422eb1f7ffef757",
        "b0335f85ed46e7e81eba1d20de4c24e0228755e69dfa8848bea7207ab946404d",
    }
)


class ClosureProfileError(RuntimeError):
    """Stable failure raised for an unknown or incomplete closure profile."""


@dataclass(frozen=True)
class RuntimeClosureProfile:
    """Bind one closure route to exact image, runtime, trace, and ownership policy."""

    name: str
    base_image: str
    base_image_digest: str
    derived_image_digest: str
    identity: WorkerIdentity
    python_executable: str
    python_version: str
    target_architecture: str
    first_party_files: frozenset[Path]
    accepted_trace_context_sha256s: frozenset[str]
    use_ngc_native_components: bool
    allow_license_review: bool
    license_source_components: frozenset[str]
    require_all_license_sources: bool
    requires_clean_runtime_lock: bool


RUNTIME_CLOSURE_PROFILES = {
    NGC_RUNTIME_PROFILE_NAME: RuntimeClosureProfile(
        name=NGC_RUNTIME_PROFILE_NAME,
        base_image=BASE_IMAGE,
        base_image_digest=BASE_IMAGE_DIGEST,
        derived_image_digest=DERIVED_IMAGE_DIGEST,
        identity=DIFFUSERS_WORKER_IDENTITY,
        python_executable="python",
        python_version=TARGET_PYTHON_VERSION,
        target_architecture=TARGET_ARCHITECTURE,
        first_party_files=frozenset({Path("/opt/bottie/diffusers_worker.py")}),
        accepted_trace_context_sha256s=frozenset(),
        use_ngc_native_components=True,
        allow_license_review=True,
        license_source_components=NGC_LICENSE_SOURCE_COMPONENTS,
        require_all_license_sources=False,
        requires_clean_runtime_lock=False,
    ),
    CLEAN_RUNTIME_PROFILE_NAME: RuntimeClosureProfile(
        name=CLEAN_RUNTIME_PROFILE_NAME,
        base_image=CLEAN_BASE_IMAGE,
        base_image_digest=CLEAN_BASE_IMAGE_DIGEST,
        derived_image_digest=CLEAN_REBUILT_IMAGE_DIGEST,
        identity=PYTORCH_WORKER_IDENTITY,
        python_executable="/opt/bottie/venv/bin/python",
        python_version=TARGET_PYTHON_VERSION,
        target_architecture=TARGET_ARCHITECTURE,
        first_party_files=frozenset(
            {
                Path("/opt/bottie/diffusers_pytorch_worker.py"),
                Path("/opt/bottie/diffusers_worker.py"),
                Path("/opt/bottie/mlx_worker.py"),
            }
        ),
        accepted_trace_context_sha256s=CLEAN_TRACE_CONTEXT_SHA256S,
        use_ngc_native_components=False,
        allow_license_review=False,
        license_source_components=CLEAN_LICENSE_SOURCE_COMPONENTS,
        require_all_license_sources=True,
        requires_clean_runtime_lock=True,
    ),
}
DEFAULT_RUNTIME_CLOSURE_PROFILE = RUNTIME_CLOSURE_PROFILES[NGC_RUNTIME_PROFILE_NAME]


def runtime_closure_profile(name: str) -> RuntimeClosureProfile:
    """Return one closed profile without accepting caller-defined runtime identities."""
    try:
        return RUNTIME_CLOSURE_PROFILES[name]
    except (KeyError, TypeError) as error:
        raise ClosureProfileError("runtime closure profile is unsupported") from error
