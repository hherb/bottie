//! Anthropic-compatible Localmail definition, block correlation, audit, and provider-reuse tests.

use super::*;
use crate::{
    generation_localmail_tools::NativeLocalmailToolExecutor,
    generation_tools::{execute_anthropic_tool_round, stream_anthropic_tools},
    inference::{AnthropicProvider, AnthropicToolCall},
};

/// Socket-free Localmail executor retaining exactly the provider-selected calls it receives.
#[derive(Clone, Default)]
struct GenerationAnthropicLocalmailExecutor {
    calls: Arc<Mutex<Vec<crate::tool_loop::NativeToolCall>>>,
}

impl NativeLocalmailToolExecutor for GenerationAnthropicLocalmailExecutor {
    /// Returns one bounded untrusted email-search result through the common envelope.
    fn execute(&self, call: &crate::tool_loop::NativeToolCall) -> MemoryToolExecution {
        self.calls.lock().unwrap().push(call.clone());
        bounded_memory_tool_success(json!({
            "results": [{
                "messageId": "42",
                "subject": "Quarterly plan",
                "sender": {"name": "Alex", "address": "alex@example.test"},
                "date": "2026-08-23T08:00:00Z",
                "snippet": "Review the bounded plan.",
                "hasAttachments": false
            }],
            "untrusted": true
        }))
    }
}

#[test]
fn executes_and_persists_anthropic_email_with_exact_call_identity() {
    let (store, conversation_id, _message_id, run_id) = active_run("anthropic");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let localmail = GenerationAnthropicLocalmailExecutor::default();

    let results = execute_anthropic_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![AnthropicToolCall::fixture(
            "toolu_anthropic_email_1",
            "search_email",
            json!({"query": "quarterly plan", "resultLimit": 3}),
        )],
        &cancellation,
        false,
        None,
        None,
        Some(&localmail),
    )
    .expect("Anthropic Email round should execute");

    assert_eq!(results[0].tool_use_id, "toolu_anthropic_email_1");
    assert!(!results[0].is_error);
    assert!(results[0].content.contains("Quarterly plan"));
    assert!(results[0].content.contains(r#""untrusted":true"#));
    assert_eq!(
        localmail.calls.lock().unwrap()[0].call_id,
        "toolu_anthropic_email_1"
    );
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

    assert_eq!(tool.tool_name, "search_email");
    assert_eq!(tool.arguments["query"], "quarterly plan");
    assert_eq!(tool.audit.policy, ToolAuditPolicy::Safe);
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert!(tool.result.as_ref().is_some_and(|result| !result.is_error));
    assert_eq!(state.call_count(), 1);
}

#[test]
fn rejects_anthropic_email_when_no_configured_executor_is_present() {
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
            "toolu_anthropic_email_disabled",
            "open_email",
            json!({"messageId": "42"}),
        )],
        &cancellation,
        false,
        None,
        None,
        None,
    )
    .expect("disabled Email call should close through the bounded result envelope");

    assert!(results[0].is_error);
    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("42"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_anthropic_email_result_after_preserved_reasoning_blocks() {
    let (store, _conversation_id, _message_id, run_id) = active_run("anthropic");
    let arguments = json!({"query": "quarterly plan", "resultLimit": 3});
    let tool_response = anthropic_sse_response(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":7}}}),
        json!({
            "type":"content_block_start",
            "index":0,
            "content_block":{"type":"thinking","thinking":"","signature":""}
        }),
        json!({
            "type":"content_block_delta",
            "index":0,
            "delta":{"type":"thinking_delta","thinking":"Check Localmail"}
        }),
        json!({
            "type":"content_block_delta",
            "index":0,
            "delta":{"type":"signature_delta","signature":"opaque-signature"}
        }),
        json!({"type":"content_block_stop","index":0}),
        json!({
            "type":"content_block_start",
            "index":1,
            "content_block":{"type":"redacted_thinking","data":"opaque-redacted-state"}
        }),
        json!({"type":"content_block_stop","index":1}),
        json!({
            "type":"content_block_start",
            "index":2,
            "content_block":{
                "type":"tool_use",
                "id":"toolu_anthropic_email_1",
                "name":"search_email",
                "input":{}
            }
        }),
        json!({
            "type":"content_block_delta",
            "index":2,
            "delta":{"type":"input_json_delta","partial_json":arguments.to_string()}
        }),
        json!({"type":"content_block_stop","index":2}),
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
            "delta":{"type":"text_delta","text":"Final answer from email"}
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
        std::env::temp_dir().join(format!(
            "bottie-anthropic-email-model-{}",
            uuid::Uuid::new_v4()
        )),
        store.clone(),
        Diagnostics::default(),
    );
    let localmail = GenerationAnthropicLocalmailExecutor::default();

    let usage = tauri::async_runtime::block_on(stream_anthropic_tools(
        provider,
        email_tool_request(),
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        None,
        None,
        Some(Arc::new(localmail.clone())),
    ))
    .expect("two-round Anthropic-compatible Email generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(
        sink.text.lock().unwrap().as_str(),
        "Final answer from email"
    );
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    assert_eq!(localmail.calls.lock().unwrap().len(), 1);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(3));
    assert_eq!(requests[0]["tools"][0]["name"], "search_email");
    assert_eq!(requests[0]["tools"][1]["name"], "open_email");
    assert_eq!(requests[0]["tools"][2]["name"], "current_time");
    let assistant_blocks = &requests[1]["messages"][1]["content"];
    assert_eq!(assistant_blocks[0]["type"], "thinking");
    assert_eq!(assistant_blocks[0]["thinking"], "Check Localmail");
    assert_eq!(assistant_blocks[0]["signature"], "opaque-signature");
    assert_eq!(assistant_blocks[1]["type"], "redacted_thinking");
    assert_eq!(assistant_blocks[1]["data"], "opaque-redacted-state");
    assert_eq!(assistant_blocks[2]["type"], "tool_use");
    assert_eq!(assistant_blocks[2]["id"], "toolu_anthropic_email_1");
    assert_eq!(assistant_blocks[2]["name"], "search_email");
    assert_eq!(assistant_blocks[2]["input"], arguments);
    assert_eq!(requests[1]["messages"][2]["role"], "user");
    let result_block = &requests[1]["messages"][2]["content"][0];
    assert_eq!(result_block["type"], "tool_result");
    assert_eq!(result_block["tool_use_id"], "toolu_anthropic_email_1");
    assert!(
        result_block["content"]
            .as_str()
            .is_some_and(|content| content.contains("Quarterly plan"))
    );
}

/// Builds one explicit Anthropic Email request for the two-round fixture.
fn email_tool_request() -> ChatRequest {
    ChatRequest {
        provider_id: "anthropic".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Find the quarterly plan email".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: false,
        email_enabled: true,
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
