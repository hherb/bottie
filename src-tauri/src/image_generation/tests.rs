use serde_json::json;

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, ImageGenerationProvider,
    ImageGenerationRequest, validate_qwen_image_base_url,
};

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
