//! Path-backed tests for the transactional local image-model cache.

use std::{
    fs,
    io::{self, Cursor, Read},
    path::PathBuf,
};

use sha2::{Digest, Sha256};

use crate::local_image_worker::{
    model_acquisition::{ModelFileContract, ModelPackageManifest},
    model_cache::{
        CacheError, CachePromotionFault, CacheWriteStatus, ModelCacheTransaction,
        reopen_cached_package,
    },
};

const MODEL_BYTES: &[u8] = b"weights";
const CONFIG_BYTES: &[u8] = b"model";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn file(relative_path: &str, bytes: &[u8]) -> ModelFileContract {
    ModelFileContract {
        relative_path: relative_path.into(),
        byte_size: bytes.len() as u64,
        sha256: digest(bytes),
    }
}

fn manifest() -> ModelPackageManifest {
    let files = vec![
        file("model.json", CONFIG_BYTES),
        file("weights/model.bin", MODEL_BYTES),
    ];
    ModelPackageManifest {
        model_id: "Qwen/Qwen-Image-2512".into(),
        runtime_id: "mlx-gen@0123456789abcdef".into(),
        license: "Apache-2.0".into(),
        source_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        expected_disk_bytes: files.iter().map(|file| file.byte_size).sum(),
        expected_memory_bytes: 20 * 1_024 * 1_024 * 1_024,
        files,
    }
}

fn temp_cache(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "bottie-local-model-cache-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

struct InterruptedReader {
    bytes: Cursor<Vec<u8>>,
    failed: bool,
}

impl InterruptedReader {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: Cursor::new(bytes.to_vec()),
            failed: false,
        }
    }
}

impl Read for InterruptedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, "fixture"));
        }
        let count = self.bytes.read(buffer)?;
        if count > 0 {
            self.failed = true;
        }
        Ok(count)
    }
}

#[test]
fn interrupted_stream_resumes_after_restart_and_reopens_through_activation() {
    let root = temp_cache("resume");
    let package = manifest();
    let transaction = ModelCacheTransaction::open(&root, package.clone()).unwrap();
    let mut interrupted = InterruptedReader::new(b"wei");
    assert_eq!(
        transaction.write_file("weights/model.bin", 0, &mut interrupted),
        Err(CacheError::Interrupted)
    );
    drop(transaction);

    let transaction = ModelCacheTransaction::open(&root, package.clone()).unwrap();
    assert_eq!(transaction.resume_offset("weights/model.bin").unwrap(), 3);
    assert_eq!(
        transaction
            .write_file("weights/model.bin", 3, &mut Cursor::new(b"ghts"))
            .unwrap(),
        CacheWriteStatus::Complete
    );
    assert_eq!(
        transaction
            .write_file("model.json", 0, &mut Cursor::new(CONFIG_BYTES))
            .unwrap(),
        CacheWriteStatus::Complete
    );
    let location = transaction.promote().unwrap();
    assert!(PathBuf::from(&location.model_directory).is_dir());
    assert_eq!(location.model_id, package.model_id);
    assert_eq!(location.model_revision, package.source_revision);

    let reopened = reopen_cached_package(&root, package)
        .unwrap()
        .expect("promoted package should reopen");
    assert_eq!(reopened, location);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manifest_drift_discards_the_previous_partial_transaction() {
    let root = temp_cache("drift");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    assert_eq!(
        transaction
            .write_file("weights/model.bin", 0, &mut Cursor::new(b"wei"))
            .unwrap(),
        CacheWriteStatus::Incomplete
    );
    let old_staging = transaction.staging_root_for_test();
    drop(transaction);

    let mut drifted = manifest();
    drifted.runtime_id = "mlx-gen@fedcba9876543210".into();
    drifted.source_revision = "abcdef0123456789abcdef0123456789abcdef01".into();
    let replacement = ModelCacheTransaction::open(&root, drifted).unwrap();
    assert_eq!(replacement.resume_offset("weights/model.bin").unwrap(), 0);
    assert_eq!(replacement.staging_root_for_test(), old_staging);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn writes_accept_only_exact_declared_paths_and_offsets() {
    let root = temp_cache("paths");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    assert_eq!(
        transaction.write_file("../outside", 0, &mut Cursor::new(b"x")),
        Err(CacheError::InvalidPath)
    );
    assert_eq!(
        transaction.write_file("other.bin", 0, &mut Cursor::new(b"x")),
        Err(CacheError::InvalidPath)
    );
    assert_eq!(
        transaction.write_file("weights/model.bin", 1, &mut Cursor::new(MODEL_BYTES)),
        Err(CacheError::InvalidState)
    );
    assert!(!root.join("outside").exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn write_rejects_a_symlinked_staging_parent_without_touching_the_target() {
    use std::os::unix::fs::symlink;

    let root = temp_cache("symlink");
    let outside = temp_cache("symlink-outside");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    symlink(
        &outside,
        transaction.staging_root_for_test().join("weights"),
    )
    .unwrap();
    assert_eq!(
        transaction.write_file("weights/model.bin", 0, &mut Cursor::new(MODEL_BYTES)),
        Err(CacheError::Integrity)
    );
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn opening_a_broken_staging_symlink_replaces_only_that_managed_link() {
    use std::os::unix::fs::symlink;

    let root = temp_cache("broken-staging-link");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    let staging = transaction.staging_root_for_test();
    transaction.discard().unwrap();
    symlink(root.join("missing-target"), &staging).unwrap();

    let replacement = ModelCacheTransaction::open(&root, manifest()).unwrap();
    assert!(replacement.staging_root_for_test().is_dir());
    assert!(!replacement.staging_root_for_test().is_symlink());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn digest_and_size_failures_remove_the_untrusted_file() {
    let root = temp_cache("integrity");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    let staged_file = transaction
        .staging_root_for_test()
        .join("weights/model.bin");
    assert_eq!(
        transaction.write_file("weights/model.bin", 0, &mut Cursor::new(b"changed")),
        Err(CacheError::Integrity)
    );
    assert!(!staged_file.exists());
    assert_eq!(
        transaction.write_file("weights/model.bin", 0, &mut Cursor::new(b"weights!")),
        Err(CacheError::Integrity)
    );
    assert!(!staged_file.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verified_promotion_is_atomic_and_retryable_before_rename() {
    let root = temp_cache("promotion");
    let package = manifest();
    let transaction = ModelCacheTransaction::open(&root, package.clone()).unwrap();
    transaction
        .write_file("model.json", 0, &mut Cursor::new(CONFIG_BYTES))
        .unwrap();
    transaction
        .write_file("weights/model.bin", 0, &mut Cursor::new(MODEL_BYTES))
        .unwrap();
    let staging = transaction.staging_root_for_test();
    let final_root = transaction.final_root_for_test();
    assert_eq!(
        transaction.promote_with_fault(CachePromotionFault::BeforeRename),
        Err(CacheError::Storage)
    );
    assert!(staging.is_dir());
    assert!(!final_root.exists());

    transaction.promote().unwrap();
    assert!(!staging.exists());
    assert!(final_root.is_dir());
    assert!(reopen_cached_package(&root, package).unwrap().is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reacquisition_replaces_an_invalid_exact_promoted_package() {
    let root = temp_cache("replace-corrupt-final");
    let package = manifest();
    let original = ModelCacheTransaction::open(&root, package.clone()).unwrap();
    original
        .write_file("model.json", 0, &mut Cursor::new(CONFIG_BYTES))
        .unwrap();
    original
        .write_file("weights/model.bin", 0, &mut Cursor::new(MODEL_BYTES))
        .unwrap();
    let final_root = original.final_root_for_test();
    original.promote().unwrap();
    fs::write(final_root.join("weights/model.bin"), b"changed").unwrap();

    let replacement = ModelCacheTransaction::open(&root, package.clone()).unwrap();
    replacement
        .write_file("model.json", 0, &mut Cursor::new(CONFIG_BYTES))
        .unwrap();
    replacement
        .write_file("weights/model.bin", 0, &mut Cursor::new(MODEL_BYTES))
        .unwrap();
    replacement.promote().unwrap();

    assert_eq!(
        fs::read(final_root.join("weights/model.bin")).unwrap(),
        MODEL_BYTES
    );
    assert!(reopen_cached_package(&root, package).unwrap().is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn promotion_rechecks_the_exact_native_manifest_before_rename() {
    let root = temp_cache("manifest-before-rename");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    transaction
        .write_file("model.json", 0, &mut Cursor::new(CONFIG_BYTES))
        .unwrap();
    transaction
        .write_file("weights/model.bin", 0, &mut Cursor::new(MODEL_BYTES))
        .unwrap();
    let staging = transaction.staging_root_for_test();
    let final_root = transaction.final_root_for_test();
    fs::remove_file(staging.join(".bottie-model-manifest.json")).unwrap();

    assert_eq!(transaction.promote(), Err(CacheError::Integrity));
    assert!(staging.is_dir());
    assert!(!final_root.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_discard_removes_only_the_exact_partial_transaction() {
    let root = temp_cache("cleanup");
    let transaction = ModelCacheTransaction::open(&root, manifest()).unwrap();
    transaction
        .write_file("weights/model.bin", 0, &mut Cursor::new(b"wei"))
        .unwrap();
    let staging = transaction.staging_root_for_test();
    transaction.discard().unwrap();
    assert!(!staging.exists());
    assert!(root.exists());
    fs::remove_dir_all(root).unwrap();
}
