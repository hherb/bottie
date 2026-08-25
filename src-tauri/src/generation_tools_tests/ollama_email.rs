//! Ollama-only Localmail definition, dispatch, audit, and provider-reuse tests.

use super::*;
use crate::generation_localmail_tools::NativeLocalmailToolExecutor;

/// Socket-free Localmail executor retaining exactly the provider-selected calls it receives.
#[derive(Clone, Default)]
struct GenerationLocalmailExecutor {
    calls: Arc<Mutex<Vec<crate::tool_loop::NativeToolCall>>>,
}

impl NativeLocalmailToolExecutor for GenerationLocalmailExecutor {
    fn execute(&self, call: &crate::tool_loop::NativeToolCall) -> MemoryToolExecution {
        self.calls.lock().unwrap().push(call.clone());
        bounded_memory_tool_success(json!({
            "results": [{
                "messageId": "42",
                "subject": "Quarterly status",
                "sender": {"address": "ops@example.com", "name": "Ops"},
                "date": "2026-08-23T08:00:00Z",
                "snippet": "Bounded inert summary.",
                "hasAttachments": false
            }],
            "untrusted": true
        }))
    }
}

#[test]
fn executes_and_persists_an_ollama_email_call_through_the_configured_executor() {
    let (store, conversation_id, _message_id, run_id) = active_run("ollama");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let localmail = GenerationLocalmailExecutor::default();

    let results = execute_ollama_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OllamaToolCall::fixture(
            0,
            "search_email",
            json!({"query": "quarterly status", "limit": 3}),
        )],
        &cancellation,
        false,
        None,
        None,
        Some(&localmail),
    )
    .expect("configured Email round should execute");

    assert_eq!(results[0].tool_name, "search_email");
    assert!(results[0].content.contains("Bounded inert summary."));
    assert!(results[0].content.contains(r#""untrusted":true"#));
    assert_eq!(
        localmail.calls.lock().unwrap()[0].arguments["query"],
        "quarterly status"
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
    assert_eq!(tool.audit.policy, ToolAuditPolicy::Safe);
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert!(tool.result.as_ref().is_some_and(|result| !result.is_error));
}

#[test]
fn rejects_email_calls_when_no_configured_executor_is_present() {
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

    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("42"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_ollama_email_result_and_final_answer_across_two_requests() {
    let (store, _conversation_id, _message_id, run_id) = active_run("ollama");
    let tool_chunk = json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "index": 0,
                    "name": "search_email",
                    "arguments": {"query": "quarterly status", "limit": 3}
                }
            }]
        },
        "done": true,
        "prompt_eval_count": 7,
        "eval_count": 2
    });
    let final_chunk = json!({
        "message": {"role": "assistant", "content": "Final answer from email"},
        "done": true,
        "prompt_eval_count": 11,
        "eval_count": 3
    });
    let (base_url, requests, server) = fixture_server(vec![tool_chunk, final_chunk]);
    let provider =
        OllamaProvider::with_base_url(&base_url).expect("fixture endpoint should validate");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-email-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let localmail = GenerationLocalmailExecutor::default();
    let request = ChatRequest {
        provider_id: "ollama".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Find the quarterly status email".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: false,
        email_enabled: true,
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
        None,
        Some(Arc::new(localmail.clone())),
    ))
    .expect("two-round Ollama generation should complete")
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
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(4));
    assert_eq!(requests[0]["tools"][0]["function"]["name"], "search_email");
    assert_eq!(requests[0]["tools"][1]["function"]["name"], "open_email");
    assert_eq!(
        requests[0]["tools"][2]["function"]["name"],
        "read_email_attachment"
    );
    assert_eq!(requests[0]["tools"][3]["function"]["name"], "current_time");
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(requests[1]["messages"][2]["tool_name"], "search_email");
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Bounded inert summary."))
    );
}
