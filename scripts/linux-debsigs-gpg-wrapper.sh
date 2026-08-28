#!/usr/bin/env bash

# Adapts legacy debsigs input to the canonical payload independently reconstructed by Bottie.

set -euo pipefail

readonly EXPECTED_DEBSIGS_ARGUMENT_COUNT=4
readonly SHA256_DIGEST_ALGORITHM=SHA256
readonly SHA256_DIGEST_ALGORITHM_ID=8

expected_key_id="$(printf '%s' "${BOTTIE_LINUX_SIGNING_KEY_ID:-}" | LC_ALL=C /usr/bin/tr '[:lower:]' '[:upper:]')"

if [[ ! "$expected_key_id" =~ ^([A-F0-9]{16}|[A-F0-9]{40})$ ]]; then
  echo "Unexpected legacy debsigs GnuPG arguments." >&2
  exit 1
fi
if (( $# != EXPECTED_DEBSIGS_ARGUMENT_COUNT )); then
  echo "Unexpected legacy debsigs GnuPG arguments." >&2
  exit 1
fi
if [[ "$1" != "--openpgp" ||
  "$2" != "--detach-sign" ||
  "$3" != "--default-key" ||
  "$4" != "$expected_key_id" ]]; then
  echo "Unexpected legacy debsigs GnuPG arguments." >&2
  exit 1
fi

canonical_payload_path="$BOTTIE_LINUX_SIGNING_PAYLOAD_PATH"
debsigs_payload_path="$BOTTIE_LINUX_DEBSIGS_PAYLOAD_PATH"
embedded_signature_path="$BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH"
public_keyring_path="$BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH"

cat > "$debsigs_payload_path"
if ! cmp -s "$debsigs_payload_path" "$canonical_payload_path"; then
  echo "Legacy debsigs payload differs from the canonical Debian payload." >&2
  exit 1
fi
if ! /usr/bin/gpg --no-options --batch --armor --pinentry-mode loopback \
  --passphrase-file "$BOTTIE_LINUX_SIGNING_PASSPHRASE_PATH" \
  --local-user "$expected_key_id" \
  --digest-algo "$SHA256_DIGEST_ALGORITHM" \
  --output "$embedded_signature_path" --detach-sign "$canonical_payload_path" \
  >/dev/null 2>&1; then
  echo "Linux origin signature creation failed." >&2
  exit 1
fi
if ! signature_digest_algorithm="$(
  LC_ALL=C /usr/bin/gpg --no-options --batch --list-packets "$embedded_signature_path" 2>/dev/null |
    awk '$1 == "digest" && $2 == "algo" { sub(/,$/, "", $3); print $3 }'
)"; then
  echo "Linux origin signature did not use SHA-256." >&2
  exit 1
fi
if [[ "$signature_digest_algorithm" != "$SHA256_DIGEST_ALGORITHM_ID" ]]; then
  echo "Linux origin signature did not use SHA-256." >&2
  exit 1
fi
/usr/bin/gpgv --keyring "$public_keyring_path" "$embedded_signature_path" "$canonical_payload_path" \
  >/dev/null 2>&1
cat "$embedded_signature_path"
