"""Closed runtime profiles for Bottie's Linux Diffusers proof harness."""

from __future__ import annotations

from dataclasses import dataclass

from diffusers_pytorch_worker import PYTORCH_WORKER_IDENTITY
from diffusers_worker import DIFFUSERS_WORKER_IDENTITY
from mlx_worker import WorkerIdentity

UCC_POLICY_UNRESTRICTED = "unrestricted"
UCC_POLICY_ABLATE = "ablate"
UCC_POLICY_ABSENT = "absent"
UCC_POLICIES = frozenset(
    {UCC_POLICY_UNRESTRICTED, UCC_POLICY_ABLATE, UCC_POLICY_ABSENT}
)


@dataclass(frozen=True)
class RuntimeProfile:
    """Bind one proof selection to its identity, interpreter, worker, and UCC policy."""

    identity: WorkerIdentity
    worker_script: str
    python_executable: str
    ucc_policy: str

    def __post_init__(self) -> None:
        """Reject incomplete or open-ended runtime execution profiles."""
        if (
            not self.worker_script.startswith("/")
            or not self.python_executable
            or self.ucc_policy not in UCC_POLICIES
        ):
            raise ValueError("runtime proof profile is invalid")


RUNTIME_PROFILES = {
    "ngc-25.11": RuntimeProfile(
        DIFFUSERS_WORKER_IDENTITY,
        "/opt/bottie/diffusers_worker.py",
        "python",
        UCC_POLICY_UNRESTRICTED,
    ),
    "pytorch-2.10-cu130": RuntimeProfile(
        PYTORCH_WORKER_IDENTITY,
        "/opt/bottie/diffusers_pytorch_worker.py",
        "python",
        UCC_POLICY_ABLATE,
    ),
    "pytorch-2.10-cu130-clean": RuntimeProfile(
        PYTORCH_WORKER_IDENTITY,
        "/opt/bottie/diffusers_pytorch_worker.py",
        "/opt/bottie/venv/bin/python",
        UCC_POLICY_ABSENT,
    ),
}
DEFAULT_RUNTIME_PROFILE = RUNTIME_PROFILES["ngc-25.11"]
