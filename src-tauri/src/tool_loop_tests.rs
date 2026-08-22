//! Provider-neutral bounded native tool-loop policy tests.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{
    storage::{ConversationStore, MessageState, NewStoredMessage, SemanticEmbedder, StoredRole},
    tool_dispatch::{MemoryToolExecution, bounded_memory_tool_success},
    tool_loop::{
        MAX_TOOL_LOOP_CALLS, MAX_TOOL_LOOP_OUTPUT_BYTES, MAX_TOOL_LOOP_ROUNDS, NativeToolCall,
        TOOL_LOOP_TIMEOUT, ToolLoopCancellation, ToolLoopErrorCode, ToolLoopState,
    },
};

/// Embedding dimensions fixed by Bottie's active EmbeddingGemma contract.
const TEST_EMBEDDING_DIMENSIONS: usize = 768;

/// Deterministic embedding boundary for the dispatcher integration fixture.
#[derive(Default)]
struct LoopEmbedder;

impl SemanticEmbedder for LoopEmbedder {
    /// Produces one valid fixed-size vector per input.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts
            .iter()
            .map(|_| vec![0.0; TEST_EMBEDDING_DIMENSIONS])
            .collect())
    }
}

/// Creates one isolated initialized store for tool-loop fixtures.
fn test_store() -> ConversationStore {
    let path = std::env::temp_dir()
        .join("bottie-tool-loop-tests")
        .join(format!("{}.sqlite3", uuid::Uuid::new_v4()));
    ConversationStore::initialize(path).expect("tool-loop fixture store should initialize")
}

/// Builds one provider-neutral raw tool call with a small valid argument object.
fn call(call_id: impl Into<String>, tool_name: &str) -> NativeToolCall {
    NativeToolCall {
        call_id: call_id.into(),
        tool_name: tool_name.into(),
        arguments: json!({"query": "bounded memory", "limit": 1}),
    }
}

#[test]
fn executes_multiple_calls_across_bounded_rounds_through_the_dispatcher() {
    let store = test_store();
    let conversation = store
        .create_conversation("Tool loop fixture")
        .expect("conversation should create");
    let message = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Keep tool recursion bounded.".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("fixture message should append");
    let started_at = Instant::now();
    let mut state = ToolLoopState::new(started_at);
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = LoopEmbedder;

    let search = state
        .execute_memory_round(
            &store,
            &mut embedder,
            vec![call("call-search", "search_memory")],
            &cancellation,
        )
        .expect("first round should execute");
    assert_eq!(search[0].call_id, "call-search");
    assert!(matches!(
        search[0].execution,
        MemoryToolExecution::Success { .. }
    ));

    let opened = state
        .execute_memory_round(
            &store,
            &mut embedder,
            vec![NativeToolCall {
                call_id: "call-open".into(),
                tool_name: "open_memory".into(),
                arguments: json!({
                    "conversationId": conversation.id,
                    "messageId": message.id,
                    "before": 0,
                    "after": 0
                }),
            }],
            &cancellation,
        )
        .expect("second round should execute");
    assert_eq!(opened[0].call_id, "call-open");
    assert!(state.complete(&cancellation).is_ok());
    assert_eq!(state.round_count(), 2);
    assert_eq!(state.call_count(), 2);
    assert!(state.aggregate_output_bytes() > 0);
}

#[test]
fn rejects_call_and_recursion_limits_before_executing_an_excess_round() {
    let started_at = Instant::now();
    let cancellation = ToolLoopCancellation::default();
    let executions = Arc::new(AtomicUsize::new(0));
    let mut calls_state = ToolLoopState::new(started_at);
    let maximum_calls = (0..MAX_TOOL_LOOP_CALLS)
        .map(|index| call(format!("call-{index}"), "search_memory"))
        .collect();

    calls_state
        .execute_round_with(maximum_calls, &cancellation, Instant::now, |_| {
            executions.fetch_add(1, Ordering::SeqCst);
            bounded_memory_tool_success(json!({"matches": []}))
        })
        .expect("maximum call batch should execute");
    let error = calls_state
        .execute_round_with(
            vec![call("excess", "search_memory")],
            &cancellation,
            Instant::now,
            |_| panic!("excess call must not execute"),
        )
        .expect_err("call ceiling should terminate the loop");
    assert_eq!(error.code, ToolLoopErrorCode::CallLimitExceeded);
    assert_eq!(executions.load(Ordering::SeqCst), MAX_TOOL_LOOP_CALLS);

    let mut rounds_state = ToolLoopState::new(started_at);
    for round in 0..MAX_TOOL_LOOP_ROUNDS {
        rounds_state
            .execute_round_with(
                vec![call(format!("round-{round}"), "search_memory")],
                &cancellation,
                Instant::now,
                |_| bounded_memory_tool_success(json!({"matches": []})),
            )
            .expect("allowed recursion round should execute");
    }
    let error = rounds_state
        .execute_round_with(
            vec![call("excess-round", "search_memory")],
            &cancellation,
            Instant::now,
            |_| panic!("excess recursion round must not execute"),
        )
        .expect_err("recursion ceiling should terminate the loop");
    assert_eq!(error.code, ToolLoopErrorCode::RecursionLimitExceeded);
}

#[test]
fn stops_before_returning_aggregate_output_beyond_the_loop_ceiling() {
    let started_at = Instant::now();
    let cancellation = ToolLoopCancellation::default();
    let mut state = ToolLoopState::new(started_at);
    let per_call_bytes = MAX_TOOL_LOOP_OUTPUT_BYTES / 2;
    let calls = vec![
        call("large-1", "search_memory"),
        call("large-2", "search_memory"),
        call("large-3", "search_memory"),
    ];

    let error = state
        .execute_round_with(calls, &cancellation, Instant::now, |_| {
            MemoryToolExecution::Success {
                result: json!({"value": "x".repeat(per_call_bytes)}),
            }
        })
        .expect_err("aggregate output ceiling should terminate the loop");
    assert_eq!(error.code, ToolLoopErrorCode::AggregateOutputExceeded);
    assert!(state.aggregate_output_bytes() <= MAX_TOOL_LOOP_OUTPUT_BYTES);
}

#[test]
fn propagates_timeout_and_cancellation_before_additional_tool_work() {
    let started_at = Instant::now();
    let cancellation = ToolLoopCancellation::default();
    let mut timed_out = ToolLoopState::new(started_at);
    let error = timed_out
        .execute_round_with(
            vec![call("late", "search_memory")],
            &cancellation,
            || started_at + TOOL_LOOP_TIMEOUT + Duration::from_millis(1),
            |_| panic!("timed-out call must not execute"),
        )
        .expect_err("expired deadline should terminate the loop");
    assert_eq!(error.code, ToolLoopErrorCode::TimedOut);

    let cancellation = ToolLoopCancellation::default();
    let mut cancelled = ToolLoopState::new(started_at);
    let calls = vec![
        call("first", "search_memory"),
        call("second", "search_memory"),
    ];
    let executions = AtomicUsize::new(0);
    let error = cancelled
        .execute_round_with(calls, &cancellation, Instant::now, |_| {
            executions.fetch_add(1, Ordering::SeqCst);
            cancellation.cancel();
            bounded_memory_tool_success(Value::Null)
        })
        .expect_err("cancellation should terminate the active round");
    assert_eq!(error.code, ToolLoopErrorCode::Cancelled);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

#[test]
fn terminal_loops_reject_later_rounds_without_restarting_execution() {
    let started_at = Instant::now();
    let cancellation = ToolLoopCancellation::default();
    let mut state = ToolLoopState::new(started_at);
    state
        .complete(&cancellation)
        .expect("fresh loop should complete");

    let error = state
        .execute_round_with(
            vec![call("late", "search_memory")],
            &cancellation,
            Instant::now,
            |_| panic!("terminal loop must not restart"),
        )
        .expect_err("terminal state should stay closed");
    assert_eq!(error.code, ToolLoopErrorCode::InvalidState);
}

#[test]
fn completion_still_honors_cancellation_and_the_overall_deadline() {
    let started_at = Instant::now();
    let cancellation = ToolLoopCancellation::default();
    cancellation.cancel();
    let mut cancelled = ToolLoopState::new(started_at);
    let error = cancelled
        .complete_with(&cancellation, started_at)
        .expect_err("cancelled loop must not complete normally");
    assert_eq!(error.code, ToolLoopErrorCode::Cancelled);

    let cancellation = ToolLoopCancellation::default();
    let mut timed_out = ToolLoopState::new(started_at);
    let error = timed_out
        .complete_with(&cancellation, started_at + TOOL_LOOP_TIMEOUT)
        .expect_err("expired loop must not complete normally");
    assert_eq!(error.code, ToolLoopErrorCode::TimedOut);
}
