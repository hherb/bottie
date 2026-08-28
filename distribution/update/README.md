# Bottie signed update delivery

Bottie's automatic-update trust root is deliberately not configured yet. Tauri requires every updater artifact to
carry a valid updater signature; this verification cannot be disabled. The matching public key must be embedded in
the application, while the private key must remain outside the repository and recoverable for the lifetime of every
installed build that trusts it.

The credential-free contract in `scripts/update-delivery.mjs` prepares that boundary without creating a production
key or enabling network behavior. It:

- accepts only numeric versions and immutable `https://github.com/hherb/bottie/releases/download/v<version>/...`
  artifact URLs;
- accepts one signed artifact per supported desktop target and emits Tauri's deterministic static update-manifest
  shape;
- binds publication evidence to the manifest, public-key, and artifact SHA-256 values while retaining no signature,
  key content, URL, filename, or host path; and
- accepts retained delivery evidence only after the exact manifest is published.

Run the focused credential-free contract with:

```sh
npm run update:contract:test
```

## Production key boundary

Do not generate an updater key as an ordinary build or test step. After explicit release-owner authorization, use the
locked repository Tauri CLI to create one password-protected production key at a secure path outside the repository:

```sh
npm run tauri signer generate -- -w <secure-path-outside-the-repository>
```

Back up the private key and password before embedding its public half. Losing the private key prevents future signed
updates for installed builds; disclosing it breaks the update trust boundary. Never commit the private key, its
password, or a `.env` copy. The later runtime slice must commit only the reviewed public key, enable Tauri v2 updater
artifacts, use the fixed GitHub `latest.json` endpoint over HTTPS, keep checking and installation Rust-owned, and add
an explicit user-controlled update experience. It must test signature failure and must not allow downgrade or
insecure transport.

Creating a tag, GitHub Release, release asset, or `latest.json` publication is a separate external action. A green
contract test, generated manifest, draft release, or uploaded workflow artifact is not publication evidence.

See the official [Tauri updater documentation](https://v2.tauri.app/plugin/updater/) for the upstream signing and
static-manifest protocol.
