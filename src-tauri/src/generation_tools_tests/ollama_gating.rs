//! Ollama request-level tool enablement tests.

use super::*;

#[test]
fn rejects_memory_calls_when_only_web_was_explicitly_enabled() {
    let (store, _conversation_id, _message_id, run_id) = active_run("ollama");
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;
    let web_search = GenerationWebSearchExecutor;

    let results = execute_ollama_tool_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OllamaToolCall::fixture(
            0,
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

    assert!(results[0].content.contains(r#""code":"unsupported_tool""#));
    assert!(!results[0].content.contains("must remain disabled"));
}
