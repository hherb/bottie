#!/usr/bin/env bash

# Adapts legacy debsigs input to the canonical payload independently reconstructed by Bottie.

set -euo pipefail

canonical_payload_path="$BOTTIE_LINUX_SIGNING_PAYLOAD_PATH"
debsigs_payload_path="$BOTTIE_LINUX_DEBSIGS_PAYLOAD_PATH"
embedded_signature_path="$BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH"
public_keyring_path="$BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH"

cat > "$debsigs_payload_path"
if ! cmp -s "$debsigs_payload_path" "$canonical_payload_path"; then
  echo "Legacy debsigs payload order differed; canonical Debian payload used." >&2
fi
/usr/bin/gpg --batch --pinentry-mode loopback \
  --passphrase-file "$BOTTIE_LINUX_SIGNING_PASSPHRASE_PATH" \
  --output "$embedded_signature_path" "$@" < "$canonical_payload_path"
/usr/bin/gpgv --keyring "$public_keyring_path" "$embedded_signature_path" "$canonical_payload_path" \
  >/dev/null 2>&1
cat "$embedded_signature_path"
