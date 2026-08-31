//! Ollama-only web-fetch execution and durable provider-reuse tests.

use super::*;
use crate::generation_web_tools::NativeWebFetchExecutor;

/// Socket-free native web-fetch executor for durable orchestration tests.
struct GenerationWebFetchExecutor;

impl NativeWebFetchExecutor for GenerationWebFetchExecutor {
    /// Returns one bounded untrusted inert-page result through the common envelope.
    fn execute(&self, _call: &crate::tool_loop::NativeToolCall) -> MemoryToolExecution {
        bounded_memory_tool_success(json!({
            "sourceUrl": "https://example.com/release",
            "title": "Example release",
            "publishedAt": "2026-08-23",
            "content": "Bounded fixture page.",
            "untrusted": true
        }))
    }
}

#[test]
fn executes_and_persists_an_ollama_web_fetch_before_returning_the_result() {
    let (store, conversation_id, _message_id, run_id) = active_run("ollama");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_fetch = GenerationWebFetchExecutor;

    let results = execute_ollama_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OllamaToolCall::fixture(
            0,
            "web_fetch",
            json!({"url": "https://example.com/release"}),
        )],
        &cancellation,
        false,
        None,
        Some(&web_fetch),
        None,
    )
    .expect("web-fetch round should execute");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_name, "web_fetch");
    assert!(results[0].content.contains("Bounded fixture page."));
    assert!(!results[0].content.contains("<p>"));
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
fn rejects_web_fetch_when_the_ollama_fetch_executor_is_absent() {
    let (store, _conversation_id, _message_id, run_id) = active_run("ollama");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;

    let results = execute_ollama_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OllamaToolCall::fixture(
            0,
            "web_fetch",
            json!({"url": "https://example.com/private"}),
        )],
        &cancellation,
        true,
        None,
        None,
        None,
    )
    .expect("disabled fetch should close through the bounded result envelope");

    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("https://example.com/private"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_ollama_web_fetch_result_and_final_answer_across_two_requests() {
    let (store, _conversation_id, _message_id, run_id) = active_run("ollama");
    let tool_chunk = json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "index": 0,
                    "name": "web_fetch",
                    "arguments": {"url": "https://example.com/release"}
                }
            }]
        },
        "done": true,
        "prompt_eval_count": 7,
        "eval_count": 2
    });
    let final_chunk = json!({
        "message": {"role": "assistant", "content": "Final answer from page"},
        "done": true,
        "prompt_eval_count": 11,
        "eval_count": 3
    });
    let (base_url, requests, server) = fixture_server(vec![tool_chunk, final_chunk]);
    let provider =
        OllamaProvider::with_base_url(&base_url).expect("fixture endpoint should validate");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-fetch-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let request = ChatRequest {
        provider_id: "ollama".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Read the current release page".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: true,
        email_enabled: false,
        audio_enabled: false,
        retain_audio: false,
        settings: ChatSettings {
            temperature: Some(0.0),
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    };

    let usage = tauri::async_runtime::block_on(stream_ollama_tools(
        provider,
        request,
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        None,
        Some(Arc::new(GenerationWebFetchExecutor)),
        None,
    ))
    .expect("two-round Ollama generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Final answer from page");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(3));
    assert_eq!(requests[0]["tools"][0]["function"]["name"], "web_search");
    assert_eq!(requests[0]["tools"][1]["function"]["name"], "web_fetch");
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(requests[1]["messages"][2]["tool_name"], "web_fetch");
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Bounded fixture page."))
    );
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains(r#""untrusted":true"#))
    );
}
