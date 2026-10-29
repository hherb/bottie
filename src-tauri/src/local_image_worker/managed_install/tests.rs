//! Consent and exact-source regression tests for the optional installer.

use super::*;

#[path = "../../local_image_worker_model_download_tests/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use crate::local_image_worker::{
    model_acquisition::{ModelFileContract, ModelPackageManifest},
    model_cache::ModelCacheTransaction,
    model_download::{ModelDownloadLimits, ModelFileSource, ModelSourcePlan},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, time::Duration};

const TERMS: &[u8] = b"test terms";
const WEIGHTS: &[u8] = b"weights";
const ETAG: &str = "\"managed-test-v1\"";
const CANCELLATION_TEST_DELAY: Duration = Duration::from_millis(250);

#[test]
fn pinned_package_preserves_research_license_and_exact_files() {
    let installer = ManagedModelInstaller::new().unwrap();
    let disclosure = installer.disclosure();
    assert_eq!(
        disclosure.package_id,
        "ddalcu/Qwen-Image-2.1-MLX-Serve-8bit"
    );
    assert_eq!(
        disclosure.source_revision,
        "fbda4caa0b4b1e17b5a29633e8deb600386f1eaf"
    );
    assert_eq!(disclosure.expected_disk_bytes, 17_647_912_015);
    assert_eq!(disclosure.license, "LicenseRef-Qwen-Research");
    assert_eq!(disclosure.total_files, 18);
    let plan = &installer.plan;
    let license = plan
        .manifest()
        .files
        .iter()
        .find(|file| file.relative_path == "LICENSE")
        .unwrap();
    assert_eq!(license.sha256, disclosure.license_sha256);
    assert_eq!(
        plan.manifest().runtime_id,
        format!("mlx-serve@{RUNTIME_SOURCE_REVISION}")
    );
    let encoded = serde_json::to_string(&disclosure).unwrap();
    assert!(!encoded.contains("relativePath"));
    assert!(!encoded.contains("expectedMemory"));
}

#[test]
fn consent_must_acknowledge_every_disclosed_identity_and_exact_terms() {
    let installer = ManagedModelInstaller::new().unwrap();
    let valid = approval(&installer);
    assert!(installer.validate_approval(&valid).is_ok());
    let mut mutations = Vec::new();
    let mut wrong = valid.clone();
    wrong.license_accepted = false;
    mutations.push(wrong);
    let mut wrong = valid.clone();
    wrong.package_id.push('x');
    mutations.push(wrong);
    let mut wrong = valid.clone();
    wrong.source_revision.push('x');
    mutations.push(wrong);
    let mut wrong = valid.clone();
    wrong.license_sha256.push('x');
    mutations.push(wrong);
    let mut wrong = valid.clone();
    wrong.expected_disk_bytes += 1;
    mutations.push(wrong);
    let mut wrong = valid.clone();
    wrong.license = "Apache-2.0".into();
    mutations.push(wrong);
    for wrong in mutations {
        assert_eq!(
            installer.validate_approval(&wrong),
            Err(ManagedInstallError::ApprovalRequired)
        );
    }
    let mut omitted = serde_json::to_value(valid).unwrap();
    omitted.as_object_mut().unwrap().remove("licenseAccepted");
    let omitted: ManagedModelInstallApproval = serde_json::from_value(omitted).unwrap();
    assert!(!omitted.license_accepted);
    assert!(installer.validate_approval(&omitted).is_err());
}

#[tokio::test]
async fn rejected_consent_and_precancellation_never_create_the_cache() {
    let installer = ManagedModelInstaller::new().unwrap();
    let root =
        std::env::temp_dir().join(format!("bottie-qwen21-no-install-{}", uuid::Uuid::new_v4()));
    let mut consent = approval(&installer);
    consent.license_accepted = false;
    let cancellation = ModelDownloadCancellation::default();
    assert_eq!(
        installer
            .download(&root, &consent, &cancellation, |_| {})
            .await
            .err(),
        Some(ManagedInstallError::ApprovalRequired)
    );
    consent.license_accepted = true;
    cancellation.cancel();
    assert_eq!(
        installer
            .download(&root, &consent, &cancellation, |_| {})
            .await
            .err(),
        Some(ManagedInstallError::Download(DownloadError::Cancelled))
    );
    assert!(!root.exists());
}

/// Constructs the exact acknowledgement a deliberate install action must supply.
fn approval(installer: &ManagedModelInstaller) -> ManagedModelInstallApproval {
    let disclosure = installer.disclosure();
    ManagedModelInstallApproval {
        package_id: disclosure.package_id,
        source_revision: disclosure.source_revision,
        license: disclosure.license,
        license_sha256: disclosure.license_sha256,
        expected_disk_bytes: disclosure.expected_disk_bytes,
        license_accepted: true,
    }
}

#[tokio::test]
async fn resumes_verified_source_bound_partials_and_returns_only_model_files() {
    let mut response = fixtures::response("206 Partial Content", 4, ETAG, b"ghts");
    response
        .headers
        .push(("Content-Range".into(), "bytes 3-6/7".into()));
    let (url, request, server) = fixtures::serve(response);
    let installer = fixture_installer(&url);
    let root = fixtures::temp_cache("managed-qwen21-resume");
    let transaction = ModelCacheTransaction::open_bound(
        &root,
        installer.plan.manifest().clone(),
        &installer.plan.resume_binding(),
    )
    .unwrap();
    transaction
        .write_file("LICENSE", 0, &mut Cursor::new(TERMS))
        .unwrap();
    transaction
        .write_file("weights.bin", 0, &mut Cursor::new(b"wei"))
        .unwrap();
    drop(transaction);
    assert_eq!(
        installer
            .resume_progress(&root)
            .unwrap()
            .unwrap()
            .downloaded_bytes,
        (TERMS.len() + 3) as u64
    );
    let mut progress = Vec::new();
    let result = installer
        .download(
            &root,
            &approval(&installer),
            &ModelDownloadCancellation::default(),
            |status| progress.push(status),
        )
        .await
        .unwrap();
    server.join().unwrap();
    assert_eq!(
        fs::read(result.directory().join("weights.bin")).unwrap(),
        WEIGHTS
    );
    assert_eq!(fs::read(result.directory().join("LICENSE")).unwrap(), TERMS);
    assert_eq!(progress.last().unwrap().completed_files, 2);
    let wire = request.lock().unwrap().to_ascii_lowercase();
    assert!(wire.contains("range: bytes=3-"));
    assert!(wire.contains(&format!("if-range: {ETAG}")));
    assert!(!wire.contains("authorization"));
    assert!(!wire.contains("/private/"));
    // The source listener is gone: a finished install must verify/reuse its cache without another transfer.
    let reused = installer
        .download(
            &root,
            &approval(&installer),
            &ModelDownloadCancellation::default(),
            |_| panic!("cached install must not download"),
        )
        .await
        .unwrap();
    assert_eq!(reused.directory(), result.directory());
    fs::write(result.directory().join("weights.bin"), b"corrupt").unwrap();
    assert_eq!(
        installer
            .download(
                &root,
                &approval(&installer),
                &ModelDownloadCancellation::default(),
                |_| {}
            )
            .await
            .err(),
        Some(ManagedInstallError::Cache(CacheError::Integrity))
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn corrupted_model_bytes_never_promote_or_become_runtime_ready() {
    let (url, _, server) = fixtures::serve(fixtures::response("200 OK", 7, ETAG, b"corrupt"));
    let installer = fixture_installer(&url);
    let root = fixtures::temp_cache("managed-qwen21-integrity");
    let transaction = ModelCacheTransaction::open_bound(
        &root,
        installer.plan.manifest().clone(),
        &installer.plan.resume_binding(),
    )
    .unwrap();
    transaction
        .write_file("LICENSE", 0, &mut Cursor::new(TERMS))
        .unwrap();
    drop(transaction);
    let result = installer
        .download(
            &root,
            &approval(&installer),
            &ModelDownloadCancellation::default(),
            |_| {},
        )
        .await;
    server.join().unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_dir(root.join("packages")).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_retains_synced_partials_without_promoting_files() {
    let mut response = fixtures::response("200 OK", WEIGHTS.len(), ETAG, b"wei");
    response.delayed_body = b"ghts".to_vec();
    response.delay = CANCELLATION_TEST_DELAY;
    let (url, _, server) = fixtures::serve(response);
    let installer = fixture_installer(&url);
    let root = fixtures::temp_cache("managed-qwen21-cancel");
    let transaction = ModelCacheTransaction::open_bound(
        &root,
        installer.plan.manifest().clone(),
        &installer.plan.resume_binding(),
    )
    .unwrap();
    transaction
        .write_file("LICENSE", 0, &mut Cursor::new(TERMS))
        .unwrap();
    drop(transaction);
    let cancellation = ModelDownloadCancellation::default();
    let result = installer
        .download(&root, &approval(&installer), &cancellation, |status| {
            if status.downloaded_bytes > TERMS.len() as u64 {
                cancellation.cancel();
            }
        })
        .await;
    server.join().unwrap();
    assert_eq!(
        result.err(),
        Some(ManagedInstallError::Download(DownloadError::Cancelled))
    );
    let resume = installer.resume_progress(&root).unwrap().unwrap();
    assert_eq!(resume.completed_files, 1);
    assert_eq!(resume.downloaded_bytes, (TERMS.len() + 3) as u64);
    assert_eq!(fs::read_dir(root.join("packages")).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
}

/// Small exact source plan exercises the production consent wrapper and transactional downloader.
fn fixture_installer(url: &str) -> ManagedModelInstaller {
    let files = [("LICENSE", TERMS), ("weights.bin", WEIGHTS)];
    let manifest = ModelPackageManifest {
        model_id: "Qwen-Image-2.1-MLX-Serve-8bit".into(),
        package_id: "ddalcu/Qwen-Image-2.1-MLX-Serve-8bit".into(),
        runtime_id: format!("mlx-serve@{RUNTIME_SOURCE_REVISION}"),
        license: "LicenseRef-Qwen-Research".into(),
        source_revision: fixtures::SOURCE_REVISION.into(),
        expected_disk_bytes: (TERMS.len() + WEIGHTS.len()) as u64,
        expected_memory_bytes: 128 * 1_024 * 1_024 * 1_024,
        files: files
            .iter()
            .map(|(path, bytes)| ModelFileContract {
                relative_path: (*path).into(),
                byte_size: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(bytes)),
            })
            .collect(),
    };
    let sources = files
        .iter()
        .map(|(path, _)| ModelFileSource {
            relative_path: (*path).into(),
            strong_etag: ETAG.into(),
        })
        .collect();
    let plan =
        ModelSourcePlan::for_loopback_fixture(manifest, url, fixtures::SOURCE_REVISION, sources)
            .unwrap();
    let mut installer = ManagedModelInstaller::from_plan(plan).unwrap();
    installer.downloader = ModelDownloader::for_fixture(ModelDownloadLimits::for_fixture(
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(3),
        32,
        32,
        1,
    ))
    .unwrap();
    installer
}
