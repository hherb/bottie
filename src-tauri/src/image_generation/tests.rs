use std::{
    io::{Cursor, Read, Write},
    net::TcpListener,
    thread,
};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::json;
use url::Url;

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, GeneratedImageDownloader,
    GeneratedImageReference, ImageGenerationProvider, ImageGenerationRequest,
    local::normalize_local_outputs, validate_qwen_image_base_url,
};
use crate::local_image_worker::execution::LocalGeneratedOutput;
use crate::storage::ConversationStore;

#[test]
fn accepts_only_official_qwen_image_api_roots() {
    for base_url in [
        "https://dashscope-intl.aliyuncs.com/api/v1/",
        "https://dashscope.aliyuncs.com/api/v1/",
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/",
        "https://workspace.cn-beijing.maas.aliyuncs.com/api/v1/",
    ] {
        assert!(validate_qwen_image_base_url(base_url).is_ok(), "{base_url}");
    }
    for base_url in [
        "https://example.com/api/v1/",
        "https://maas.aliyuncs.com/api/v1/",
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1/",
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/?key=secret",
    ] {
        assert!(
            validate_qwen_image_base_url(base_url).is_err(),
            "{base_url}"
        );
    }
}

#[test]
fn rejects_empty_malformed_and_unbounded_model_studio_keys() {
    assert!(
        DashScopeQwenImageProvider::for_fixture(
            "https://dashscope-intl.aliyuncs.com/api/v1/",
            " ",
        )
        .is_err()
    );
    assert!(
        DashScopeQwenImageProvider::for_fixture(
            "https://dashscope-intl.aliyuncs.com/api/v1/",
            "key\nvalue",
        )
        .is_err()
    );
    assert!(
        DashScopeQwenImageProvider::for_fixture(
            "https://dashscope-intl.aliyuncs.com/api/v1/",
            &"x".repeat(513),
        )
        .is_err()
    );
}

#[test]
fn validates_and_normalizes_one_provider_neutral_generation_request() {
    let request = ImageGenerationRequest::new("  Draw   a Bottie poster  ", 2_048, 2_048, 2)
        .expect("a documented Qwen-Image-2.0 request should be valid");

    assert_eq!(request.prompt(), "Draw   a Bottie poster");
    assert_eq!(request.dimensions(), (2_048, 2_048));
    assert_eq!(request.count(), 2);
    assert!(request.prompt_extend());
}

#[test]
fn local_generation_accepts_only_the_measured_shape_without_prompt_extension() {
    let request = ImageGenerationRequest::new_local("Draw Bottie", 512, 512, 1)
        .expect("the measured local profile should be valid");

    assert_eq!(request.dimensions(), (512, 512));
    assert_eq!(request.count(), 1);
    assert!(!request.prompt_extend());
    assert!(ImageGenerationRequest::new_local("Draw Bottie", 2_048, 2_048, 1).is_err());
    assert!(ImageGenerationRequest::new_local("Draw Bottie", 512, 512, 2).is_err());
}

#[test]
fn rejects_empty_oversized_and_unsupported_generation_requests() {
    assert!(ImageGenerationRequest::new(" ", 2_048, 2_048, 1).is_err());
    assert!(ImageGenerationRequest::new("x".repeat(1_001), 2_048, 2_048, 1).is_err());
    assert!(ImageGenerationRequest::new("draw", 4_096, 4_096, 1).is_err());
    assert!(ImageGenerationRequest::new("draw", 1_033, 1_032, 1).is_err());
    assert!(ImageGenerationRequest::new("draw", 2_048, 2_048, 0).is_err());
    assert!(ImageGenerationRequest::new("draw", 2_048, 2_048, 7).is_err());
}

#[test]
fn serializes_only_the_pinned_qwen_image_2_model_and_closed_parameters() {
    let provider = DashScopeQwenImageProvider::for_fixture(
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/",
        "test-only-key",
    )
    .expect("the fixture provider should build");
    let request = ImageGenerationRequest::new("Draw Bottie", 2_688, 1_536, 1)
        .expect("the documented landscape dimensions should be valid");

    let value = provider
        .fixture_request_json(&request)
        .expect("the request should serialize");

    assert_eq!(value["model"], DASHSCOPE_QWEN_IMAGE_MODEL_ID);
    assert_eq!(
        value["input"]["messages"],
        json!([{"role": "user", "content": [{"text": "Draw Bottie"}]}])
    );
    assert_eq!(value["parameters"]["size"], "2688*1536");
    assert_eq!(value["parameters"]["n"], 1);
    assert_eq!(value["parameters"]["prompt_extend"], true);
    assert_eq!(value["parameters"]["watermark"], false);
    assert!(value.get("providerId").is_none());
    assert!(value.get("apiKey").is_none());
}

#[test]
fn decodes_only_bounded_https_image_references() {
    let response = br#"{
        "output": {"choices": [{"finish_reason": "stop", "message": {
            "role": "assistant", "content": [
            {"image": "https://result.example/generated.png"}
        ]}}]},
        "request_id": "provider-secret-correlation"
    }"#;

    let images = DashScopeQwenImageProvider::fixture_decode_response(response, (2_048, 2_048), 1)
        .expect("a bounded provider response should decode");

    assert_eq!(images.len(), 1);
    assert_eq!(images[0].url(), "https://result.example/generated.png");
    assert_eq!(images[0].dimensions(), (2_048, 2_048));
}

#[test]
fn rejects_insecure_extra_or_malformed_image_references() {
    for response in [
        br#"{"output":{"choices":[{"finish_reason":"stop","message":{"role":"assistant",
            "content":[{"image":"http://result.example/a.png"}]}}]}}"#
            .as_slice(),
        br#"{"output":{"choices":[{"finish_reason":"stop","message":{"role":"assistant",
            "content":[{"image":"https://result.example/a.png"},
            {"image":"https://result.example/b.png"}]}}]}}"#
            .as_slice(),
        br#"{"output":{"choices":[]}}"#.as_slice(),
    ] {
        assert!(
            DashScopeQwenImageProvider::fixture_decode_response(response, (2_048, 2_048), 1)
                .is_err()
        );
    }
}

#[test]
fn rejects_nonterminal_provider_choices() {
    let response = br#"{
        "output": {"choices": [{"finish_reason": "length", "message": {
            "role": "assistant", "content": [
            {"image": "https://result.example/generated.png"}
        ]}}]}
    }"#;

    assert!(
        DashScopeQwenImageProvider::fixture_decode_response(response, (2_048, 2_048), 1).is_err()
    );
}

#[test]
fn advertises_exact_generation_and_editing_capabilities() {
    let provider = DashScopeQwenImageProvider::for_fixture(
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/",
        "test-only-key",
    )
    .expect("the fixture provider should build");

    let capabilities = provider.capabilities();

    assert_eq!(capabilities.provider_id, "qwen-image");
    assert_eq!(capabilities.model_id, DASHSCOPE_QWEN_IMAGE_MODEL_ID);
    assert!(capabilities.generation);
    assert!(capabilities.editing);
    assert_eq!(capabilities.max_outputs, 6);
    assert_eq!(capabilities.max_pixels, 2_048 * 2_048);
    assert_eq!(capabilities.execution, "cloud");
}

#[test]
fn downloads_decodes_and_stages_only_the_exact_requested_png_dimensions() {
    let body = png_fixture(64, 32);
    let (url, server) = download_fixture("200 OK", "image/png", &body, None);
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let reference = GeneratedImageReference {
        url,
        width: 64,
        height: 32,
    };

    let images = tauri::async_runtime::block_on(
        GeneratedImageDownloader::for_fixture()
            .expect("downloader should build")
            .download_all(&[reference], &store),
    )
    .expect("valid PNG should download");
    server.join().expect("fixture server should stop");

    assert_eq!(images.len(), 1);
    assert_eq!((images[0].width, images[0].height), (64, 32));
    assert!(images[0].temporary_path.is_file());
    assert!(!images[0].sha256.is_empty());
}

#[test]
fn rejects_wrong_media_type_signature_dimensions_and_redirects_without_urls_in_errors() {
    let fixtures = [
        ("200 OK", "text/plain", png_fixture(64, 32), None, (64, 32)),
        ("200 OK", "image/png", b"not a png".to_vec(), None, (64, 32)),
        ("200 OK", "image/png", png_fixture(32, 64), None, (64, 32)),
        (
            "302 Found",
            "image/png",
            Vec::new(),
            Some("https://secret.example/result.png"),
            (64, 32),
        ),
    ];

    for (status, media_type, body, location, dimensions) in fixtures {
        let (url, server) = download_fixture(status, media_type, &body, location);
        let store =
            ConversationStore::initialize(test_database_path()).expect("store should initialize");
        let reference = GeneratedImageReference {
            url: url.clone(),
            width: dimensions.0,
            height: dimensions.1,
        };

        let error = tauri::async_runtime::block_on(
            GeneratedImageDownloader::for_fixture()
                .expect("downloader should build")
                .download_all(&[reference], &store),
        )
        .expect_err("unsafe response should fail closed");
        server.join().expect("fixture server should stop");

        let serialized = serde_json::to_string(&error).expect("error should serialize");
        assert!(!serialized.contains(url.as_str()));
        assert!(!serialized.contains("secret.example"));
    }
}

#[test]
fn removes_every_temporary_file_when_a_later_output_fails() {
    let database_path = test_database_path();
    let store = ConversationStore::initialize(database_path).expect("store should initialize");
    let temporary_directory = store.generated_asset_temporary_directory();
    let valid_body = png_fixture(64, 32);
    let invalid_body = b"not a png".to_vec();
    let (valid_url, valid_server) = download_fixture("200 OK", "image/png", &valid_body, None);
    let (invalid_url, invalid_server) =
        download_fixture("200 OK", "image/png", &invalid_body, None);
    let references = [
        GeneratedImageReference {
            url: valid_url,
            width: 64,
            height: 32,
        },
        GeneratedImageReference {
            url: invalid_url,
            width: 64,
            height: 32,
        },
    ];

    tauri::async_runtime::block_on(
        GeneratedImageDownloader::for_fixture()
            .expect("downloader should build")
            .download_all(&references, &store),
    )
    .expect_err("one invalid output should reject the complete result set");
    valid_server.join().expect("valid fixture should stop");
    invalid_server.join().expect("invalid fixture should stop");

    assert_eq!(
        std::fs::read_dir(temporary_directory)
            .expect("temporary directory should exist")
            .count(),
        0
    );
}

#[test]
fn local_worker_pngs_use_the_shared_native_normalization_boundary() {
    let database_path = test_database_path();
    let store = ConversationStore::initialize(database_path).expect("store should initialize");
    let worker_directory = store
        .generated_asset_temporary_directory()
        .join("fixture-worker");
    std::fs::create_dir_all(&worker_directory).unwrap();
    let worker_path = worker_directory.join("output.png");
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(64, 64, Rgba([20, 40, 60, 255])))
        .save_with_format(&worker_path, ImageFormat::Png)
        .unwrap();
    let output = LocalGeneratedOutput::for_test(worker_path.clone(), 64, 64, 42);

    let prepared = normalize_local_outputs(vec![output], &store)
        .expect("the worker PNG should pass shared normalization");

    assert_eq!(prepared.len(), 1);
    assert_eq!((prepared[0].width, prepared[0].height), (64, 64));
    assert!(!worker_path.exists());
}

/// Creates one deterministic PNG response body.
fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        Rgba([20, 40, 60, 255]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("PNG fixture should encode");
    bytes.into_inner()
}

/// Serves one fixed loopback response for strict downloader tests.
fn download_fixture(
    status: &str,
    media_type: &str,
    body: &[u8],
    location: Option<&str>,
) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener should bind");
    let address = listener.local_addr().expect("fixture address should exist");
    let status = status.to_owned();
    let media_type = media_type.to_owned();
    let body = body.to_vec();
    let location = location.map(str::to_owned);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("fixture request should arrive");
        let mut request = [0_u8; 2_048];
        let _ = stream.read(&mut request);
        let location_header = location
            .map(|value| format!("Location: {value}\r\n"))
            .unwrap_or_default();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {media_type}\r\nContent-Length: {}\r\n{location_header}Connection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("fixture headers should write");
        stream.write_all(&body).expect("fixture body should write");
    });
    (
        Url::parse(&format!("http://{address}/temporary-result.png"))
            .expect("fixture URL should parse"),
        server,
    )
}

/// Creates an isolated application-private path for downloader tests.
fn test_database_path() -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("bottie-image-download-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("test directory should exist");
    directory.join("bottie.sqlite3")
}
