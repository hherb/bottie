"""Validate path-free licence review evidence for one exact Diffusers closure."""

from __future__ import annotations

import base64
import binascii
import hashlib
import re
from pathlib import PurePosixPath


SCHEMA_VERSION = 1
MAX_FIELD_BYTES = 1_024
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
IMAGE_DIGEST_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")
REJECTED_LICENSE_EXPRESSIONS = frozenset(
    {"N/A", "NOASSERTION", "NONE", "TBD", "UNDECLARED", "UNLICENSED", "UNKNOWN"}
)


class LicenseReviewError(RuntimeError):
    """Stable failure raised when licence review evidence is incomplete or has drifted."""


def validate_license_review(
    manifest: object,
    expected_component_ids: set[str],
    expected_image_digest: str,
    expected_trace: dict,
) -> dict[str, dict]:
    """Return exact component reviews after validating closed image and trace bindings."""
    if not isinstance(manifest, dict) or set(manifest) != {
        "schemaVersion",
        "derivedImageDigest",
        "trace",
        "components",
    }:
        raise LicenseReviewError("licence review schema is not closed")
    if manifest["schemaVersion"] != SCHEMA_VERSION:
        raise LicenseReviewError("licence review schema version is unsupported")
    if (
        not isinstance(expected_image_digest, str)
        or IMAGE_DIGEST_PATTERN.fullmatch(expected_image_digest) is None
        or manifest["derivedImageDigest"] != expected_image_digest
    ):
        raise LicenseReviewError("licence review image identity has drifted")
    _validate_trace(manifest["trace"], expected_trace)
    raw_components = manifest["components"]
    if not isinstance(raw_components, list):
        raise LicenseReviewError("licence review components are invalid")
    components = [_validate_component(component) for component in raw_components]
    identities = [component["identity"] for component in components]
    if identities != sorted(identities, key=str.encode) or len(identities) != len(set(identities)):
        raise LicenseReviewError("licence review component identities are not canonical")
    if set(identities) != expected_component_ids:
        raise LicenseReviewError("licence review component coverage has drifted")
    return {component["identity"]: component for component in components}


def _validate_trace(value: object, expected: dict) -> None:
    """Require both proof-trace digests to match the exact reviewed execution."""
    keys = {"pythonTraceSha256", "processMapsSha256"}
    if not isinstance(value, dict) or set(value) != keys or set(expected) != keys:
        raise LicenseReviewError("licence review trace binding is invalid")
    for key in sorted(keys):
        if not isinstance(expected[key], str) or SHA256_PATTERN.fullmatch(expected[key]) is None:
            raise LicenseReviewError("expected trace digest is invalid")
        if value[key] != expected[key]:
            raise LicenseReviewError("licence review trace identity has drifted")


def _validate_component(value: object) -> dict:
    """Normalize one closed component record with measured source and review bytes."""
    if not isinstance(value, dict) or set(value) != {
        "identity",
        "reviewedLicenseExpression",
        "licenseFiles",
        "reviewEvidence",
    }:
        raise LicenseReviewError("licence review component schema is not closed")
    identity = _bounded_text(value["identity"], "component identity")
    if identity.startswith(("first-party:", "host-driver:")) or ":" not in identity or "@" not in identity:
        raise LicenseReviewError("licence review component identity is invalid")
    expression = _bounded_text(value["reviewedLicenseExpression"], "licence expression")
    if expression.upper() in REJECTED_LICENSE_EXPRESSIONS:
        raise LicenseReviewError("licence review expression is incomplete")
    raw_files = value["licenseFiles"]
    if not isinstance(raw_files, list) or not raw_files:
        raise LicenseReviewError("licence review has no measured licence bytes")
    files = [_validate_license_file(file) for file in raw_files]
    names = [file["relativeName"] for file in files]
    if names != sorted(names, key=str.encode) or len(names) != len(set(names)):
        raise LicenseReviewError("licence review file evidence is not canonical")
    review_evidence = _validate_measurement(value["reviewEvidence"], "review evidence")
    return {
        "identity": identity,
        "reviewedLicenseExpression": expression,
        "licenseFiles": files,
        "reviewEvidence": review_evidence,
    }


def _validate_license_file(value: object) -> dict:
    """Validate one portable evidence name and its exact byte measurement."""
    if not isinstance(value, dict) or set(value) != {
        "relativeName",
        "byteSize",
        "sha256",
        "contentsBase64",
    }:
        raise LicenseReviewError("licence file evidence schema is not closed")
    name = _bounded_text(value["relativeName"], "licence evidence name")
    path = PurePosixPath(name)
    if path.is_absolute() or path.as_posix() != name or any(part in {"", ".", ".."} for part in path.parts):
        raise LicenseReviewError("licence evidence name is not portable")
    measurement = _validate_measurement(
        {
            "byteSize": value["byteSize"],
            "sha256": value["sha256"],
            "contentsBase64": value["contentsBase64"],
        },
        "licence file evidence",
    )
    return {"relativeName": name, **measurement}


def _validate_measurement(value: object, label: str) -> dict:
    """Validate a non-empty exact byte count and SHA-256 measurement."""
    if not isinstance(value, dict) or set(value) != {"byteSize", "sha256", "contentsBase64"}:
        raise LicenseReviewError(f"{label} schema is not closed")
    byte_size = value["byteSize"]
    sha256 = value["sha256"]
    if not isinstance(byte_size, int) or isinstance(byte_size, bool) or byte_size <= 0:
        raise LicenseReviewError(f"{label} byte size is invalid")
    if not isinstance(sha256, str) or SHA256_PATTERN.fullmatch(sha256) is None:
        raise LicenseReviewError(f"{label} digest is invalid")
    encoded = value["contentsBase64"]
    if not isinstance(encoded, str):
        raise LicenseReviewError(f"{label} contents are invalid")
    try:
        contents = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError) as error:
        raise LicenseReviewError(f"{label} contents are invalid") from error
    if len(contents) != byte_size or hashlib.sha256(contents).hexdigest() != sha256:
        raise LicenseReviewError(f"{label} contents have drifted")
    return {"byteSize": byte_size, "sha256": sha256}


def _bounded_text(value: object, label: str) -> str:
    """Return one printable bounded field without trimming or normalization guesses."""
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or len(value.encode()) > MAX_FIELD_BYTES
        or any(ord(character) < 32 for character in value)
    ):
        raise LicenseReviewError(f"{label} is invalid")
    return value
