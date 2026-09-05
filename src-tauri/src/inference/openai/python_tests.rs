//! OpenAI-compatible runtime-gated Python definition tests.

use super::*;

#[test]
fn request_maps_python_only_when_the_contained_runtime_is_available() {
    let request: ChatRequest = serde_json::from_value(serde_json::json!({
        "providerId": "openai",
        "modelId": "gpt-example",
        "messages": [{"role": "user", "content": [{"type": "text", "text": "calculate"}]}]
    }))
    .unwrap();

    let without_runtime = serde_json::to_value(
        OpenAiToolSession::new(request.clone(), false)
            .unwrap()
            .request,
    )
    .unwrap();
    let with_runtime =
        serde_json::to_value(OpenAiToolSession::new(request, true).unwrap().request).unwrap();

    assert_eq!(without_runtime["tools"].as_array().map(Vec::len), Some(1));
    assert!(without_runtime["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .all(|tool| tool["function"]["name"] != "run_python")
    }));
    assert_eq!(with_runtime["tools"].as_array().map(Vec::len), Some(2));
    assert_eq!(with_runtime["tools"][1]["function"]["name"], "run_python");
    assert_eq!(
        with_runtime["tools"][1]["function"]["parameters"]["additionalProperties"],
        false
    );
}
