//! oMLX Localmail definition, dispatch, audit, and provider-reuse tests.

use super::*;
use crate::generation_localmail_tools::NativeLocalmailToolExecutor;

/// Socket-free Localmail executor retaining exactly the provider-selected calls it receives.
#[derive(Clone, Default)]
struct GenerationOmlxLocalmailExecutor {
    calls: Arc<Mutex<Vec<crate::tool_loop::NativeToolCall>>>,
}

impl NativeLocalmailToolExecutor for GenerationOmlxLocalmailExecutor {
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
fn executes_and_persists_omlx_email_with_exact_call_identity() {
    let (store, conversation_id, _message_id, run_id) = active_run("omlx");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let localmail = GenerationOmlxLocalmailExecutor::default();

    let results = execute_openai_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OpenAiToolCall::fixture(
            "call_omlx_email_1",
            "search_email",
            json!({"query": "quarterly plan", "resultLimit": 3}),
        )],
        &cancellation,
        false,
        None,
        None,
        Some(&localmail),
    )
    .expect("oMLX Email round should execute");

    assert_eq!(results[0].tool_call_id, "call_omlx_email_1");
    assert!(results[0].content.contains("Quarterly plan"));
    assert!(results[0].content.contains(r#""untrusted":true"#));
    assert_eq!(
        localmail.calls.lock().unwrap()[0].call_id,
        "call_omlx_email_1"
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
fn rejects_omlx_email_when_no_configured_executor_is_present() {
    let (store, _conversation_id, _message_id, run_id) = active_run("omlx");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;

    let results = execute_openai_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OpenAiToolCall::fixture(
            "call_omlx_email_disabled",
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
fn streams_omlx_email_result_and_final_answer_across_two_requests() {
    let (store, _conversation_id, _message_id, run_id) = active_run("omlx");
    let tool_event = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_omlx_email_1",
            "type": "function",
            "function": {
                "name": "search_email",
                "arguments": serde_json::to_string(&json!({
                    "query": "quarterly plan",
                    "resultLimit": 3
                })).unwrap()
            }
        }]}}]
    });
    let first_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 7, "completion_tokens": 2}
    });
    let final_event = json!({"choices": [{"delta": {"content": "Final answer from email"}}]});
    let final_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 11, "completion_tokens": 3}
    });
    let responses = vec![
        sse_response(&[tool_event, first_usage]),
        sse_response(&[final_event, final_usage]),
    ];
    let (base_url, requests, server) = response_fixture_server("text/event-stream", responses);
    let provider = OmlxProvider::with_base_url(&base_url).expect("fixture endpoint should build");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-omlx-email-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let localmail = GenerationOmlxLocalmailExecutor::default();
    let request = ChatRequest {
        provider_id: "omlx".into(),
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
        audio_enabled: false,
        retain_audio: false,
        settings: ChatSettings {
            temperature: None,
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    };

    let usage = tauri::async_runtime::block_on(stream_omlx_tools(
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
    .expect("two-round oMLX Email generation should complete")
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
    assert_eq!(
        requests[1]["messages"][1]["tool_calls"][0]["id"],
        "call_omlx_email_1"
    );
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(
        requests[1]["messages"][2]["tool_call_id"],
        "call_omlx_email_1"
    );
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Quarterly plan"))
    );
}
