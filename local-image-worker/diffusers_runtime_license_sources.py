"""Bind exact external source archives to runtime-closure licence evidence."""

from __future__ import annotations

import copy
import hashlib
import io
import os
import re
import stat
import tarfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


MAX_ARCHIVE_BYTES = 16 * 1024 * 1024
MAX_LICENSE_BYTES = 1024 * 1024
MAX_RUNTIME_MEMBER_BYTES = 64 * 1024 * 1024
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")


class ExternalLicenseSourceError(RuntimeError):
    """Stable failure raised when authoritative external source bytes have drifted."""


@dataclass(frozen=True)
class RuntimeArchiveMemberSpec:
    """Bind one installed runtime file to its exact regular source-archive member."""

    runtime_path: Path
    member_name: str


@dataclass(frozen=True)
class ExternalLicenseSourceSpec:
    """Describe one fixed authoritative archive and its exact licence member."""

    component_identity: str
    archive_name: str
    archive_byte_size: int
    archive_sha256: str
    source_url: str
    member_name: str
    evidence_relative_name: str
    byte_size: int
    sha256: str
    runtime_members: tuple[RuntimeArchiveMemberSpec, ...] = ()


EXTERNAL_LICENSE_SOURCE_SPECS = (
    ExternalLicenseSourceSpec(
        component_identity="native:nvidia-nvpl-blas@0.2.0",
        archive_name="nvpl_blas-linux-sbsa-0.2.0.1-archive.tar.xz",
        archive_byte_size=752_668,
        archive_sha256="ba29f6a9d3831b6ae5c9265b4d124c13b9b9e0faea025359b02b41ad230975c2",
        source_url=(
            "https://developer.download.nvidia.com/compute/nvpl/redist/nvpl_blas/linux-sbsa/"
            "nvpl_blas-linux-sbsa-0.2.0.1-archive.tar.xz"
        ),
        member_name="nvpl_blas-linux-sbsa-0.2.0.1-archive/LICENSE",
        evidence_relative_name="nvidia/nvpl-blas-0.2.0.1/LICENSE",
        byte_size=19_072,
        sha256="d81174652f0c448a5736afc5d50663606863bfd6ee8c8416fbd9a628c6f8802f",
        runtime_members=(
            RuntimeArchiveMemberSpec(
                runtime_path=Path("/usr/local/lib/libnvpl_blas_core.so.0.2.0"),
                member_name=(
                    "nvpl_blas-linux-sbsa-0.2.0.1-archive/lib/"
                    "libnvpl_blas_core.so.0.2.0"
                ),
            ),
            RuntimeArchiveMemberSpec(
                runtime_path=Path("/usr/local/lib/libnvpl_blas_lp64_gomp.so.0.2.0"),
                member_name=(
                    "nvpl_blas-linux-sbsa-0.2.0.1-archive/lib/"
                    "libnvpl_blas_lp64_gomp.so.0.2.0"
                ),
            ),
        ),
    ),
    ExternalLicenseSourceSpec(
        component_identity="native:nvidia-nvpl-lapack@0.2.2",
        archive_name="nvpl_lapack-linux-sbsa-0.2.2.1-archive.tar.xz",
        archive_byte_size=3_945_436,
        archive_sha256="cdfbf69517a044e99e3e6231c8b2f4e845fd0de57775ccad6b4b0b4fe7e91e84",
        source_url=(
            "https://developer.download.nvidia.com/compute/nvpl/redist/nvpl_lapack/linux-sbsa/"
            "nvpl_lapack-linux-sbsa-0.2.2.1-archive.tar.xz"
        ),
        member_name="nvpl_lapack-linux-sbsa-0.2.2.1-archive/LICENSE",
        evidence_relative_name="nvidia/nvpl-lapack-0.2.2.1/LICENSE",
        byte_size=19_072,
        sha256="d81174652f0c448a5736afc5d50663606863bfd6ee8c8416fbd9a628c6f8802f",
        runtime_members=(
            RuntimeArchiveMemberSpec(
                runtime_path=Path("/usr/local/lib/libnvpl_lapack_core.so.0.2.2"),
                member_name=(
                    "nvpl_lapack-linux-sbsa-0.2.2.1-archive/lib/"
                    "libnvpl_lapack_core.so.0.2.2"
                ),
            ),
            RuntimeArchiveMemberSpec(
                runtime_path=Path("/usr/local/lib/libnvpl_lapack_lp64_gomp.so.0.2.2"),
                member_name=(
                    "nvpl_lapack-linux-sbsa-0.2.2.1-archive/lib/"
                    "libnvpl_lapack_lp64_gomp.so.0.2.2"
                ),
            ),
        ),
    ),
    ExternalLicenseSourceSpec(
        component_identity="python:sentencepiece@0.2.2",
        archive_name="sentencepiece-0.2.2.tar.gz",
        archive_byte_size=8_218_435,
        archive_sha256="3d2b5e824b5622038dc7b490897efe05ebbbb9e7350fc142f3ecc8789ef9bdf6",
        source_url=(
            "https://files.pythonhosted.org/packages/cc/33/"
            "ea3cb3839607eb175da835244a798f797f478c5ddf0e8ecdf57ea85a4c70/"
            "sentencepiece-0.2.2.tar.gz"
        ),
        member_name="sentencepiece-0.2.2/sentencepiece/LICENSE",
        evidence_relative_name="pypi/sentencepiece-0.2.2/LICENSE",
        byte_size=11_358,
        sha256="cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
    ),
    ExternalLicenseSourceSpec(
        component_identity="python:tokenizers@0.23.2",
        archive_name="tokenizers-0.23.2.tar.gz",
        archive_byte_size=385_745,
        archive_sha256="7f0f085686b9de0d0079e6f874ae053600db64c5d13049e0bbc0119926d25aac",
        source_url=(
            "https://files.pythonhosted.org/packages/18/1e/"
            "bc6587c5ab643b2e17776cace9070a2ae73549c86bffac9934a600bf3c31/"
            "tokenizers-0.23.2.tar.gz"
        ),
        member_name="tokenizers-0.23.2/tokenizers/LICENSE",
        evidence_relative_name="pypi/tokenizers-0.23.2/LICENSE",
        byte_size=11_357,
        sha256="c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
    ),
)


def license_source_specs_for_components(
    component_identities: frozenset[str],
) -> tuple[ExternalLicenseSourceSpec, ...]:
    """Resolve one closed profile allowlist to its exact source catalog entries."""
    selected = tuple(
        spec
        for spec in EXTERNAL_LICENSE_SOURCE_SPECS
        if spec.component_identity in component_identities
    )
    if (
        not component_identities
        or len(selected) != len(component_identities)
        or {spec.component_identity for spec in selected} != component_identities
    ):
        raise ExternalLicenseSourceError(
            "external licence source profile allowlist is invalid"
        )
    return selected


def verified_external_license_sources(
    source_root: Path,
    specs: tuple[ExternalLicenseSourceSpec, ...] = EXTERNAL_LICENSE_SOURCE_SPECS,
    *,
    require_all: bool = False,
) -> dict[str, dict]:
    """Return path-free evidence for every present recognized authoritative archive."""
    if source_root.is_symlink():
        raise ExternalLicenseSourceError("external licence source root is invalid")
    try:
        root = source_root.resolve(strict=True)
    except OSError as error:
        raise ExternalLicenseSourceError(
            "external licence source root is unavailable"
        ) from error
    if not root.is_dir():
        raise ExternalLicenseSourceError("external licence source root is invalid")
    sources = {}
    for spec in specs:
        _validate_spec(spec)
        archive = root / spec.archive_name
        if archive.is_symlink():
            raise ExternalLicenseSourceError(
                "external licence source archive is invalid"
            )
        if not archive.exists():
            continue
        if spec.component_identity in sources:
            raise ExternalLicenseSourceError(
                "external licence source component is duplicated"
            )
        sources[spec.component_identity] = _verified_source(archive, spec)
    if require_all and len(sources) != len(specs):
        raise ExternalLicenseSourceError(
            "external licence source profile evidence is incomplete"
        )
    if not sources:
        raise ExternalLicenseSourceError(
            "external licence source root has no recognized archives"
        )
    return sources


def apply_external_license_sources(
    components: dict[str, dict], sources: dict[str, dict]
) -> dict[str, dict]:
    """Copy component records and add only exact source bytes, never licence expressions."""
    enriched = copy.deepcopy(components)
    for identity, source in sources.items():
        component = enriched.get(identity)
        if component is None:
            raise ExternalLicenseSourceError(
                "external licence source component is absent"
            )
        if component.get("licenseFiles"):
            raise ExternalLicenseSourceError(
                "external licence source component already has licence bytes"
            )
        component["licenseFiles"] = copy.deepcopy(source["licenseFiles"])
        provenance = component.setdefault("provenance", {})
        if "licenseSource" in provenance:
            raise ExternalLicenseSourceError(
                "external licence source provenance conflicts"
            )
        provenance["licenseSource"] = copy.deepcopy(source["provenance"])
    return enriched


def _verified_source(archive: Path, spec: ExternalLicenseSourceSpec) -> dict:
    """Verify one exact archive, licence member, and any installed runtime members."""
    archive_bytes = _stable_regular_bytes(archive, MAX_ARCHIVE_BYTES, "archive")
    if (
        len(archive_bytes) != spec.archive_byte_size
        or hashlib.sha256(archive_bytes).hexdigest() != spec.archive_sha256
    ):
        raise ExternalLicenseSourceError("external licence source archive has drifted")
    try:
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:*") as bundle:
            licence_bytes = _unique_regular_member(
                bundle, spec.member_name, MAX_LICENSE_BYTES
            )
            for runtime in spec.runtime_members:
                member_bytes = _unique_regular_member(
                    bundle,
                    runtime.member_name,
                    MAX_RUNTIME_MEMBER_BYTES,
                )
                installed_bytes = _stable_regular_bytes(
                    runtime.runtime_path,
                    MAX_RUNTIME_MEMBER_BYTES,
                    "runtime member",
                )
                if (
                    len(member_bytes) != len(installed_bytes)
                    or hashlib.sha256(member_bytes).digest()
                    != hashlib.sha256(installed_bytes).digest()
                ):
                    raise ExternalLicenseSourceError(
                        "external licence source runtime member has drifted"
                    )
    except (OSError, tarfile.TarError) as error:
        raise ExternalLicenseSourceError(
            "external licence source archive is invalid"
        ) from error
    if (
        len(licence_bytes) != spec.byte_size
        or hashlib.sha256(licence_bytes).hexdigest() != spec.sha256
    ):
        raise ExternalLicenseSourceError("external licence source member has drifted")
    return {
        "licenseFiles": [
            {
                "relativeName": spec.evidence_relative_name,
                "byteSize": len(licence_bytes),
                "sha256": hashlib.sha256(licence_bytes).hexdigest(),
            }
        ],
        "provenance": {
            "kind": "authoritative-source-archive",
            "sourceUrl": spec.source_url,
            "archiveName": spec.archive_name,
            "archiveByteSize": len(archive_bytes),
            "archiveSha256": hashlib.sha256(archive_bytes).hexdigest(),
            "memberName": spec.member_name,
            "byteSize": len(licence_bytes),
            "sha256": hashlib.sha256(licence_bytes).hexdigest(),
            "runtimeMemberCount": len(spec.runtime_members),
        },
    }


def _unique_regular_member(bundle: tarfile.TarFile, name: str, maximum: int) -> bytes:
    """Read one bounded exact regular member and reject duplicate archive names."""
    matches = [member for member in bundle.getmembers() if member.name == name]
    if (
        len(matches) != 1
        or not matches[0].isreg()
        or matches[0].size <= 0
        or matches[0].size > maximum
    ):
        raise ExternalLicenseSourceError(
            "external licence source archive member is invalid"
        )
    stream = bundle.extractfile(matches[0])
    if stream is None:
        raise ExternalLicenseSourceError(
            "external licence source archive member is invalid"
        )
    contents = stream.read(maximum + 1)
    if len(contents) != matches[0].size:
        raise ExternalLicenseSourceError(
            "external licence source archive member has drifted"
        )
    return contents


def _stable_regular_bytes(path: Path, maximum: int, label: str) -> bytes:
    """Read one no-follow regular file while requiring stable identity and size."""
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_size <= 0
            or before.st_size > maximum
        ):
            os.close(descriptor)
            raise ExternalLicenseSourceError(
                f"external licence source {label} is invalid"
            )
        with os.fdopen(descriptor, "rb") as stream:
            contents = stream.read(maximum + 1)
            after = os.fstat(stream.fileno())
    except OSError as error:
        raise ExternalLicenseSourceError(
            f"external licence source {label} is unavailable"
        ) from error
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    observed = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if stable != observed or len(contents) != before.st_size:
        raise ExternalLicenseSourceError(
            f"external licence source {label} changed while read"
        )
    return contents


def _validate_spec(spec: ExternalLicenseSourceSpec) -> None:
    """Reject unsafe or malformed fixed source-catalog entries before filesystem access."""
    archive_name = PurePosixPath(spec.archive_name)
    if (
        not spec.component_identity
        or ":" not in spec.component_identity
        or "@" not in spec.component_identity
        or archive_name.name != spec.archive_name
        or not _is_portable_member_name(spec.member_name)
        or not _is_portable_member_name(spec.evidence_relative_name)
        or spec.archive_byte_size <= 0
        or SHA256_PATTERN.fullmatch(spec.archive_sha256) is None
        or not spec.source_url.startswith("https://")
        or len(spec.source_url.encode()) > 2_048
        or any(ord(character) < 32 for character in spec.source_url)
        or spec.byte_size <= 0
        or SHA256_PATTERN.fullmatch(spec.sha256) is None
        or any(
            not runtime.runtime_path.is_absolute()
            or not _is_portable_member_name(runtime.member_name)
            for runtime in spec.runtime_members
        )
    ):
        raise ExternalLicenseSourceError(
            "external licence source specification is invalid"
        )


def _is_portable_member_name(value: str) -> bool:
    """Return whether a source member name is canonical, relative, and traversal-free."""
    if not isinstance(value, str) or not value:
        return False
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and path.as_posix() == value
        and all(part not in {"", ".", ".."} for part in path.parts)
    )
