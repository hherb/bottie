//! Loopback tests for strict resumable local image-model downloads.

use std::{
    fs,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::local_image_worker::{
    model_acquisition::{AcquisitionPhase, ModelAcquisition},
    model_cache::ModelCacheTransaction,
    model_download::{
        DownloadError, ModelDownloadCancellation, ModelDownloadLimits, ModelDownloader,
        ModelFileSource, ModelSourcePlan,
    },
};

#[path = "local_image_worker_model_download_tests/fixtures.rs"]
mod fixtures;

use fixtures::{
    FixtureResponse, MODEL_BYTES, SOURCE_ETAG, SOURCE_REVISION, approved_acquisition, block_on,
    limits, manifest, response, serve, source_plan, source_plan_with_etag, temp_cache,
};

#[test]
fn downloads_a_full_exact_file_and_publishes_only_synced_progress() {
    let (root_url, request, server) = serve(response(
        "200 OK",
        MODEL_BYTES.len(),
        SOURCE_ETAG,
        MODEL_BYTES,
    ));
    let plan = source_plan(&root_url);
    let package = plan.manifest().clone();
    let cache = temp_cache("full");
    let observed = Arc::new(Mutex::new(Vec::new()));
    let progress = Arc::clone(&observed);
    let cache_for_progress = cache.clone();
    let package_for_progress = package.clone();
    let mut acquisition = approved_acquisition();

    let location = block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &plan,
                &mut acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                move |status| {
                    let transaction = ModelCacheTransaction::open(
                        &cache_for_progress,
                        package_for_progress.clone(),
                    )
                    .unwrap();
                    assert_eq!(
                        transaction.resume_offset("weights/model.bin").unwrap(),
                        status.downloaded_bytes
                    );
                    progress.lock().unwrap().push(status);
                },
            ),
    )
    .expect("exact response should download and promote");
    server.join().unwrap();

    assert_eq!(location.model_revision, SOURCE_REVISION);
    assert_eq!(acquisition.status().phase, AcquisitionPhase::Ready);
    assert_eq!(observed.lock().unwrap().last().unwrap().completed_files, 1);
    let request = request.lock().unwrap();
    assert!(request.starts_with(&format!(
        "GET /models/{SOURCE_REVISION}/weights/model.bin HTTP/1.1"
    )));
    assert!(!request.contains("Range:"));
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn resumes_only_with_an_exact_range_validator_and_remaining_length() {
    let mut ranged = response("206 Partial Content", 4, SOURCE_ETAG, b"ghts");
    ranged
        .headers
        .push(("Content-Range".into(), "bytes 3-6/7".into()));
    let (root_url, request, server) = serve(ranged);
    let plan = source_plan(&root_url);
    let cache = temp_cache("resume");
    let transaction =
        ModelCacheTransaction::open_bound(&cache, manifest(), &plan.resume_binding_for_test())
            .unwrap();
    transaction
        .write_file("weights/model.bin", 0, &mut std::io::Cursor::new(b"wei"))
        .unwrap();
    drop(transaction);
    let mut acquisition = approved_acquisition();

    block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &plan,
                &mut acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                |_| {},
            ),
    )
    .expect("exact range should complete the retained prefix");
    server.join().unwrap();
    let request = request.lock().unwrap().to_ascii_lowercase();
    assert!(request.contains("range: bytes=3-"));
    assert!(request.contains(&format!("if-range: {}", SOURCE_ETAG.to_ascii_lowercase())));
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn rejects_ignored_or_wrong_ranges_and_validator_drift_then_discards_partial() {
    for (name, mut fixture) in [
        (
            "ignored",
            response("200 OK", MODEL_BYTES.len(), SOURCE_ETAG, MODEL_BYTES),
        ),
        (
            "wrong-range",
            response("206 Partial Content", 4, SOURCE_ETAG, b"ghts"),
        ),
        (
            "validator-drift",
            response("206 Partial Content", 4, "\"changed\"", b"ghts"),
        ),
        (
            "duplicate-validator",
            response("206 Partial Content", 4, SOURCE_ETAG, b"ghts"),
        ),
    ] {
        if name != "ignored" {
            fixture
                .headers
                .push(("Content-Range".into(), "bytes 3-6/7".into()));
        }
        if name == "wrong-range" {
            fixture.headers.pop();
            fixture
                .headers
                .push(("Content-Range".into(), "bytes 2-5/7".into()));
        }
        if name == "duplicate-validator" {
            fixture.headers.push(("ETag".into(), "\"second\"".into()));
        }
        let (root_url, _, server) = serve(fixture);
        let plan = source_plan(&root_url);
        let cache = temp_cache(name);
        let transaction =
            ModelCacheTransaction::open_bound(&cache, manifest(), &plan.resume_binding_for_test())
                .unwrap();
        transaction
            .write_file("weights/model.bin", 0, &mut std::io::Cursor::new(b"wei"))
            .unwrap();
        drop(transaction);
        let mut acquisition = approved_acquisition();

        let result = block_on(
            ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
                .unwrap()
                .download_to_cache(
                    &plan,
                    &mut acquisition,
                    &cache,
                    &ModelDownloadCancellation::default(),
                    |_| {},
                ),
        );
        server.join().unwrap();
        assert_eq!(
            result.unwrap_err(),
            DownloadError::InvalidResponse,
            "{name}"
        );
        let replacement = ModelCacheTransaction::open(&cache, manifest()).unwrap();
        assert_eq!(replacement.resume_offset("weights/model.bin").unwrap(), 0);
        fs::remove_dir_all(cache).unwrap();
    }
}

#[test]
fn rejects_redirects_and_overflow_before_retaining_untrusted_bytes() {
    for (name, fixture) in [
        (
            "redirect",
            FixtureResponse {
                status: "302 Found",
                headers: vec![("Location".into(), "https://secret.example/weights".into())],
                first_body: Vec::new(),
                delayed_body: Vec::new(),
                delay: Duration::ZERO,
            },
        ),
        (
            "overflow",
            response("200 OK", MODEL_BYTES.len() + 1, SOURCE_ETAG, b"weights!"),
        ),
    ] {
        let (root_url, _, server) = serve(fixture);
        let cache = temp_cache(name);
        let mut acquisition = approved_acquisition();
        let error = block_on(
            ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
                .unwrap()
                .download_to_cache(
                    &source_plan(&root_url),
                    &mut acquisition,
                    &cache,
                    &ModelDownloadCancellation::default(),
                    |_| {},
                ),
        )
        .unwrap_err();
        server.join().unwrap();
        assert_eq!(error, DownloadError::InvalidResponse);
        let replacement = ModelCacheTransaction::open(&cache, manifest()).unwrap();
        assert_eq!(replacement.resume_offset("weights/model.bin").unwrap(), 0);
        fs::remove_dir_all(cache).unwrap();
    }
}

#[test]
fn retains_synced_truncation_for_a_later_exact_resume() {
    let (root_url, _, server) = serve(response("200 OK", MODEL_BYTES.len(), SOURCE_ETAG, b"wei"));
    let cache = temp_cache("truncated");
    let mut acquisition = approved_acquisition();
    let error = block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &source_plan(&root_url),
                &mut acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                |_| {},
            ),
    )
    .unwrap_err();
    server.join().unwrap();
    assert_eq!(error, DownloadError::Interrupted);
    let reopened = ModelCacheTransaction::open(&cache, manifest()).unwrap();
    assert_eq!(reopened.resume_offset("weights/model.bin").unwrap(), 3);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn source_validator_drift_discards_a_retained_prefix_before_the_next_request() {
    let (first_root, _, first_server) =
        serve(response("200 OK", MODEL_BYTES.len(), SOURCE_ETAG, b"wei"));
    let cache = temp_cache("source-binding");
    let mut first_acquisition = approved_acquisition();
    let first_result = block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &source_plan(&first_root),
                &mut first_acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                |_| {},
            ),
    );
    first_server.join().unwrap();
    assert_eq!(first_result.unwrap_err(), DownloadError::Interrupted);

    let changed_etag = "\"fixture-weights-v2\"";
    let (second_root, request, second_server) = serve(response(
        "200 OK",
        MODEL_BYTES.len(),
        changed_etag,
        MODEL_BYTES,
    ));
    let mut second_acquisition = approved_acquisition();
    block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &source_plan_with_etag(&second_root, changed_etag),
                &mut second_acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                |_| {},
            ),
    )
    .expect("changed source binding should start a clean full request");
    second_server.join().unwrap();
    assert!(
        !request
            .lock()
            .unwrap()
            .to_ascii_lowercase()
            .contains("range:")
    );
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn propagates_cancellation_and_timeout_after_syncing_received_bytes() {
    for (name, cancel, expected) in [
        ("cancel", true, DownloadError::Cancelled),
        ("timeout", false, DownloadError::Timeout),
    ] {
        let fixture = FixtureResponse {
            status: "200 OK",
            headers: vec![
                ("Content-Length".into(), MODEL_BYTES.len().to_string()),
                ("ETag".into(), SOURCE_ETAG.into()),
            ],
            first_body: b"wei".to_vec(),
            delayed_body: b"ghts".to_vec(),
            delay: Duration::from_millis(200),
        };
        let (root_url, _, server) = serve(fixture);
        let cache = temp_cache(name);
        let cancellation = ModelDownloadCancellation::default();
        let mut acquisition = approved_acquisition();
        let cancellation_thread = if cancel {
            let signal = cancellation.clone();
            Some(thread::spawn(move || {
                thread::sleep(Duration::from_millis(40));
                signal.cancel();
            }))
        } else {
            None
        };
        let timeout = if cancel {
            Duration::from_secs(1)
        } else {
            Duration::from_millis(50)
        };
        let error = block_on(
            ModelDownloader::for_fixture(limits(timeout))
                .unwrap()
                .download_to_cache(
                    &source_plan(&root_url),
                    &mut acquisition,
                    &cache,
                    &cancellation,
                    |_| {},
                ),
        )
        .unwrap_err();
        if let Some(cancellation_thread) = cancellation_thread {
            cancellation_thread.join().unwrap();
        }
        server.join().unwrap();
        assert_eq!(error, expected);
        let reopened = ModelCacheTransaction::open(&cache, manifest()).unwrap();
        assert!(reopened.resume_offset("weights/model.bin").unwrap() <= 3);
        fs::remove_dir_all(cache).unwrap();
    }
}

#[test]
fn rejects_unapproved_roots_revision_drift_and_incomplete_file_bindings() {
    let files = vec![ModelFileSource {
        relative_path: "weights/model.bin".into(),
        strong_etag: SOURCE_ETAG.into(),
    }];
    assert_eq!(
        ModelSourcePlan::new(
            manifest(),
            "http://example.com/models/",
            SOURCE_REVISION,
            files.clone()
        )
        .unwrap_err(),
        DownloadError::InvalidPlan
    );
    assert_eq!(
        ModelSourcePlan::new(
            manifest(),
            "https://example.com/models/",
            "abcdef0123456789abcdef0123456789abcdef01",
            files,
        )
        .unwrap_err(),
        DownloadError::InvalidPlan
    );
    assert_eq!(
        ModelSourcePlan::new(
            manifest(),
            "https://example.com/models/",
            SOURCE_REVISION,
            vec![],
        )
        .unwrap_err(),
        DownloadError::InvalidPlan
    );
}

#[test]
fn refuses_to_open_the_cache_or_network_before_explicit_acquisition_approval() {
    let plan = ModelSourcePlan::new(
        manifest(),
        "https://example.com/models/",
        SOURCE_REVISION,
        vec![ModelFileSource {
            relative_path: "weights/model.bin".into(),
            strong_etag: SOURCE_ETAG.into(),
        }],
    )
    .unwrap();
    let cache = temp_cache("approval");
    let mut acquisition = ModelAcquisition::new(manifest()).unwrap();
    let result = block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &plan,
                &mut acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                |_| {},
            ),
    );

    assert_eq!(result.unwrap_err(), DownloadError::ApprovalRequired);
    assert!(fs::read_dir(&cache).unwrap().next().is_none());
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn rejects_package_limits_before_any_network_request() {
    let plan = ModelSourcePlan::new(
        manifest(),
        "https://example.com/models/",
        SOURCE_REVISION,
        vec![ModelFileSource {
            relative_path: "weights/model.bin".into(),
            strong_etag: SOURCE_ETAG.into(),
        }],
    )
    .unwrap();
    let cache = temp_cache("limits");
    let mut acquisition = approved_acquisition();
    let result = block_on(
        ModelDownloader::for_fixture(ModelDownloadLimits::for_fixture(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            6,
            6,
            1,
        ))
        .unwrap()
        .download_to_cache(
            &plan,
            &mut acquisition,
            &cache,
            &ModelDownloadCancellation::default(),
            |_| {},
        ),
    );
    assert_eq!(result.unwrap_err(), DownloadError::LimitExceeded);
    fs::remove_dir_all(cache).unwrap();
}
