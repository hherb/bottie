//! Anthropic-specific durable generation-loop tests.

use super::*;
use crate::{
    generation_tools::{execute_anthropic_tool_round, stream_anthropic_tools},
    inference::{AnthropicProvider, AnthropicToolCall},
};

#[test]
fn preserves_call_identity_while_persisting_the_exact_result() {
    let (store, conversation_id, message_id, run_id) = active_run("anthropic");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;

    let results = execute_anthropic_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![AnthropicToolCall::fixture(
            "toolu_anthropic_1",
            "open_memory",
            json!({
                "conversationId": conversation_id,
                "messageId": message_id,
                "before": 0,
                "after": 0
            }),
        )],
        &cancellation,
        true,
        None,
        None,
        None,
    )
    .expect("Anthropic tool round should execute");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_use_id, "toolu_anthropic_1");
    assert!(!results[0].is_error);
    assert!(results[0].content.contains(r#""ok":true"#));
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
    assert_eq!(tool.tool_name, "open_memory");
    assert!(tool.result.as_ref().is_some_and(|result| !result.is_error));
    assert_eq!(state.call_count(), 1);
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_call_result_and_final_answer_across_two_requests() {
    let (store, conversation_id, message_id, run_id) = active_run("anthropic");
    let arguments = json!({
        "conversationId": conversation_id,
        "messageId": message_id,
        "before": 0,
        "after": 0
    });
    let tool_response = anthropic_sse_response(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":7}}}),
        json!({
            "type":"content_block_start",
            "index":0,
            "content_block":{"type":"tool_use","id":"toolu_anthropic_1","name":"open_memory","input":{}}
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
            "delta":{"type":"text_delta","text":"Final answer"}
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
        std::env::temp_dir().join(format!("bottie-tool-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );

    let usage = tauri::async_runtime::block_on(stream_anthropic_tools(
        provider,
        tool_request(),
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        None,
        None,
        None,
    ))
    .expect("two-round Anthropic generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Final answer");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    assert!(
        usage
            .cost_usd
            .is_some_and(|cost| (cost - 0.003).abs() < f64::EPSILON)
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(4));
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(requests[1]["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(requests[1]["messages"][2]["role"], "user");
    assert_eq!(
        requests[1]["messages"][2]["content"][0]["type"],
        "tool_result"
    );
    assert_eq!(
        requests[1]["messages"][2]["content"][0]["tool_use_id"],
        "toolu_anthropic_1"
    );
    assert!(
        requests[1]["messages"][2]["content"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains(r#""ok":true"#))
    );
}

#[test]
fn executes_and_persists_an_anthropic_web_search_before_returning_the_result() {
    let (store, conversation_id, _message_id, run_id) = active_run("anthropic");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_search = GenerationWebSearchExecutor;

    let results = execute_anthropic_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![AnthropicToolCall::fixture(
            "toolu_web_1",
            "web_search",
            json!({"query": "current Rust release", "freshness": "month", "limit": 3}),
        )],
        &cancellation,
        false,
        Some(&web_search),
        None,
        None,
    )
    .expect("Anthropic web-search round should execute");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_use_id, "toolu_web_1");
    assert!(!results[0].is_error);
    assert!(results[0].content.contains("https://example.com/release"));
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

    assert_eq!(tool.tool_name, "web_search");
    assert_eq!(tool.arguments["query"], "current Rust release");
    assert_eq!(tool.audit.policy, ToolAuditPolicy::Safe);
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert!(tool.result.as_ref().is_some_and(|result| !result.is_error));
    assert_eq!(state.call_count(), 1);
}

#[test]
fn rejects_anthropic_memory_calls_when_only_web_was_explicitly_enabled() {
    let (store, _conversation_id, _message_id, run_id) = active_run("anthropic");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_search = GenerationWebSearchExecutor;

    let results = execute_anthropic_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![AnthropicToolCall::fixture(
            "toolu_disabled_memory",
            "search_memory",
            json!({"query": "must remain disabled"}),
        )],
        &cancellation,
        false,
        Some(&web_search),
        None,
        None,
    )
    .expect("disabled memory call should close through the bounded result envelope");

    assert!(results[0].is_error);
    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("must remain disabled"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_anthropic_web_search_result_and_final_answer_across_two_requests() {
    let (store, _conversation_id, _message_id, run_id) = active_run("anthropic");
    let arguments = json!({"query": "current Rust release", "freshness": "month", "limit": 3});
    let tool_response = anthropic_sse_response(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":7}}}),
        json!({
            "type":"content_block_start",
            "index":0,
            "content_block":{"type":"tool_use","id":"toolu_web_1","name":"web_search","input":{}}
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
            "delta":{"type":"text_delta","text":"Final answer"}
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
        std::env::temp_dir().join(format!("bottie-tool-model-{}", uuid::Uuid::new_v4())),
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
        None,
        None,
    ))
    .expect("two-round Anthropic web generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Final answer");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(3));
    assert_eq!(requests[0]["tools"][0]["name"], "web_search");
    assert_eq!(requests[0]["tools"][1]["name"], "web_fetch");
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(
        requests[1]["messages"][1]["content"][0]["id"],
        "toolu_web_1"
    );
    assert_eq!(requests[1]["messages"][2]["role"], "user");
    assert_eq!(
        requests[1]["messages"][2]["content"][0]["tool_use_id"],
        "toolu_web_1"
    );
    assert!(
        requests[1]["messages"][2]["content"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("https://example.com/release"))
    );
}

/// Builds one explicit Anthropic memory-tool request for the two-round fixture.
fn tool_request() -> ChatRequest {
    ChatRequest {
        provider_id: "anthropic".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Open this exact memory".into(),
            }],
        }],
        memory_enabled: true,
        web_enabled: false,
        email_enabled: false,
        audio_enabled: false,
        retain_audio: false,
        settings: ChatSettings {
            temperature: None,
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    }
}

/// Builds one explicit Anthropic web-search request for the two-round fixture.
fn web_tool_request() -> ChatRequest {
    ChatRequest {
        provider_id: "anthropic".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Find the current Rust release".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: true,
        email_enabled: false,
        audio_enabled: false,
        retain_audio: false,
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
