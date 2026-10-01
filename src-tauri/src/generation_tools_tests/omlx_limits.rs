//! Full provider exchanges proving saved budgets reach oMLX and allow long tool sequences.

use super::*;
use crate::{generation_context::request_with_generation_limits, inference::GenerationLimits};

#[test]
#[ignore = "requires loopback fixture access"]
fn configured_omlx_generation_allows_twelve_and_custom_tool_rounds() {
    for rounds in [12, 13] {
        let (store, conversation_id, _, run_id) = active_run("omlx");
        let responses = (0..rounds)
            .map(|round| {
                sse_response(&[json!({
                    "choices": [{"delta": {"tool_calls": ([0, 1].map(|index| json!({
                        "index": index, "id": format!("clock-{round}-{index}"), "type": "function",
                        "function": {"name": "current_time", "arguments": "{}"}
                    })))}}]
                })])
            })
            .chain(std::iter::once(sse_response(&[json!({
                "choices": [{"delta": {"content": "Final answer after every configured round."}}]
            })])))
            .collect();
        let (base_url, requests, server) = response_fixture_server("text/event-stream", responses);
        let provider = OmlxProvider::with_base_url(&base_url).unwrap();
        let sink = RecordingSink::default();
        let indexer = SemanticIndexer::start(
            std::env::temp_dir().join(format!("bottie-limits-model-{}", uuid::Uuid::new_v4())),
            store.clone(),
            Diagnostics::default(),
        );
        let request: ChatRequest = serde_json::from_value(json!({
            "providerId": "omlx", "modelId": "tool-model",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "Check the clock."}]}],
        })).unwrap();
        let limits = GenerationLimits {
            max_tool_rounds: rounds,
            max_tool_calls: rounds * 2,
            max_output_tokens: 12_288,
        };
        tauri::async_runtime::block_on(stream_omlx_tools(
            provider,
            request_with_generation_limits(request, limits),
            sink.clone(),
            store.clone(),
            run_id,
            indexer.query_embedder(),
            ToolLoopCancellation::default(),
            None,
            None,
            None,
            Arc::new(crate::python_approval::PythonApprovalController::default()),
            None,
        ))
        .expect("the configured rounds must reach the final answer");
        server.join().unwrap();
        assert_eq!(
            sink.text.lock().unwrap().as_str(),
            "Final answer after every configured round."
        );
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), rounds + 1);
        assert!(
            requests
                .iter()
                .all(|request| request["max_tokens"] == 12_288)
        );
        assert_eq!(
            requests.last().unwrap()["messages"]
                .as_array()
                .unwrap()
                .len(),
            1 + rounds * 3
        );
        let reopened = store.load_conversation(&conversation_id).unwrap();
        assert_eq!(
            reopened.messages[1]
                .provider_run
                .as_ref()
                .unwrap()
                .tool_invocations
                .len(),
            rounds * 2
        );
    }
}
