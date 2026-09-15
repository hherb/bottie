//! Hosted Qwen-Image-2.0 provider-neutral editing and wire-shape tests.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use futures_util::future::{AbortHandle, Abortable};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::json;

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, ImageEditingProvider,
    ImageEditingRequest,
};
use crate::inference::ProviderErrorCode;
use crate::storage::{
    GeneratedAssetExecution, GeneratedImageSourceFormat, GeneratedImageSourceReference,
    ValidatedGeneratedImageSource,
};

const EDIT_SOURCE_MAX_BYTES: usize = 10 * 1_024 * 1_024;

/// Encodes a tiny valid PNG for native editing-source fixtures.
fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![127_u8; width as usize * height as usize * 4];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

/// Creates one native validated source with an opaque non-path identity.
fn source(format: GeneratedImageSourceFormat, bytes: Vec<u8>) -> ValidatedGeneratedImageSource {
    ValidatedGeneratedImageSource::for_test(
        GeneratedImageSourceReference::Attachment(uuid::Uuid::new_v4().to_string()),
        format,
        8,
        8,
        bytes,
    )
    .expect("validated source should build")
}

#[test]
fn builds_one_provider_neutral_edit_request_from_exact_hosted_lineage() {
    let request = ImageEditingRequest::new(
        "  Turn this into a Bottie poster  ",
        1_024,
        1_024,
        1,
        vec![source(GeneratedImageSourceFormat::Png, png_fixture(8, 8))],
    )
    .expect("hosted edit request should be valid");

    assert_eq!(request.prompt(), "Turn this into a Bottie poster");
    assert_eq!(request.dimensions(), (1_024, 1_024));
    assert_eq!(request.count(), 1);
    assert_eq!(request.sources().len(), 1);
    assert_eq!(
        request.sources()[0].format(),
        GeneratedImageSourceFormat::Png
    );
    assert_eq!(request.sources()[0].dimensions(), (8, 8));
    assert!(
        request.sources()[0]
            .bytes()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    assert_eq!(request.provenance().provider_id, "qwen-image");
    assert_eq!(request.provenance().model_id, DASHSCOPE_QWEN_IMAGE_MODEL_ID);
    assert_eq!(
        request.provenance().execution,
        GeneratedAssetExecution::Cloud
    );
    assert_eq!(request.provenance().seed, None);
}

#[test]
fn edit_request_requires_one_to_three_bounded_validated_native_sources() {
    let new_source = || source(GeneratedImageSourceFormat::Png, png_fixture(8, 8));

    assert!(ImageEditingRequest::new("Edit", 1_024, 1_024, 1, Vec::new()).is_err());
    assert!(
        ImageEditingRequest::new(
            "Edit",
            1_024,
            1_024,
            1,
            vec![new_source(), new_source(), new_source(), new_source()],
        )
        .is_err()
    );
    assert!(
        ValidatedGeneratedImageSource::for_test(
            GeneratedImageSourceReference::Attachment(uuid::Uuid::new_v4().to_string()),
            GeneratedImageSourceFormat::Png,
            8,
            8,
            vec![0; EDIT_SOURCE_MAX_BYTES + 1],
        )
        .is_err()
    );
}

#[test]
fn serializes_ordered_native_edit_bytes_as_closed_data_uris_followed_by_text() {
    let request = ImageEditingRequest::new(
        "Use image 1's shape and image 2's colors",
        1_024,
        1_536,
        2,
        vec![
            source(GeneratedImageSourceFormat::Png, png_fixture(8, 8)),
            source(
                GeneratedImageSourceFormat::Jpeg,
                vec![0xff, 0xd8, 0xff, 0xd9],
            ),
        ],
    )
    .expect("edit request should be valid");
    let provider = DashScopeQwenImageProvider::for_fixture(
        "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/",
        "test-only-key",
    )
    .expect("fixture provider should build");

    let value = provider
        .fixture_edit_request_json(&request)
        .expect("edit request should serialize");
    let content = value["input"]["messages"][0]["content"]
        .as_array()
        .expect("content should be an array");

    assert_eq!(value["model"], DASHSCOPE_QWEN_IMAGE_MODEL_ID);
    assert_eq!(content.len(), 3);
    assert!(
        content[0]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(
        content[1]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,")
    );
    assert_eq!(
        content[2],
        json!({"text": "Use image 1's shape and image 2's colors"})
    );
    assert_eq!(value["parameters"]["size"], "1024*1536");
    assert_eq!(value["parameters"]["n"], 2);
    assert_eq!(value["parameters"]["prompt_extend"], true);
    assert_eq!(value["parameters"]["watermark"], false);
    let serialized = value.to_string();
    assert!(!serialized.contains("sourceId"));
    assert!(!serialized.contains("sha256"));
    assert!(!serialized.contains("generated-assets"));
}

#[test]
fn executes_one_exact_edit_payload_and_decodes_bounded_references() {
    let body = br#"{
        "output":{"choices":[{"finish_reason":"stop","message":{
            "role":"assistant","content":[{"image":"https://result.example/edited.png"}]
        }}]}
    }"#;
    let (base_url, server) = provider_fixture("200 OK", "application/json", body);
    let provider = DashScopeQwenImageProvider::for_http_fixture(&base_url, "test-only-key")
        .expect("fixture provider should build");
    let request = ImageEditingRequest::new(
        "Preserve image order",
        1_024,
        1_024,
        1,
        vec![
            source(GeneratedImageSourceFormat::Png, png_fixture(8, 8)),
            source(
                GeneratedImageSourceFormat::Jpeg,
                vec![0xff, 0xd8, 0xff, 0xd9],
            ),
        ],
    )
    .expect("edit request should be valid");

    let references = tauri::async_runtime::block_on(provider.edit(request))
        .expect("fixture edit should succeed");
    let received = server.join().expect("fixture server should stop");

    assert_eq!(references.len(), 1);
    assert_eq!(references[0].url(), "https://result.example/edited.png");
    assert_eq!(references[0].dimensions(), (1_024, 1_024));
    assert!(received.starts_with("POST /api/v1/services/aigc/multimodal-generation/generation "));
    assert!(received.contains("authorization: Bearer test-only-key\r\n"));
    let payload = received
        .split_once("\r\n\r\n")
        .expect("fixture request should contain a body")
        .1;
    let value: serde_json::Value = serde_json::from_str(payload).expect("body should be JSON");
    let content = value["input"]["messages"][0]["content"]
        .as_array()
        .expect("content should be an array");
    assert!(
        content[0]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(
        content[1]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,")
    );
    assert_eq!(content[2], json!({"text": "Preserve image order"}));
}

#[test]
fn edit_execution_redacts_status_and_transport_failures() {
    let (base_url, server) = provider_fixture(
        "429 Too Many Requests",
        "application/json",
        br#"{"secret":"provider-controlled-body"}"#,
    );
    let provider = DashScopeQwenImageProvider::for_http_fixture(&base_url, "test-only-key")
        .expect("fixture provider should build");
    let error = tauri::async_runtime::block_on(provider.edit(edit_request()))
        .expect_err("provider status should fail closed");
    server.join().expect("fixture server should stop");

    assert_eq!(error.code, ProviderErrorCode::Server);
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("provider-controlled-body")
    );

    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener should bind");
    let base_url = format!("http://{}/api/v1/", listener.local_addr().unwrap());
    drop(listener);
    let provider = DashScopeQwenImageProvider::for_http_fixture(&base_url, "test-only-key")
        .expect("fixture provider should build");
    let error = tauri::async_runtime::block_on(provider.edit(edit_request()))
        .expect_err("transport failure should be normalized");

    assert_eq!(error.code, ProviderErrorCode::Unavailable);
    assert!(!serde_json::to_string(&error).unwrap().contains(&base_url));
}

#[test]
fn edit_execution_rejects_oversized_response_before_reading_provider_bytes() {
    let body = vec![b'x'; 256 * 1_024 + 1];
    let (base_url, server) = provider_fixture("200 OK", "application/json", &body);
    let provider = DashScopeQwenImageProvider::for_http_fixture(&base_url, "test-only-key")
        .expect("fixture provider should build");

    let error = tauri::async_runtime::block_on(provider.edit(edit_request()))
        .expect_err("oversized response should fail closed");
    server.join().expect("fixture server should stop");

    assert_eq!(error.code, ProviderErrorCode::MalformedResponse);
    assert_eq!(
        error.diagnostic.as_deref(),
        Some("response exceeds byte limit")
    );
}

#[test]
fn edit_execution_is_cancellation_safe_while_waiting_for_a_response() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener should bind");
    let base_url = format!("http://{}/api/v1/", listener.local_addr().unwrap());
    let (abort, registration) = AbortHandle::new_pair();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("fixture request should arrive");
        let _ = read_request(&mut stream);
        abort.abort();
    });
    let provider = DashScopeQwenImageProvider::for_http_fixture(&base_url, "test-only-key")
        .expect("fixture provider should build");

    let result =
        tauri::async_runtime::block_on(Abortable::new(provider.edit(edit_request()), registration));
    server.join().expect("fixture server should stop");

    assert!(result.is_err());
}

/// Returns one minimal valid hosted editing request for execution fixtures.
fn edit_request() -> ImageEditingRequest {
    ImageEditingRequest::new(
        "Edit this image",
        1_024,
        1_024,
        1,
        vec![source(GeneratedImageSourceFormat::Png, png_fixture(8, 8))],
    )
    .expect("edit request should be valid")
}

/// Serves one fixed provider response and returns the exact received request.
fn provider_fixture(
    status: &str,
    media_type: &str,
    body: &[u8],
) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener should bind");
    let base_url = format!("http://{}/api/v1/", listener.local_addr().unwrap());
    let status = status.to_owned();
    let media_type = media_type.to_owned();
    let body = body.to_vec();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("fixture request should arrive");
        let request = read_request(&mut stream);
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {media_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("fixture headers should write");
        let _ = stream.write_all(&body);
        request
    });
    (base_url, server)
}

/// Reads one complete request using its bounded HTTP content length.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    const MAX_FIXTURE_REQUEST_BYTES: usize = 128 * 1_024;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4_096];
    loop {
        let read = stream
            .read(&mut buffer)
            .expect("fixture request should read");
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        assert!(bytes.len() <= MAX_FIXTURE_REQUEST_BYTES);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .expect("fixture request should provide a content length");
        if bytes.len() >= header_end + 4 + content_length {
            break;
        }
    }
    String::from_utf8(bytes).expect("fixture request should be UTF-8")
}
