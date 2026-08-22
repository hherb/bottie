//! OpenAI-compatible durable call correlation and loopback generation tests.

use super::*;

#[test]
fn preserves_openai_call_identity_while_persisting_the_exact_result() {
    let (store, conversation_id, message_id, run_id) = active_run("openai");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;

    let results = execute_openai_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OpenAiToolCall::fixture(
            "call_openai_1",
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
    )
    .expect("OpenAI tool round should execute");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id, "call_openai_1");
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
fn executes_openai_web_search_with_exact_call_identity_and_durable_result() {
    let (store, conversation_id, _message_id, run_id) = active_run("openai");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_search = GenerationWebSearchExecutor;

    let results = execute_openai_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OpenAiToolCall::fixture(
            "call_openai_web_1",
            "web_search",
            json!({"query": "current Rust release", "freshness": "month", "limit": 3}),
        )],
        &cancellation,
        false,
        Some(&web_search),
    )
    .expect("OpenAI web-search round should execute");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id, "call_openai_web_1");
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
    assert_eq!(tool.audit.policy, ToolAuditPolicy::Safe);
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert!(tool.result.as_ref().is_some_and(|result| !result.is_error));
    assert_eq!(state.call_count(), 1);
}

#[test]
fn rejects_openai_memory_calls_when_only_web_was_explicitly_enabled() {
    let (store, _conversation_id, _message_id, run_id) = active_run("openai");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_search = GenerationWebSearchExecutor;

    let results = execute_openai_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OpenAiToolCall::fixture(
            "call_openai_disabled_memory",
            "search_memory",
            json!({"query": "must remain disabled"}),
        )],
        &cancellation,
        false,
        Some(&web_search),
    )
    .expect("disabled memory call should close through the bounded result envelope");

    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("must remain disabled"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_openai_web_search_result_and_final_answer_across_two_requests() {
    let (store, _conversation_id, _message_id, run_id) = active_run("openai");
    let tool_event = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_openai_web_1",
            "type": "function",
            "function": {
                "name": "web_search",
                "arguments": serde_json::to_string(&json!({
                    "query": "current Rust release",
                    "freshness": "month",
                    "limit": 3
                })).unwrap()
            }
        }]}}]
    });
    let first_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 7, "completion_tokens": 2, "cost": 0.001}
    });
    let final_event = json!({"choices": [{"delta": {"content": "Final answer"}}]});
    let final_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 11, "completion_tokens": 3, "cost": 0.002}
    });
    let responses = vec![
        sse_response(&[tool_event, first_usage]),
        sse_response(&[final_event, final_usage]),
    ];
    let (base_url, requests, server) = response_fixture_server("text/event-stream", responses);
    let provider =
        OpenAiProvider::for_loopback_fixture(&base_url).expect("fixture endpoint should build");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-tool-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let request = ChatRequest {
        provider_id: "openai".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Find the current Rust release".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: true,
        settings: ChatSettings {
            temperature: None,
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    };

    let usage = tauri::async_runtime::block_on(stream_openai_tools(
        provider,
        request,
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        Some(Arc::new(GenerationWebSearchExecutor)),
    ))
    .expect("two-round OpenAI web generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Final answer");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(1));
    assert_eq!(requests[0]["tools"][0]["function"]["name"], "web_search");
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(
        requests[1]["messages"][1]["tool_calls"][0]["id"],
        "call_openai_web_1"
    );
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(
        requests[1]["messages"][2]["tool_call_id"],
        "call_openai_web_1"
    );
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains("https://example.com/release"))
    );
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_openai_tool_call_result_and_final_answer_across_two_requests() {
    let (store, conversation_id, message_id, run_id) = active_run("openai");
    let tool_event = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_openai_1",
            "type": "function",
            "function": {
                "name": "open_memory",
                "arguments": serde_json::to_string(&json!({
                    "conversationId": conversation_id,
                    "messageId": message_id,
                    "before": 0,
                    "after": 0
                })).unwrap()
            }
        }]}}]
    });
    let first_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 7, "completion_tokens": 2, "cost": 0.001}
    });
    let final_event = json!({"choices": [{"delta": {"content": "Final answer"}}]});
    let final_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 11, "completion_tokens": 3, "cost": 0.002}
    });
    let responses = vec![
        sse_response(&[tool_event, first_usage]),
        sse_response(&[final_event, final_usage]),
    ];
    let (base_url, requests, server) = response_fixture_server("text/event-stream", responses);
    let provider =
        OpenAiProvider::for_loopback_fixture(&base_url).expect("fixture endpoint should build");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-tool-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let request = ChatRequest {
        provider_id: "openai".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Open this exact memory".into(),
            }],
        }],
        memory_enabled: true,
        web_enabled: false,
        settings: ChatSettings {
            temperature: None,
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    };

    let usage = tauri::async_runtime::block_on(stream_openai_tools(
        provider,
        request,
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        None,
    ))
    .expect("two-round OpenAI generation should complete")
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
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(3));
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(
        requests[1]["messages"][1]["tool_calls"][0]["id"],
        "call_openai_1"
    );
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(requests[1]["messages"][2]["tool_call_id"], "call_openai_1");
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains(r#""ok":true"#))
    );
}
