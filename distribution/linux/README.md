# Verify Bottie Linux packages

Bottie's Debian package carries an embedded OpenPGP `origin` signature. The public certificate in
[`bottie-linux-signing-public.asc`](bottie-linux-signing-public.asc) has this primary fingerprint:

```text
5C1D 104A CE47 2474 CE21 070B 065C FE6D 5D9F D8A4
```

The certificate identity is `Bottie Linux Distribution` and it expires on 24 August 2028. Confirm this fingerprint
through a trusted Bottie release channel before installing the key. A key downloaded from the same location as a
package does not independently establish that package's authenticity.

## Install the verification policy

On Debian or Ubuntu, install `debsig-verify` and run these commands from the repository root:

```sh
fingerprint=5C1D104ACE472474CE21070B065CFE6D5D9FD8A4
keyring_staging_directory="$(mktemp -d)"
gpg --batch --yes --dearmor \
  --output "$keyring_staging_directory/bottie.gpg" \
  distribution/linux/bottie-linux-signing-public.asc
sudo install -d -m 0755 \
  "/usr/share/debsig/keyrings/$fingerprint" \
  "/etc/debsig/policies/$fingerprint"
sudo install -m 0644 "$keyring_staging_directory/bottie.gpg" \
  "/usr/share/debsig/keyrings/$fingerprint/bottie.gpg"
sudo install -m 0644 distribution/linux/bottie.pol \
  "/etc/debsig/policies/$fingerprint/bottie.pol"
rm -r "$keyring_staging_directory"
```

Verify a downloaded package before installing it:

```sh
debsig-verify ./bottie_0.9.0_amd64.deb
```

Successful verification proves that the package's `debian-binary`, control archive, and data archive were signed by
the private key matching the installed Bottie certificate. It does not prove that the key itself was obtained through
an independent trusted channel, that the package is the latest release, or that the beta is approved for broad
redistribution.
