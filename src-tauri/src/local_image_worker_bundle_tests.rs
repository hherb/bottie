//! Cross-language evidence vectors for canonical local-image worker bundles.

use std::fs;

use crate::local_image_worker::worker_bundle::hash_worker_bundle;

const EXECUTABLE: &str = "bottie/bottie-local-image-diffusers-worker";
const LICENSE_METADATA: &str = concat!(
    r#"{"schemaVersion": 1, "firstPartyPathPrefixes": ["bottie", "metadata/third-party-licenses.json"], "#,
    r#""thirdPartyComponents": [{"name": "dependency", "version": "1.0.0", "#,
    r#""source": "https://example.invalid/dependency-1.0.0", "licenseExpression": "MIT", "#,
    r#""pathPrefixes": ["runtime", "licenses/dependency.txt"], "#,
    r#""licenseFiles": ["licenses/dependency.txt"]}]}"#,
);
const CROSS_LANGUAGE_BUNDLE_SHA256: &str =
    "ec5c805f95ca73de1185ec619689ce11dae09bda567b8831274528ce47347fbc";

/// Requires the native verifier to retain the candidate producer's canonical byte contract.
#[test]
fn native_bundle_hash_matches_the_python_candidate_evidence_vector() {
    let root = std::env::temp_dir().join(format!(
        "bottie-linux-worker-bundle-vector-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(root.join("bottie")).unwrap();
    fs::create_dir_all(root.join("runtime")).unwrap();
    fs::create_dir_all(root.join("licenses")).unwrap();
    fs::create_dir_all(root.join("metadata")).unwrap();
    fs::write(root.join(EXECUTABLE), b"worker-bytes").unwrap();
    fs::write(root.join("runtime/dependency.py"), b"dependency-bytes").unwrap();
    fs::write(root.join("licenses/dependency.txt"), b"MIT licence text").unwrap();
    fs::write(
        root.join("metadata/third-party-licenses.json"),
        LICENSE_METADATA,
    )
    .unwrap();

    let evidence = hash_worker_bundle(&root, &root.join(EXECUTABLE)).unwrap();

    assert_eq!(evidence.bundle_sha256, CROSS_LANGUAGE_BUNDLE_SHA256);
    assert_eq!(evidence.executable_sha256, sha256_hex(b"worker-bytes"));
    fs::remove_dir_all(root).unwrap();
}

/// Returns one lowercase SHA-256 test expectation.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    format!("{:x}", Sha256::digest(bytes))
}
