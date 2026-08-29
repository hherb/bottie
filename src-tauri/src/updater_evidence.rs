//! Verifies a protected Tauri updater signature and emits bounded path-free evidence.

use std::{
    env,
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    path::Path,
    process::ExitCode,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_SIGNING_FILE_BYTES: usize = 4_096;
const READ_BUFFER_BYTES: usize = 64 * 1_024;

/// Path-free evidence that one exact artifact passed minisign verification.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdaterEvidence {
    schema_version: u8,
    artifact: ArtifactEvidence,
    public_key_sha256: String,
    signature: SignatureEvidence,
}

/// Hash and size of the exact verified artifact bytes.
#[derive(Debug, PartialEq, Serialize)]
struct ArtifactEvidence {
    sha256: String,
    size: u64,
}

/// Public signature properties retained without signature content.
#[derive(Debug, PartialEq, Serialize)]
struct SignatureEvidence {
    format: &'static str,
    sha256: String,
    verifies: bool,
}

/// Runs the exact explicit verification mode with three caller-selected files.
fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<UpdaterEvidence, ()> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some("--verify".as_ref()) {
        return Err(());
    }
    let artifact_path = arguments.next().ok_or(())?;
    let signature_path = arguments.next().ok_or(())?;
    let public_key_path = arguments.next().ok_or(())?;
    if arguments.next().is_some() {
        return Err(());
    }
    verify_paths(
        Path::new(&artifact_path),
        Path::new(&signature_path),
        Path::new(&public_key_path),
    )
}

/// Loads bounded encoded signing files and verifies the artifact as a stream.
fn verify_paths(
    artifact_path: &Path,
    signature_path: &Path,
    public_key_path: &Path,
) -> Result<UpdaterEvidence, ()> {
    let signature_file = read_encoded_signing_file(signature_path)?;
    let public_key_file = read_encoded_signing_file(public_key_path)?;
    let artifact = File::open(artifact_path).map_err(|_| ())?;
    verify_reader(
        artifact,
        &signature_file.decoded,
        &public_key_file.decoded,
        &signature_file.encoded,
        &public_key_file.encoded,
    )
}

/// One canonical base64 wrapper plus its decoded minisign text.
struct EncodedSigningFile {
    encoded: Vec<u8>,
    decoded: String,
}

/// Reads one small canonical base64 signing file with at most one trailing line feed.
fn read_encoded_signing_file(path: &Path) -> Result<EncodedSigningFile, ()> {
    let encoded = fs::read(path).map_err(|_| ())?;
    if encoded.is_empty() || encoded.len() > MAX_SIGNING_FILE_BYTES || encoded.contains(&b'\r') {
        return Err(());
    }
    let canonical = encoded.strip_suffix(b"\n").unwrap_or(&encoded);
    if canonical.is_empty() || canonical.contains(&b'\n') {
        return Err(());
    }
    let decoded_bytes = STANDARD.decode(canonical).map_err(|_| ())?;
    if STANDARD.encode(&decoded_bytes).as_bytes() != canonical {
        return Err(());
    }
    let decoded = String::from_utf8(decoded_bytes).map_err(|_| ())?;
    Ok(EncodedSigningFile { decoded, encoded })
}

/// Verifies one artifact reader while hashing the exact artifact and signing-file bytes.
fn verify_reader(
    mut artifact: impl Read,
    signature_text: &str,
    public_key_text: &str,
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<UpdaterEvidence, ()> {
    let signature = Signature::decode(signature_text).map_err(|_| ())?;
    let public_key = PublicKey::decode(public_key_text).map_err(|_| ())?;
    let mut verifier = public_key.verify_stream(&signature).map_err(|_| ())?;
    let mut artifact_hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = artifact.read(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        size = size
            .checked_add(u64::try_from(read).map_err(|_| ())?)
            .ok_or(())?;
        artifact_hash.update(&buffer[..read]);
        verifier.update(&buffer[..read]);
    }
    if size == 0 {
        return Err(());
    }
    verifier.finalize().map_err(|_| ())?;
    Ok(UpdaterEvidence {
        schema_version: 1,
        artifact: ArtifactEvidence {
            sha256: hex_digest(artifact_hash.finalize()),
            size,
        },
        public_key_sha256: sha256(public_key_bytes),
        signature: SignatureEvidence {
            format: "minisign",
            sha256: sha256(signature_bytes),
            verifies: true,
        },
    })
}

/// Hashes one byte slice as lowercase SHA-256.
fn sha256(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes))
}

/// Encodes a fixed digest without adding a formatting dependency.
fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write as _;

    digest
        .as_ref()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        })
}

/// Writes only verified JSON or one fixed path-free failure.
fn main() -> ExitCode {
    match run(env::args_os().skip(1)) {
        Ok(evidence) => match serde_json::to_writer(io::stdout().lock(), &evidence) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
        Err(()) => {
            eprintln!("[bottie] updater artifact verification failed.");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_KEY: &str = "untrusted comment: minisign public key: E7620F1842B4E81F\n\
        RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const SIGNATURE: &str = "untrusted comment: signature from minisign secret key\n\
        RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/\
        z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\n\
        trusted comment: timestamp:1633700835\tfile:test\tprehashed\n\
        wLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";

    /// Verifies exact bytes and rejects a changed artifact under the same signature.
    #[test]
    fn verifies_and_hashes_exact_artifact_bytes() {
        let signature_file = format!("{}\n", STANDARD.encode(SIGNATURE));
        let public_key_file = format!("{}\n", STANDARD.encode(PUBLIC_KEY));
        let evidence = verify_reader(
            &b"test"[..],
            SIGNATURE,
            PUBLIC_KEY,
            signature_file.as_bytes(),
            public_key_file.as_bytes(),
        )
        .expect("the published minisign fixture should verify");

        assert_eq!(evidence.schema_version, 1);
        assert_eq!(evidence.artifact.size, 4);
        assert_eq!(evidence.artifact.sha256, sha256(b"test"));
        assert_eq!(evidence.signature.sha256, sha256(signature_file.as_bytes()));
        assert_eq!(
            evidence.public_key_sha256,
            sha256(public_key_file.as_bytes())
        );
        assert!(
            verify_reader(
                &b"changed"[..],
                SIGNATURE,
                PUBLIC_KEY,
                signature_file.as_bytes(),
                public_key_file.as_bytes(),
            )
            .is_err()
        );
    }
}
