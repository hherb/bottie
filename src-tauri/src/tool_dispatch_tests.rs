//! Provider-neutral native memory-tool dispatcher tests.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::{
    storage::{ConversationStore, MessageState, NewStoredMessage, SemanticEmbedder, StoredRole},
    tool_dispatch::{
        MAX_MEMORY_TOOL_OUTPUT_BYTES, MemoryToolExecution, MemoryToolExecutionErrorCode,
        bounded_memory_tool_success, dispatch_memory_tool, dispatch_web_fetch_tool,
        dispatch_web_search_tool,
    },
    tool_loop::NativeToolCall,
    web_fetch::{
        WebFetchError, WebFetchProvider, WebFetchRequest, WebFetchResponse,
        fixture_web_fetch_response, fixture_web_fetch_response_at,
    },
    web_policy::WebNetworkPolicy,
    web_search::{
        WebSearchError, WebSearchProvider, WebSearchRequest, WebSearchResponse,
        fixture_web_search_response,
    },
};

/// Embedding dimensions fixed by Bottie's active EmbeddingGemma contract.
const TEST_EMBEDDING_DIMENSIONS: usize = 768;
/// Deterministic embedding boundary that records whether native model work occurred.
#[derive(Default)]
struct DispatchEmbedder {
    inputs: Vec<String>,
    fail: bool,
}

/// Deterministic provider-neutral web-search fixture that records validated requests.
#[derive(Clone)]
struct DispatchSearchProvider {
    requests: Arc<Mutex<Vec<WebSearchRequest>>>,
    result: Result<WebSearchResponse, WebSearchError>,
}

/// Deterministic native web-fetch fixture that records validated requests.
#[derive(Clone)]
struct DispatchFetchProvider {
    requests: Arc<Mutex<Vec<WebFetchRequest>>>,
    result: Result<WebFetchResponse, WebFetchError>,
}

impl WebFetchProvider for DispatchFetchProvider {
    async fn fetch(&self, request: WebFetchRequest) -> Result<WebFetchResponse, WebFetchError> {
        self.requests.lock().unwrap().push(request);
        self.result.clone()
    }
}

impl WebSearchProvider for DispatchSearchProvider {
    /// Uses a stable fixture identity without depending on the concrete Brave adapter.
    fn provider_id(&self) -> &'static str {
        "fixture"
    }

    /// Records the native request and returns the configured bounded outcome.
    async fn search(&self, request: WebSearchRequest) -> Result<WebSearchResponse, WebSearchError> {
        self.requests.lock().unwrap().push(request);
        self.result.clone()
    }
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

#[test]
fn dispatches_validated_web_search_through_the_provider_neutral_boundary() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = DispatchSearchProvider {
        requests: requests.clone(),
        result: Ok(fixture_web_search_response()),
    };
    let call = NativeToolCall {
        call_id: "provider-call".into(),
        tool_name: "web_search".into(),
        arguments: json!({
            "query": "current Rust release",
            "freshness": "month",
            "includeDomains": ["rust-lang.org"],
            "limit": 3
        }),
    };

    let execution = tauri::async_runtime::block_on(dispatch_web_search_tool(
        &provider,
        &call,
        &WebNetworkPolicy::default(),
        None,
    ));
    let result = success_result(execution);
    assert_eq!(result["providerId"], json!("fixture"));
    assert_eq!(
        result["results"][0]["url"],
        json!("https://example.com/result")
    );

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].query(), "current Rust release");
    assert_eq!(requests[0].result_limit(), 3);
    assert_eq!(requests[0].include_domains(), &["rust-lang.org"]);
    assert!(requests[0].exclude_domains().is_empty());
}

#[test]
fn rejects_invalid_web_search_before_network_work_and_redacts_provider_failures() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = DispatchSearchProvider {
        requests: requests.clone(),
        result: Err(WebSearchError {
            code: crate::web_search::WebSearchErrorCode::Unavailable,
            message: "provider body private query".into(),
            retryable: true,
        }),
    };
    let invalid_call = NativeToolCall {
        call_id: "provider-call".into(),
        tool_name: "web_search".into(),
        arguments: json!({"query": "private query", "includeDomains": ["https://invalid"]}),
    };
    let invalid = tauri::async_runtime::block_on(dispatch_web_search_tool(
        &provider,
        &invalid_call,
        &WebNetworkPolicy::default(),
        None,
    ));
    let MemoryToolExecution::Error { error } = invalid else {
        panic!("invalid search should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::InvalidArguments);
    assert!(requests.lock().unwrap().is_empty());

    let failed_call = NativeToolCall {
        call_id: "provider-call".into(),
        tool_name: "web_search".into(),
        arguments: json!({"query": "private query"}),
    };
    let failed = tauri::async_runtime::block_on(dispatch_web_search_tool(
        &provider,
        &failed_call,
        &WebNetworkPolicy::default(),
        None,
    ));
    let MemoryToolExecution::Error { error } = failed else {
        panic!("provider failure should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::Unavailable);
    assert!(!error.message.contains("private"));
    assert!(!error.message.contains("provider body"));
}

#[test]
fn dispatches_validated_web_fetch_through_the_bounded_native_boundary() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = DispatchFetchProvider {
        requests: requests.clone(),
        result: Ok(fixture_web_fetch_response()),
    };
    let call = NativeToolCall {
        call_id: "provider-call".into(),
        tool_name: "web_fetch".into(),
        arguments: json!({"url": "https://www.iana.org/release#notes"}),
    };

    let execution = tauri::async_runtime::block_on(dispatch_web_fetch_tool(
        &provider,
        &call,
        &WebNetworkPolicy::default(),
        None,
    ));
    let result = success_result(execution);
    assert_eq!(result["sourceUrl"], json!("https://www.iana.org/release"));
    assert_eq!(result["title"], json!("IANA release"));
    assert_eq!(result["publishedAt"], json!("2026-08-23"));
    assert_eq!(result["content"], json!("Bounded fixture page."));
    assert_eq!(result["untrusted"], json!(true));

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url(), "https://www.iana.org/release");
}

#[test]
fn rejects_invalid_web_fetch_before_network_work_and_redacts_failures() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = DispatchFetchProvider {
        requests: requests.clone(),
        result: Err(WebFetchError::unavailable()),
    };
    let invalid_call = NativeToolCall {
        call_id: "provider-call".into(),
        tool_name: "web_fetch".into(),
        arguments: json!({"url": "http://127.0.0.1/private"}),
    };
    let invalid = tauri::async_runtime::block_on(dispatch_web_fetch_tool(
        &provider,
        &invalid_call,
        &WebNetworkPolicy::default(),
        None,
    ));
    let MemoryToolExecution::Error { error } = invalid else {
        panic!("invalid fetch should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::InvalidArguments);
    assert!(requests.lock().unwrap().is_empty());

    let failed_call = NativeToolCall {
        call_id: "provider-call".into(),
        tool_name: "web_fetch".into(),
        arguments: json!({"url": "https://www.iana.org/private"}),
    };
    let failed = tauri::async_runtime::block_on(dispatch_web_fetch_tool(
        &provider,
        &failed_call,
        &WebNetworkPolicy::default(),
        None,
    ));
    let MemoryToolExecution::Error { error } = failed else {
        panic!("fetch failure should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::Unavailable);
    assert!(!error.message.contains("private"));
}

#[test]
fn applies_saved_web_policy_before_results_cross_the_dispatch_boundary() {
    let search_requests = Arc::new(Mutex::new(Vec::new()));
    let search_provider = DispatchSearchProvider {
        requests: search_requests.clone(),
        result: Ok(fixture_web_search_response()),
    };
    let search_call = NativeToolCall {
        call_id: "search-call".into(),
        tool_name: "web_search".into(),
        arguments: json!({"query": "bounded policy"}),
    };
    let policy = WebNetworkPolicy {
        blocked_domains: vec!["example.com".into()],
        ..WebNetworkPolicy::default()
    }
    .normalized()
    .unwrap();
    let search = tauri::async_runtime::block_on(dispatch_web_search_tool(
        &search_provider,
        &search_call,
        &policy,
        None,
    ));
    assert_eq!(success_result(search)["results"], json!([]));
    assert_eq!(search_requests.lock().unwrap().len(), 1);

    let fetch_requests = Arc::new(Mutex::new(Vec::new()));
    let fetch_provider = DispatchFetchProvider {
        requests: fetch_requests.clone(),
        result: Ok(fixture_web_fetch_response()),
    };
    let fetch_call = NativeToolCall {
        call_id: "fetch-call".into(),
        tool_name: "web_fetch".into(),
        arguments: json!({"url": "https://www.iana.org/release"}),
    };
    let policy = WebNetworkPolicy {
        blocked_domains: vec!["iana.org".into()],
        ..WebNetworkPolicy::default()
    }
    .normalized()
    .unwrap();
    let fetch = tauri::async_runtime::block_on(dispatch_web_fetch_tool(
        &fetch_provider,
        &fetch_call,
        &policy,
        None,
    ));
    let MemoryToolExecution::Error { error } = fetch else {
        panic!("blocked fetch should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::InvalidArguments);
    assert!(fetch_requests.lock().unwrap().is_empty());
    assert!(!error.message.contains("iana"));

    let final_requests = Arc::new(Mutex::new(Vec::new()));
    let final_provider = DispatchFetchProvider {
        requests: final_requests.clone(),
        result: Ok(fixture_web_fetch_response_at(
            "https://example.com/redirected",
        )),
    };
    let policy = WebNetworkPolicy {
        allowed_domains: vec!["iana.org".into()],
        ..WebNetworkPolicy::default()
    }
    .normalized()
    .unwrap();
    let final_result = tauri::async_runtime::block_on(dispatch_web_fetch_tool(
        &final_provider,
        &fetch_call,
        &policy,
        None,
    ));
    let MemoryToolExecution::Error { error } = final_result else {
        panic!("policy-mismatched final URL should return an error envelope");
    };
    assert_eq!(error.code, MemoryToolExecutionErrorCode::InvalidArguments);
    assert_eq!(final_requests.lock().unwrap().len(), 1);
    assert!(!error.message.contains("example"));
}
