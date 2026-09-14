//! Hugging Face immutable-resolution tests for the local model downloader.

use std::{fs, time::Duration};

use crate::local_image_worker::{
    model_cache::ModelCacheTransaction,
    model_download::{
        DownloadError, ModelDownloadCancellation, ModelDownloader, ModelFileSource, ModelSourcePlan,
    },
};

use super::fixtures::{
    FixtureResponse, MODEL_BYTES, SOURCE_ETAG, SOURCE_REVISION, approved_acquisition, block_on,
    hugging_face_source_plan, limits, manifest, response, serve_sequence, temp_cache,
};

fn resolver_response(
    revision: &str,
    etag: &str,
    linked_size: usize,
    location: &str,
) -> FixtureResponse {
    FixtureResponse {
        status: "302 Found",
        headers: vec![
            ("Content-Length".into(), "0".into()),
            ("Location".into(), location.into()),
            ("X-Repo-Commit".into(), revision.into()),
            ("X-Linked-ETag".into(), etag.into()),
            ("X-Linked-Size".into(), linked_size.to_string()),
        ],
        first_body: Vec::new(),
        delayed_body: Vec::new(),
        delay: Duration::ZERO,
    }
}

#[test]
fn downloads_only_after_exact_hugging_face_resolution() {
    let mut resolver = resolver_response(
        SOURCE_REVISION,
        SOURCE_ETAG,
        MODEL_BYTES.len(),
        "/resolved/model.bin?public=signed",
    );
    resolver.status = "307 Temporary Redirect";
    let responses = vec![
        resolver,
        response(
            "200 OK",
            MODEL_BYTES.len(),
            "\"cdn-validator\"",
            MODEL_BYTES,
        ),
    ];
    let (root_url, requests, server) = serve_sequence(responses);
    let plan = hugging_face_source_plan(&root_url);
    let cache = temp_cache("hugging-face");
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
    .expect("exact resolver metadata and final bytes should activate");
    server.join().unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with(&format!(
        "GET /models/{SOURCE_REVISION}/weights/model.bin HTTP/1.1"
    )));
    assert!(requests[1].starts_with("GET /resolved/model.bin?public=signed HTTP/1.1"));
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn resumes_through_a_fresh_exact_resolution_without_trusting_the_final_etag() {
    let mut final_response = response("206 Partial Content", 4, "\"cdn-only\"", b"ghts");
    final_response
        .headers
        .push(("Content-Range".into(), "bytes 3-6/7".into()));
    let responses = vec![
        resolver_response(
            SOURCE_REVISION,
            SOURCE_ETAG,
            MODEL_BYTES.len(),
            "/resolved/model.bin?public=fresh",
        ),
        final_response,
    ];
    let (root_url, requests, server) = serve_sequence(responses);
    let plan = hugging_face_source_plan(&root_url);
    let cache = temp_cache("hugging-face-resume");
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
    .expect("fresh exact resolution should resume the retained prefix");
    server.join().unwrap();

    let requests = requests.lock().unwrap();
    assert!(requests[0].to_ascii_lowercase().contains("range: bytes=3-"));
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains(&format!("if-range: {}", SOURCE_ETAG.to_ascii_lowercase()))
    );
    assert!(requests[1].to_ascii_lowercase().contains("range: bytes=3-"));
    assert!(!requests[1].to_ascii_lowercase().contains("if-range:"));
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn rejects_hugging_face_resolution_drift_before_requesting_or_retaining_file_bytes() {
    for (name, response) in [
        (
            "revision",
            resolver_response(
                "abcdef0123456789abcdef0123456789abcdef01",
                SOURCE_ETAG,
                MODEL_BYTES.len(),
                "/resolved/model.bin",
            ),
        ),
        (
            "etag",
            resolver_response(
                SOURCE_REVISION,
                "\"changed\"",
                MODEL_BYTES.len(),
                "/resolved/model.bin",
            ),
        ),
        (
            "size",
            resolver_response(
                SOURCE_REVISION,
                SOURCE_ETAG,
                MODEL_BYTES.len() + 1,
                "/resolved/model.bin",
            ),
        ),
    ] {
        let (root_url, requests, server) = serve_sequence(vec![response]);
        let cache = temp_cache(name);
        let mut acquisition = approved_acquisition();
        let error = block_on(
            ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
                .unwrap()
                .download_to_cache(
                    &hugging_face_source_plan(&root_url),
                    &mut acquisition,
                    &cache,
                    &ModelDownloadCancellation::default(),
                    |_| {},
                ),
        )
        .unwrap_err();
        server.join().unwrap();
        assert_eq!(error, DownloadError::InvalidResponse, "{name}");
        assert_eq!(requests.lock().unwrap().len(), 1, "{name}");
        let replacement = ModelCacheTransaction::open(&cache, manifest()).unwrap();
        assert_eq!(replacement.resume_offset("weights/model.bin").unwrap(), 0);
        fs::remove_dir_all(cache).unwrap();
    }
}

#[test]
fn rejects_a_second_redirect_before_retaining_bytes() {
    let responses = vec![
        resolver_response(
            SOURCE_REVISION,
            SOURCE_ETAG,
            MODEL_BYTES.len(),
            "/resolved/model.bin",
        ),
        resolver_response(
            SOURCE_REVISION,
            SOURCE_ETAG,
            MODEL_BYTES.len(),
            "/third/model.bin",
        ),
    ];
    let (root_url, requests, server) = serve_sequence(responses);
    let cache = temp_cache("second-redirect");
    let mut acquisition = approved_acquisition();
    let error = block_on(
        ModelDownloader::for_fixture(limits(Duration::from_secs(1)))
            .unwrap()
            .download_to_cache(
                &hugging_face_source_plan(&root_url),
                &mut acquisition,
                &cache,
                &ModelDownloadCancellation::default(),
                |_| {},
            ),
    )
    .unwrap_err();
    server.join().unwrap();
    assert_eq!(error, DownloadError::InvalidResponse);
    assert_eq!(requests.lock().unwrap().len(), 2);
    let replacement = ModelCacheTransaction::open(&cache, manifest()).unwrap();
    assert_eq!(replacement.resume_offset("weights/model.bin").unwrap(), 0);
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn production_plan_rejects_untrusted_repository_and_resolution_hosts() {
    let files = vec![ModelFileSource {
        relative_path: "weights/model.bin".into(),
        strong_etag: SOURCE_ETAG.into(),
    }];
    assert_eq!(
        ModelSourcePlan::for_hugging_face(
            manifest(),
            "AbstractFramework/qwen-image-2512-4bit/extra",
            SOURCE_REVISION,
            files.clone(),
        )
        .unwrap_err(),
        DownloadError::InvalidPlan
    );
    let plan = ModelSourcePlan::for_hugging_face(
        manifest(),
        "AbstractFramework/qwen-image-2512-4bit",
        SOURCE_REVISION,
        files,
    )
    .unwrap();
    let source = ModelFileSource {
        relative_path: "weights/model.bin".into(),
        strong_etag: SOURCE_ETAG.into(),
    };
    for location in [
        "https://example.com/model.bin",
        "http://us.aws.cdn.hf.co/model.bin",
        "https://cdn.hf.co.evil.example/model.bin",
        "https://user@us.aws.cdn.hf.co/model.bin",
    ] {
        assert_eq!(
            plan.resolved_url(&source, location).unwrap_err(),
            DownloadError::InvalidResponse,
            "{location}"
        );
    }
    assert!(
        plan.resolved_url(
            &source,
            "https://us.aws.cdn.hf.co/xet/package?public=signed"
        )
        .is_ok()
    );
    let relative = format!(
        "/api/resolve-cache/models/AbstractFramework/qwen-image-2512-4bit/{SOURCE_REVISION}/\
         weights%2Fmodel.bin?etag=fixture"
    )
    .replace(char::is_whitespace, "");
    assert!(plan.resolved_url(&source, &relative).is_ok());
    let unencoded = relative.replace("weights%2Fmodel.bin", "weights/model.bin");
    assert_eq!(
        plan.resolved_url(&source, &unencoded).unwrap_err(),
        DownloadError::InvalidResponse
    );
}

#[test]
fn cache_binding_distinguishes_direct_delivery_from_hugging_face_resolution() {
    let (root_url, _, server) = serve_sequence(Vec::new());
    let resolved = hugging_face_source_plan(&root_url);
    let direct = super::fixtures::source_plan(&root_url);

    assert_ne!(
        resolved.resume_binding_for_test(),
        direct.resume_binding_for_test()
    );
    server.join().unwrap();
}
