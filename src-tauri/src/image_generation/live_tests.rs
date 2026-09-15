//! Opt-in live check for the exact hosted Qwen-Image-2.0 adapter.

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, ImageGenerationProvider,
    ImageGenerationRequest, validate_qwen_image_base_url,
};

const API_KEY_ENV: &str = "BOTTIE_LIVE_QWEN_IMAGE_API_KEY";
const BASE_URL_ENV: &str = "BOTTIE_LIVE_QWEN_IMAGE_BASE_URL";
const SINGAPORE_WORKSPACE_SUFFIX: &str = ".ap-southeast-1.maas.aliyuncs.com";
const LIVE_OUTPUT_DIMENSIONS: (u32, u32) = (512, 512);
const LIVE_OUTPUT_COUNT: u8 = 1;
const LIVE_PROMPT: &str = "A simple blue circle centered on a white background, no text.";

#[test]
#[ignore = "requires an explicit billed request with a throwaway Singapore key and workspace"]
fn live_qwen_image_generation_uses_one_low_risk_output() {
    let base_url =
        std::env::var(BASE_URL_ENV).expect("set the opt-in Singapore workspace API root");
    let validated = validate_qwen_image_base_url(&base_url)
        .expect("the live fixture requires an official Qwen Image API root");
    assert!(
        validated
            .host_str()
            .is_some_and(|host| host.ends_with(SINGAPORE_WORKSPACE_SUFFIX)),
        "the live fixture is restricted to a Singapore Model Studio workspace"
    );
    let api_key = std::env::var(API_KEY_ENV).expect("set the opt-in throwaway Model Studio key");
    let provider = DashScopeQwenImageProvider::new(validated.as_str(), api_key)
        .expect("build the exact hosted image adapter");
    let request = ImageGenerationRequest::new(
        LIVE_PROMPT,
        LIVE_OUTPUT_DIMENSIONS.0,
        LIVE_OUTPUT_DIMENSIONS.1,
        LIVE_OUTPUT_COUNT,
    )
    .expect("build the fixed one-output live request");

    let references = tauri::async_runtime::block_on(provider.generate(request))
        .expect("complete one bounded hosted image request");

    assert_eq!(
        provider.capabilities().model_id,
        DASHSCOPE_QWEN_IMAGE_MODEL_ID
    );
    assert_eq!(references.len(), usize::from(LIVE_OUTPUT_COUNT));
    assert_eq!(references[0].dimensions(), LIVE_OUTPUT_DIMENSIONS);
    assert!(references[0].url().starts_with("https://"));
}
