//! Anthropic-compatible durable web-fetch mapping and loopback generation tests.

use super::*;
use crate::{
    generation_tools::{execute_anthropic_tool_round, stream_anthropic_tools},
    generation_web_tools::NativeWebFetchExecutor,
    inference::{AnthropicProvider, AnthropicToolCall},
};

/// Socket-free native web-fetch executor for Anthropic-compatible orchestration tests.
struct AnthropicWebFetchExecutor;

impl NativeWebFetchExecutor for AnthropicWebFetchExecutor {
    /// Returns one bounded untrusted inert-page result through the common envelope.
    fn execute(&self, _call: &crate::tool_loop::NativeToolCall) -> MemoryToolExecution {
        bounded_memory_tool_success(json!({
            "sourceUrl": "https://example.com/release",
            "title": "Example release",
            "publishedAt": "2026-08-23",
            "content": "Bounded Anthropic fixture page.",
            "untrusted": true
        }))
    }
}

#[test]
fn executes_anthropic_web_fetch_with_exact_call_identity_and_durable_result() {
    let (store, conversation_id, _message_id, run_id) = active_run("anthropic");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_fetch = AnthropicWebFetchExecutor;

    let results = execute_anthropic_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![AnthropicToolCall::fixture(
            "toolu_fetch_1",
            "web_fetch",
            json!({"url": "https://example.com/release"}),
        )],
        &cancellation,
        false,
        None,
        Some(&web_fetch),
    )
    .expect("Anthropic web-fetch round should execute");

    assert_eq!(results[0].tool_use_id, "toolu_fetch_1");
    assert!(!results[0].is_error);
    assert!(
        results[0]
            .content
            .contains("Bounded Anthropic fixture page.")
    );
    assert!(results[0].content.contains(r#""untrusted":true"#));
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("run should complete");
    let conversation = store
        .load_conversation(&conversation_id)
        .expect("conversation should reload");
    let tool = &conversation.messages[1]
        .provider_run
        .as_ref()
        .expect("response should retain its run")
        .tool_invocations[0];

    assert_eq!(tool.tool_name, "web_fetch");
    assert_eq!(tool.arguments["url"], "https://example.com/release");
    assert_eq!(tool.audit.policy, ToolAuditPolicy::Safe);
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert!(tool.result.as_ref().is_some_and(|result| !result.is_error));
    assert_eq!(state.call_count(), 1);
}

#[test]
fn rejects_anthropic_web_fetch_when_the_fetch_executor_is_absent() {
    let (store, _conversation_id, _message_id, run_id) = active_run("anthropic");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;

    let results = execute_anthropic_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![AnthropicToolCall::fixture(
            "toolu_unadvertised_fetch",
            "web_fetch",
            json!({"url": "https://example.com/private"}),
        )],
        &cancellation,
        false,
        None,
        None,
    )
    .expect("unadvertised fetch should close through the bounded result envelope");

    assert!(results[0].is_error);
    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("https://example.com/private"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_anthropic_web_fetch_result_and_final_answer_across_two_requests() {
    let (store, _conversation_id, _message_id, run_id) = active_run("anthropic");
    let arguments = json!({"url": "https://example.com/release"});
    let tool_response = anthropic_sse_response(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":7}}}),
        json!({
            "type":"content_block_start",
            "index":0,
            "content_block":{"type":"tool_use","id":"toolu_fetch_1","name":"web_fetch","input":{}}
        }),
        json!({
            "type":"content_block_delta",
            "index":0,
            "delta":{"type":"input_json_delta","partial_json":arguments.to_string()}
        }),
        json!({"type":"content_block_stop","index":0}),
        json!({
            "type":"message_delta",
            "delta":{"stop_reason":"tool_use"},
            "usage":{"output_tokens":2,"cost_usd":0.001}
        }),
        json!({"type":"message_stop"}),
    ]);
    let final_response = anthropic_sse_response(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":11}}}),
        json!({
            "type":"content_block_start",
            "index":0,
            "content_block":{"type":"text","text":""}
        }),
        json!({
            "type":"content_block_delta",
            "index":0,
            "delta":{"type":"text_delta","text":"Final answer from page"}
        }),
        json!({"type":"content_block_stop","index":0}),
        json!({
            "type":"message_delta",
            "delta":{"stop_reason":"end_turn"},
            "usage":{"output_tokens":3,"cost_usd":0.002}
        }),
        json!({"type":"message_stop"}),
    ]);
    let (base_url, requests, server) =
        response_fixture_server("text/event-stream", vec![tool_response, final_response]);
    let provider =
        AnthropicProvider::for_loopback_fixture(&base_url).expect("fixture endpoint should build");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-fetch-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );

    let usage = tauri::async_runtime::block_on(stream_anthropic_tools(
        provider,
        web_tool_request(),
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        Some(Arc::new(GenerationWebSearchExecutor)),
        Some(Arc::new(AnthropicWebFetchExecutor)),
    ))
    .expect("two-round Anthropic web-fetch generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Final answer from page");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(2));
    assert_eq!(requests[0]["tools"][0]["name"], "web_search");
    assert_eq!(requests[0]["tools"][1]["name"], "web_fetch");
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(
        requests[1]["messages"][1]["content"][0]["id"],
        "toolu_fetch_1"
    );
    assert_eq!(requests[1]["messages"][2]["role"], "user");
    assert_eq!(
        requests[1]["messages"][2]["content"][0]["tool_use_id"],
        "toolu_fetch_1"
    );
    assert!(
        requests[1]["messages"][2]["content"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Bounded Anthropic fixture page."))
    );
    assert!(
        requests[1]["messages"][2]["content"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains(r#""untrusted":true"#))
    );
}

/// Builds one explicit Anthropic Web request for the two-round fixture.
fn web_tool_request() -> ChatRequest {
    ChatRequest {
        provider_id: "anthropic".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Read the current release page".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: true,
        settings: ChatSettings {
            temperature: None,
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    }
}

/// Encodes complete Anthropic Messages events into one SSE response body.
fn anthropic_sse_response(events: &[serde_json::Value]) -> String {
    events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect()
}
