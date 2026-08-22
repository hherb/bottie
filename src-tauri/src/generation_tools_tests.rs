//! Ollama generation-tool persistence and correlation tests.

use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use serde_json::json;

use crate::{
    diagnostics::Diagnostics,
    generation_tools::{execute_ollama_memory_round, stream_ollama_memory_tools},
    inference::{
        ChatRequest, ChatRole, ChatSettings, ChatTurn, ContentBlock, OllamaProvider,
        OllamaToolCall, ProviderError, ReasoningEffort, StreamSink, Usage,
    },
    semantic_indexer::SemanticIndexer,
    storage::{
        ConversationStore, MessageState, NewProviderRun, NewStoredMessage, ProviderRunState,
        SemanticEmbedder, StoredReasoningEffort, StoredRole,
    },
    tool_loop::{ToolLoopCancellation, ToolLoopState},
};

/// Fixed dimensions required by the semantic query boundary when a search tool is exercised.
const TEST_EMBEDDING_DIMENSIONS: usize = 768;

/// Deterministic query embedding boundary for generation orchestration tests.
struct GenerationToolEmbedder;

impl SemanticEmbedder for GenerationToolEmbedder {
    /// Returns one correctly sized vector for every normalized query input.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts
            .iter()
            .map(|_| vec![0.0; TEST_EMBEDDING_DIMENSIONS])
            .collect())
    }
}

/// Thread-safe sink used to verify normalized text and cumulative usage across provider rounds.
#[derive(Clone, Default)]
struct RecordingSink {
    text: Arc<Mutex<String>>,
    usage: Arc<Mutex<Option<Usage>>>,
}

impl StreamSink for RecordingSink {
    /// Retains normalized answer text from every Ollama round.
    fn text_delta(&self, delta: String) -> Result<(), ProviderError> {
        self.text.lock().unwrap().push_str(&delta);
        Ok(())
    }

    /// Ignores reasoning content that is not part of this fixture.
    fn reasoning_delta(&self, _delta: String) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Retains the latest cumulative usage checkpoint.
    fn usage_updated(&self, usage: Usage) -> Result<(), ProviderError> {
        *self.usage.lock().unwrap() = Some(usage);
        Ok(())
    }
}

/// Creates one active Ollama provider run that can retain tool checkpoints.
fn active_run() -> (ConversationStore, String, String, String) {
    let path = std::env::temp_dir()
        .join("bottie-generation-tool-tests")
        .join(format!("{}.sqlite3", uuid::Uuid::new_v4()));
    let store = ConversationStore::initialize(path).expect("fixture store should initialize");
    let conversation = store
        .create_conversation("Ollama tool generation")
        .expect("conversation should create");
    let request = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Open this exact memory".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("request should append");
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: request.id.clone(),
            provider_id: "ollama".into(),
            model_id: "tool-model".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: Some(512),
        })
        .expect("provider run should start");
    (store, conversation.id, request.id, run_id)
}

#[test]
fn executes_and_persists_an_ollama_tool_round_before_returning_results() {
    let (store, conversation_id, message_id, run_id) = active_run();
    let mut state = ToolLoopState::new(Instant::now());
    let cancellation = ToolLoopCancellation::default();
    let mut embedder = GenerationToolEmbedder;

    let results = execute_ollama_memory_round(
        &store,
        &run_id,
        &mut embedder,
        &mut state,
        vec![OllamaToolCall::fixture(
            0,
            "open_memory",
            json!({
                "conversationId": conversation_id,
                "messageId": message_id,
                "before": 0,
                "after": 0
            }),
        )],
        &cancellation,
    )
    .expect("tool round should execute");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_name, "open_memory");
    assert!(results[0].content.contains(r#""ok":true"#));
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("run should complete");
    let conversation = store
        .load_conversation(&conversation_id)
        .expect("conversation should reload");
    let tools = &conversation.messages[1]
        .provider_run
        .as_ref()
        .expect("response should retain its run")
        .tool_invocations;

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].tool_name, "open_memory");
    assert_eq!(tools[0].arguments["messageId"], message_id);
    assert!(
        tools[0]
            .result
            .as_ref()
            .is_some_and(|result| !result.is_error)
    );
    assert_eq!(state.call_count(), 1);
}

#[test]
#[ignore = "requires loopback fixture access"]
fn streams_an_ollama_tool_call_result_and_final_answer_across_two_requests() {
    let (store, conversation_id, message_id, run_id) = active_run();
    let tool_chunk = serde_json::json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "index": 0,
                    "name": "open_memory",
                    "arguments": {
                        "conversationId": conversation_id,
                        "messageId": message_id,
                        "before": 0,
                        "after": 0
                    }
                }
            }]
        },
        "done": true,
        "prompt_eval_count": 7,
        "eval_count": 2
    });
    let final_chunk = serde_json::json!({
        "message": {"role": "assistant", "content": "Final answer"},
        "done": true,
        "prompt_eval_count": 11,
        "eval_count": 3
    });
    let (base_url, requests, server) = fixture_server(vec![tool_chunk, final_chunk]);
    let provider =
        OllamaProvider::with_base_url(&base_url).expect("fixture endpoint should validate");
    let sink = RecordingSink::default();
    let semantic_indexer = SemanticIndexer::start(
        std::env::temp_dir().join(format!("bottie-tool-model-{}", uuid::Uuid::new_v4())),
        store.clone(),
        Diagnostics::default(),
    );
    let request = ChatRequest {
        provider_id: "ollama".into(),
        model_id: "tool-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![ContentBlock::Text {
                text: "Open this exact memory".into(),
            }],
        }],
        memory_enabled: true,
        settings: ChatSettings {
            temperature: Some(0.0),
            max_output_tokens: Some(128),
            reasoning_effort: ReasoningEffort::Off,
        },
    };

    let usage = tauri::async_runtime::block_on(stream_ollama_memory_tools(
        provider,
        request,
        sink.clone(),
        store,
        run_id,
        semantic_indexer.query_embedder(),
        ToolLoopCancellation::default(),
    ))
    .expect("two-round Ollama generation should complete")
    .expect("fixture reports usage");
    server.join().expect("fixture server should finish");

    assert_eq!(sink.text.lock().unwrap().as_str(), "Final answer");
    assert_eq!(usage.input_tokens, Some(18));
    assert_eq!(usage.output_tokens, Some(5));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["tools"].as_array().map(Vec::len), Some(3));
    assert_eq!(requests[1]["messages"][1]["role"], "assistant");
    assert_eq!(requests[1]["messages"][2]["role"], "tool");
    assert_eq!(requests[1]["messages"][2]["tool_name"], "open_memory");
    assert!(
        requests[1]["messages"][2]["content"]
            .as_str()
            .is_some_and(|content| content.contains(r#""ok":true"#))
    );
}

/// Starts a two-request loopback HTTP fixture and records each decoded JSON request body.
fn fixture_server(
    responses: Vec<serde_json::Value>,
) -> (
    String,
    Arc<Mutex<Vec<serde_json::Value>>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener should bind");
    let address = listener
        .local_addr()
        .expect("fixture address should resolve");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let server = thread::spawn(move || {
        let mut responses = VecDeque::from(responses);
        while let Some(response) = responses.pop_front() {
            let (mut stream, _) = listener.accept().expect("fixture request should connect");
            let request = read_json_request(&mut stream);
            recorded.lock().unwrap().push(request);
            let body = format!("{}\n", serde_json::to_string(&response).unwrap());
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("fixture response should write");
        }
    });
    (format!("http://{address}/"), requests, server)
}

/// Reads one content-length framed JSON request from the loopback HTTP fixture.
fn read_json_request(stream: &mut TcpStream) -> serde_json::Value {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4_096];
    loop {
        let count = stream
            .read(&mut chunk)
            .expect("fixture request should read");
        bytes.extend_from_slice(&chunk[..count]);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let body_start = header_end + 4;
        let headers = std::str::from_utf8(&bytes[..header_end]).expect("headers should be UTF-8");
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(str::to_owned)
            })
            .and_then(|length| length.parse::<usize>().ok())
            .expect("request should include content length");
        if bytes.len() >= body_start + content_length {
            return serde_json::from_slice(&bytes[body_start..body_start + content_length])
                .expect("request body should be JSON");
        }
    }
}
