//! Provider-neutral native memory-tool dispatcher tests.

use serde_json::{Value, json};

use crate::{
    storage::{ConversationStore, MessageState, NewStoredMessage, SemanticEmbedder, StoredRole},
    tool_dispatch::{
        MAX_MEMORY_TOOL_OUTPUT_BYTES, MemoryToolExecution, MemoryToolExecutionErrorCode,
        bounded_memory_tool_success, dispatch_memory_tool,
    },
    tool_loop::NativeToolCall,
};

/// Embedding dimensions fixed by Bottie's active EmbeddingGemma contract.
const TEST_EMBEDDING_DIMENSIONS: usize = 768;

/// Deterministic embedding boundary that records whether native model work occurred.
#[derive(Default)]
struct DispatchEmbedder {
    inputs: Vec<String>,
    fail: bool,
}

impl SemanticEmbedder for DispatchEmbedder {
    /// Produces one valid fixed-size vector unless the fixture requests a runtime failure.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.inputs.extend(texts.iter().cloned());
        if self.fail {
            return Err("fixture details must stay private".into());
        }
        Ok(texts
            .iter()
            .map(|_| {
                let mut embedding = vec![0.0; TEST_EMBEDDING_DIMENSIONS];
                embedding[0] = 1.0;
                embedding
            })
            .collect())
    }
}

/// Creates one isolated initialized store for dispatcher fixtures.
fn test_store() -> ConversationStore {
    let path = std::env::temp_dir()
        .join("bottie-tool-dispatch-tests")
        .join(format!("{}.sqlite3", uuid::Uuid::new_v4()));
    ConversationStore::initialize(path).expect("dispatcher fixture store should initialize")
}

/// Extracts a successful result object while asserting the common envelope shape.
fn success_result(execution: MemoryToolExecution) -> Value {
    assert!(
        serde_json::to_vec(&execution)
            .is_ok_and(|serialized| serialized.len() <= MAX_MEMORY_TOOL_OUTPUT_BYTES)
    );
    let serialized = serde_json::to_value(execution).expect("execution should serialize");
    assert_eq!(serialized["ok"], json!(true));
    assert!(serialized.get("error").is_none());
    serialized["result"].clone()
}

/// Runs one unapproved provider call through the mandatory native policy boundary.
fn dispatch(
    store: &ConversationStore,
    embedder: &mut impl SemanticEmbedder,
    tool_name: &str,
    arguments: Value,
) -> MemoryToolExecution {
    dispatch_memory_tool(
        store,
        embedder,
        &NativeToolCall {
            call_id: "provider-call".into(),
            tool_name: tool_name.into(),
            arguments,
        },
        None,
    )
}

#[test]
fn dispatches_all_three_validated_memory_tools_into_one_success_envelope() {
    let store = test_store();
    let conversation = store
        .create_conversation("Dispatcher fixture")
        .expect("conversation should create");
    let message = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Keep native memory bounded.".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("fixture message should append");
    let mut embedder = DispatchEmbedder::default();

    let search = success_result(dispatch(
        &store,
        &mut embedder,
        "search_memory",
        json!({"query": "missing phrase", "limit": 1}),
    ));
    assert_eq!(search["matches"], json!([]));

    let opened = success_result(dispatch(
        &store,
        &mut embedder,
        "open_memory",
        json!({
            "conversationId": conversation.id,
            "messageId": message.id,
            "before": 0,
            "after": 0
        }),
    ));
    assert_eq!(opened["turns"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        opened["turns"][0]["text"],
        json!("Keep native memory bounded.")
    );

    let files = success_result(dispatch(
        &store,
        &mut embedder,
        "search_attached_files",
        json!({"query": "missing file", "limit": 1}),
    ));
    assert_eq!(files["matches"], json!([]));
    assert_eq!(embedder.inputs.len(), 2);
}

#[test]
fn rejects_names_and_arguments_before_storage_or_embedding_work() {
    let store = test_store();
    let mut embedder = DispatchEmbedder::default();

    for (name, arguments, expected) in [
        (
            "web_fetch",
            json!({"url": "file:///private/secret"}),
            MemoryToolExecutionErrorCode::UnsupportedTool,
        ),
        (
            "search_memory",
            json!({"query": "north", "includePaths": true}),
            MemoryToolExecutionErrorCode::InvalidArguments,
        ),
    ] {
        let execution = dispatch(&store, &mut embedder, name, arguments.clone());
        let serialized = serde_json::to_value(&execution).expect("error should serialize");
        assert_eq!(serialized["ok"], json!(false));
        assert!(serialized.get("result").is_none());
        assert_eq!(
            serialized["error"]["code"],
            serde_json::to_value(expected).expect("error code should serialize")
        );
        let MemoryToolExecution::Error { ref error } = execution else {
            panic!("invalid calls must return an error envelope");
        };
        assert_eq!(error.code, expected);
        assert!(!error.message.contains(&arguments.to_string()));
    }
    assert!(embedder.inputs.is_empty());
}

#[test]
fn redacts_storage_and_embedding_failures_into_stable_categories() {
    let store = test_store();
    let mut embedder = DispatchEmbedder::default();
    let missing = dispatch(
        &store,
        &mut embedder,
        "open_memory",
        json!({"conversationId": "missing", "messageId": "missing"}),
    );
    let MemoryToolExecution::Error { error } = missing else {
        panic!("missing provenance should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::Unavailable);
    assert!(!error.message.contains("missing"));

    embedder.fail = true;
    let failed = dispatch(
        &store,
        &mut embedder,
        "search_memory",
        json!({"query": "north"}),
    );
    let MemoryToolExecution::Error { error } = failed else {
        panic!("embedding failure should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::ExecutionFailed);
    assert!(!error.message.contains("fixture"));
}

#[test]
fn enforces_the_serialized_result_ceiling_before_returning_success() {
    let oversized = json!({"value": "x".repeat(MAX_MEMORY_TOOL_OUTPUT_BYTES)});
    let execution = bounded_memory_tool_success(oversized);
    let MemoryToolExecution::Error { ref error } = execution else {
        panic!("oversized results must return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::OutputTooLarge);

    let serialized = serde_json::to_vec(&execution).expect("bounded error should serialize");
    assert!(serialized.len() < MAX_MEMORY_TOOL_OUTPUT_BYTES);
}
