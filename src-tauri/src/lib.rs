mod inference;

use std::{collections::HashMap, sync::Arc};

use futures_util::future::{AbortHandle, Abortable};
use inference::{
    ChatRequest, ChatRun, InferenceProvider, ModelInfo, OllamaProvider, OmlxProvider,
    ProviderError, StreamEvent, Usage,
};
use serde::Serialize;
use tauri::{State, ipc::Channel};

type ActiveRuns = Arc<tauri::async_runtime::Mutex<HashMap<String, AbortHandle>>>;

struct AppState {
    omlx: OmlxProvider,
    ollama: OllamaProvider,
    runs: ActiveRuns,
}

#[derive(Clone)]
enum LocalProvider {
    Omlx(OmlxProvider),
    Ollama(OllamaProvider),
}

impl LocalProvider {
    fn provider_id(&self) -> &'static str {
        match self {
            Self::Omlx(_) => "omlx",
            Self::Ollama(_) => "ollama",
        }
    }

    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl inference::StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        match self {
            Self::Omlx(provider) => provider.stream_chat(request, sink).await,
            Self::Ollama(provider) => provider.stream_chat(request, sink).await,
        }
    }
}

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    storage: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "bottie",
        version: env!("CARGO_PKG_VERSION"),
        storage: "local",
    }
}

#[tauri::command]
async fn discover_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, ProviderError> {
    let (omlx, ollama) =
        futures_util::future::join(state.omlx.discover_models(), state.ollama.discover_models())
            .await;
    let mut models = Vec::new();
    let mut errors = Vec::new();
    match omlx {
        Ok(mut discovered) => models.append(&mut discovered),
        Err(error) => errors.push(format!("oMLX: {}", error.message)),
    }
    match ollama {
        Ok(mut discovered) => models.append(&mut discovered),
        Err(error) => errors.push(format!("Ollama: {}", error.message)),
    }
    models.sort_by(|left, right| {
        left.provider_name
            .cmp(&right.provider_name)
            .then(left.display_name.cmp(&right.display_name))
    });
    if models.is_empty() {
        Err(ProviderError::unavailable(
            "No local inference provider is available. Start oMLX or Ollama and try again.",
            (!errors.is_empty()).then(|| errors.join("; ")),
        ))
    } else {
        Ok(models)
    }
}

struct ChannelSink {
    run_id: String,
    channel: Channel<StreamEvent>,
}

impl inference::StreamSink for ChannelSink {
    fn text_delta(&self, delta: String) -> Result<(), ProviderError> {
        self.channel
            .send(StreamEvent::TextDelta {
                run_id: self.run_id.clone(),
                delta,
            })
            .map_err(|error| {
                ProviderError::internal(
                    "The inference stream could not reach the interface.",
                    Some(error.to_string()),
                )
            })
    }

    fn usage_updated(&self, usage: Usage) -> Result<(), ProviderError> {
        self.channel
            .send(StreamEvent::UsageUpdated {
                run_id: self.run_id.clone(),
                usage,
            })
            .map_err(|error| {
                ProviderError::internal(
                    "Usage information could not reach the interface.",
                    Some(error.to_string()),
                )
            })
    }
}

#[tauri::command]
async fn start_chat(
    state: State<'_, AppState>,
    request: ChatRequest,
    on_event: Channel<StreamEvent>,
) -> Result<ChatRun, ProviderError> {
    let provider = match request.provider_id.as_str() {
        "omlx" => LocalProvider::Omlx(state.omlx.clone()),
        "ollama" => LocalProvider::Ollama(state.ollama.clone()),
        _ => {
            return Err(ProviderError::invalid_request(
                "Choose a supported local provider before sending.",
            ));
        }
    };
    let run_id = uuid::Uuid::new_v4().to_string();
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    state.runs.lock().await.insert(run_id.clone(), abort_handle);

    let runs = state.runs.clone();
    let task_run_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = on_event.send(StreamEvent::Started {
            run_id: task_run_id.clone(),
            provider_id: provider.provider_id().into(),
            model_id: request.model_id.clone(),
        });
        let sink = ChannelSink {
            run_id: task_run_id.clone(),
            channel: on_event.clone(),
        };
        match Abortable::new(provider.stream_chat(request, sink), abort_registration).await {
            Ok(Ok(usage)) => {
                let _ = on_event.send(StreamEvent::Completed {
                    run_id: task_run_id.clone(),
                    usage,
                });
            }
            Ok(Err(error)) => {
                let _ = on_event.send(StreamEvent::Failed {
                    run_id: task_run_id.clone(),
                    error,
                });
            }
            Err(_) => {
                let _ = on_event.send(StreamEvent::Cancelled {
                    run_id: task_run_id.clone(),
                });
            }
        }
        runs.lock().await.remove(&task_run_id);
    });

    Ok(ChatRun { run_id })
}

#[tauri::command]
async fn cancel_chat(run_id: String, state: State<'_, AppState>) -> Result<bool, ProviderError> {
    if let Some(handle) = state.runs.lock().await.remove(&run_id) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let omlx = OmlxProvider::new().expect("the built-in oMLX configuration must be valid");
    let ollama = OllamaProvider::new().expect("the built-in Ollama configuration must be valid");
    tauri::Builder::default()
        .manage(AppState {
            omlx,
            ollama,
            runs: Arc::new(tauri::async_runtime::Mutex::new(HashMap::new())),
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            discover_models,
            start_chat,
            cancel_chat
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
