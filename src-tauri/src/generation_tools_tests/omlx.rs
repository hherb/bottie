//! oMLX streamed native-tool loop integration tests.

use super::*;
use crate::inference::InferenceProvider;
use chrono::{Datelike, Utc};

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_omlx_clock_result_and_final_answer_across_two_requests() {
    let (store, conversation_id, _message_id, run_id) = active_run("omlx");
    let first_call_fragment = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_omlx_clock",
            "type": "function",
            "function": {"name": "current_", "arguments": "{"}
        }]}}]
    });
    let second_call_fragment = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "function": {"name": "time", "arguments": "}"}
        }]}}]
    });
    let first_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 7, "completion_tokens": 2}
    });
    let final_event = json!({"choices": [{"delta": {"content": "Clock grounded answer"}}]});
    let final_usage = json!({
        "choices": [],
        "usage": {"prompt_tokens": 11, "completion_tokens": 3}
    });
    let responses = vec![
        sse_response(&[first_call_fragment, second_call_fragment, first_usage]),
        sse_response(&[final_event, final_usage]),
    ];
    let (base_url, requests, server) = response_fixture_server("text/event-stream", responses);
    let provider = OmlxProvider::with_base_url(&base_url).expect("fixture endpoint should build");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-omlx-clock-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let request = ChatRequest {
        provider_id: "omlx".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "What is the current UTC time?".into(),
            }],
        }],
        memory_enabled: false,
        web_enabled: false,
        email_enabled: false,
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
        store.clone(),
        run_id.clone(),
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        None,
        None,
    ))
    .expect("two-round oMLX clock generation should complete")
    .expect("fixture reports usage");
    store
        .finish_provider_run(
            &run_id,
            ProviderRunState::Completed,
            None,
            Some(StoredUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cost_usd: usage.cost_usd,
            }),
        )
        .expect("fixture run should complete");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Clock grounded answer");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(1));
    assert_eq!(requests[0]["tools"][0]["function"]["name"], "current_time");
    assert_eq!(requests[0]["tool_choice"], "auto");
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(
        requests[1]["messages"][1]["tool_calls"][0]["id"],
        "call_omlx_clock"
    );
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(
        requests[1]["messages"][2]["tool_call_id"],
        "call_omlx_clock"
    );
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains(r#""utc":"#))
    );
    let reopened = store
        .load_conversation(&conversation_id)
        .expect("conversation should reopen");
    let run = reopened.messages[1]
        .provider_run
        .as_ref()
        .expect("assistant should retain its provider run");
    assert_eq!(run.state, ProviderRunState::Completed);
    assert_eq!(run.tool_invocations[0].tool_name, "current_time");
}

#[test]
#[ignore = "requires a running oMLX server on 127.0.0.1:8000"]
fn live_omlx_clock_and_memory_calls_complete_through_bottie() {
    let (store, conversation_id, message_id, run_id) = active_run("omlx");
    let provider = OmlxProvider::new().expect("oMLX provider should build");
    let model = tauri::async_runtime::block_on(provider.discover_models())
        .expect("running oMLX should discover models")
        .into_iter()
        .filter(|model| model.capabilities.tools)
        .min_by_key(|model| {
            (
                !model.model_id.contains("Qwen3.8"),
                !model.model_id.contains("1.2B"),
                model.model_id.clone(),
            )
        })
        .expect("oMLX should expose one explicit tool-capable text model");
    let prompt = format!(
        concat!(
            "Call current_time with an empty object. Also call open_memory with conversationId ",
            "{}, messageId {}, before 0, and after 0. After both native results, ",
            "reply with the four-digit UTC year and the remembered sentence."
        ),
        conversation_id, message_id
    );
    let request = ChatRequest {
        provider_id: "omlx".into(),
        model_id: model.model_id,
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text { text: prompt }],
        }],
        memory_enabled: true,
        web_enabled: false,
        email_enabled: false,
        settings: ChatSettings {
            temperature: Some(0.0),
            max_output_tokens: Some(160),
            reasoning_effort: ReasoningEffort::Off,
        },
    };
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-live-omlx-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );

    let usage = tauri::async_runtime::block_on(stream_omlx_tools(
        provider,
        request,
        sink.clone(),
        store.clone(),
        run_id.clone(),
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
        None,
        None,
    ))
    .expect("live oMLX native-tool loop should complete");
    store
        .finish_provider_run(
            &run_id,
            ProviderRunState::Completed,
            None,
            usage.map(|usage| StoredUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cost_usd: usage.cost_usd,
            }),
        )
        .expect("live run should complete");

    let reopened = store
        .load_conversation(&conversation_id)
        .expect("live audited conversation should reopen");
    let tools = &reopened.messages[1]
        .provider_run
        .as_ref()
        .expect("assistant should retain its run")
        .tool_invocations;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.tool_name.as_str())
            .collect::<Vec<_>>(),
        ["current_time", "open_memory"]
    );
    assert!(tools.iter().all(|tool| {
        tool.audit.outcome == Some(ToolAuditOutcome::Success)
            && tool.result.as_ref().is_some_and(|result| !result.is_error)
    }));
    let clock = tools
        .iter()
        .find(|tool| tool.tool_name == "current_time")
        .and_then(|tool| tool.result.as_ref())
        .expect("clock result should reopen");
    assert!(
        clock.output["result"]["utc"]
            .as_str()
            .is_some_and(|utc| utc.starts_with(&Utc::now().year().to_string()))
    );
    let memory = tools
        .iter()
        .find(|tool| tool.tool_name == "open_memory")
        .and_then(|tool| tool.result.as_ref())
        .expect("memory result should reopen");
    assert!(memory.output.to_string().contains("Open this exact memory"));
    assert!(
        !sink.text.lock().unwrap().trim().is_empty(),
        "live oMLX should return a final answer after the native results"
    );
}
