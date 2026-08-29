# Bottie signed update delivery

Bottie's production automatic-update trust root is configured. Tauri requires every updater artifact to carry a valid
updater signature; this verification cannot be disabled. The generated public half is committed as
`bottie-updater.pub` and embedded only by Rust. Its SHA-256 is
`fd4adf69a4bea10958a0f63f0658083fa29bfad10c48c792877dcdcdb8c6355c`. The password-protected private key remains
outside the repository and must stay recoverable for the lifetime of every installed build that trusts it.

The credential-free contract in `scripts/update-delivery.mjs` prepares publication without using private material. It:

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

## Completed production key ceremony

The release owner explicitly authorized the ceremony on 2026-08-28. The locked repository Tauri CLI generated one
password-protected key. Its final state is:

- the primary encrypted private-key file is outside the repository with owner-only permissions;
- a byte-identical encrypted backup is in a separate recoverable iCloud Drive recovery folder;
- the randomly generated password is stored in the macOS login Keychain under service
  `com.bottie.app.updater-signing` and account `production`;
- the committed public half matches the rotated private key; and
- a disposable signing check recovered the password from Keychain and produced canonical Tauri `.sig` content.

The initial generated pair was immediately discarded and overwritten before its public half was used because the npm
wrapper echoed that first password in its command banner. The final pair was generated with the direct locked CLI,
which did not echo its password. No installed build or published artifact trusted the discarded key.

Do not generate or rotate this key as an ordinary build or test step. For an authorized local protected build, expose
the existing key only through Tauri's standard process environment; do not copy either value into the repository:

```sh
export TAURI_SIGNING_PRIVATE_KEY_PATH='<secure-primary-path-outside-the-repository>'
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password \
  -s com.bottie.app.updater-signing -w)"
```

Losing the private key or password prevents future signed updates for installed builds; disclosing either breaks their
trust boundary. Never commit the private key, its password, or a `.env` copy.

## Runtime and protected artifact boundary

Settings exposes explicit check, install, and cancel actions through Bottie commands. Rust alone owns the fixed
`https://github.com/hherb/bottie/releases/latest/download/latest.json` endpoint, 15-second timeout, public key,
candidate recheck, strict-upgrade comparison, download, signature verification, and native installation. The WebView
receives only current/candidate versions, bounded link-free release notes, fixed status, and fixed path-free errors.
No updater plugin permission or JavaScript updater binding is granted.

Ordinary build/package commands do not create updater artifacts. `src-tauri/tauri.updater.conf.json` is applied only by
protected distribution paths. Because Bottie's platform trust steps alter bundle bytes, each protected path creates
the Tauri signature last: after notarization/stapling for the final macOS updater archive, after Authenticode for the
final MSI, and after embedded OpenPGP verification for the final DEB. Protected GitHub workflows reference
`BOTTIE_UPDATER_SIGNING_PRIVATE_KEY` and `BOTTIE_UPDATER_SIGNING_PRIVATE_KEY_PASSWORD`; configuring those environment
secrets and dispatching a runner remain separate explicit actions and were not performed in this slice.

Creating a tag, GitHub Release, release asset, or `latest.json` publication is a separate external action. A green
contract test, generated manifest, draft release, or uploaded workflow artifact is not publication evidence.

See the official [Tauri updater documentation](https://v2.tauri.app/plugin/updater/) for the upstream signing and
static-manifest protocol.
