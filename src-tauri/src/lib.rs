#![deny(missing_docs)]
//! Native application commands and lifecycle for Bottie's Tauri desktop shell.

mod command_types;
mod diagnostics;
mod inference;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};

use command_types::{AppInfo, ProviderConnectionDraft, ProviderConnectionTest, ProviderSelection};
use diagnostics::{DiagnosticEntry, Diagnostics, record_diagnostic, sanitized};
use futures_util::future::{AbortHandle, Abortable};
use inference::{
    ChatRequest, ChatRun, InferenceProvider, ModelInfo, OllamaProvider, OmlxProvider,
    ProviderError, ProviderSettings, StreamEvent, Usage, load_provider_settings,
    save_provider_settings,
};
use tauri::{Manager, State, ipc::Channel};

type ActiveRuns = Arc<tauri::async_runtime::Mutex<HashMap<String, AbortHandle>>>;
struct AppState {
    providers: tauri::async_runtime::RwLock<ProviderSet>,
    settings_path: PathBuf,
    runs: ActiveRuns,
    diagnostics: Diagnostics,
}

#[derive(Clone)]
struct ProviderSet {
    omlx: OmlxProvider,
    ollama: OllamaProvider,
    settings: ProviderSettings,
}

impl ProviderSet {
    /// Builds a complete local-provider set from validated settings.
    fn from_settings(settings: &ProviderSettings) -> Result<Self, ProviderError> {
        Ok(Self {
            omlx: OmlxProvider::with_base_url(&settings.omlx_base_url)?,
            ollama: OllamaProvider::with_base_url(&settings.ollama_base_url)?,
            settings: settings.clone(),
        })
    }

    /// Returns a snapshot of the active provider settings.
    fn settings(&self) -> ProviderSettings {
        self.settings.clone()
    }

    /// Resolves one supported local provider by stable identity.
    fn provider(&self, provider_id: &str) -> Result<LocalProvider, ProviderError> {
        match provider_id {
            "omlx" => Ok(LocalProvider::Omlx(self.omlx.clone())),
            "ollama" => Ok(LocalProvider::Ollama(self.ollama.clone())),
            _ => Err(ProviderError::invalid_request(
                "Choose a supported local provider.",
            )),
        }
    }
}

#[derive(Clone)]
enum LocalProvider {
    Omlx(OmlxProvider),
    Ollama(OllamaProvider),
}

impl LocalProvider {
    /// Returns the provider's stable routing identity.
    fn provider_id(&self) -> &'static str {
        match self {
            Self::Omlx(_) => "omlx",
            Self::Ollama(_) => "ollama",
        }
    }

    /// Streams a chat through the concrete local provider.
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

    /// Discovers models through the concrete local provider.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        match self {
            Self::Omlx(provider) => provider.discover_models().await,
            Self::Ollama(provider) => provider.discover_models().await,
        }
    }
}

#[tauri::command]
/// Returns static identity information for the running native application.
fn app_info() -> AppInfo {
    AppInfo {
        name: "bottie",
        version: env!("CARGO_PKG_VERSION"),
        storage: "local",
    }
}

#[tauri::command]
/// Returns the active non-secret local-provider settings.
async fn get_provider_settings(
    state: State<'_, AppState>,
) -> Result<ProviderSettings, ProviderError> {
    Ok(state.providers.read().await.settings())
}

#[tauri::command]
/// Validates, persists, and activates new local-provider settings.
async fn update_provider_settings(
    settings: ProviderSettings,
    state: State<'_, AppState>,
) -> Result<ProviderSettings, ProviderError> {
    let settings = settings.normalized()?;
    let providers = ProviderSet::from_settings(&settings)?;
    save_provider_settings(&state.settings_path, &settings)?;
    *state.providers.write().await = providers;
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Local provider settings saved",
        None,
        Some("Loopback endpoints updated; no credentials stored"),
    )
    .await;
    Ok(settings)
}

#[tauri::command]
/// Persists the last successfully selected provider and model pair.
async fn remember_provider_selection(
    selection: ProviderSelection,
    state: State<'_, AppState>,
) -> Result<ProviderSettings, ProviderError> {
    let mut settings = state.providers.read().await.settings();
    settings.last_provider_id = Some(selection.provider_id.clone());
    settings.last_model_id = Some(selection.model_id);
    let settings = settings.normalized()?;
    save_provider_settings(&state.settings_path, &settings)?;
    state.providers.write().await.settings = settings.clone();
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Provider and model selection remembered",
        Some(&selection.provider_id),
        None,
    )
    .await;
    Ok(settings)
}

#[tauri::command]
/// Tests a draft provider endpoint without changing active settings.
async fn test_provider_connection(
    draft: ProviderConnectionDraft,
    state: State<'_, AppState>,
) -> Result<ProviderConnectionTest, ProviderError> {
    let started = Instant::now();
    let (provider_name, base_url, provider) = match draft.provider_id.as_str() {
        "omlx" => {
            let provider = OmlxProvider::with_base_url(&draft.base_url)?;
            let base_url = provider.base_url().to_owned();
            ("oMLX", base_url, LocalProvider::Omlx(provider))
        }
        "ollama" => {
            let provider = OllamaProvider::with_base_url(&draft.base_url)?;
            let base_url = provider.base_url().to_owned();
            ("Ollama", base_url, LocalProvider::Ollama(provider))
        }
        _ => {
            return Err(ProviderError::invalid_request(
                "Choose a supported local provider to test.",
            ));
        }
    };
    let models = match provider.discover_models().await {
        Ok(models) => models,
        Err(error) => {
            let error = sanitized(error);
            record_diagnostic(
                &state.diagnostics,
                "warn",
                "Provider connection test failed",
                Some(&draft.provider_id),
                error.diagnostic.as_deref().or(Some(&error.message)),
            )
            .await;
            return Err(error);
        }
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let message = format!(
        "Connected to {provider_name}; found {} model{}.",
        models.len(),
        if models.len() == 1 { "" } else { "s" }
    );
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Provider connection tested",
        Some(&draft.provider_id),
        Some(&format!("{} models in {elapsed_ms} ms", models.len())),
    )
    .await;
    Ok(ProviderConnectionTest {
        provider_id: draft.provider_id,
        base_url,
        model_count: models.len(),
        elapsed_ms,
        message,
    })
}

#[tauri::command]
/// Returns the bounded session diagnostic history.
async fn get_diagnostics(
    state: State<'_, AppState>,
) -> Result<Vec<DiagnosticEntry>, ProviderError> {
    Ok(state.diagnostics.lock().await.iter().cloned().collect())
}

#[tauri::command]
/// Discovers models for one provider or all configured local providers.
async fn discover_models(
    provider_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let providers = state.providers.read().await.clone();
    if let Some(provider_id) = provider_id {
        let provider = providers.provider(&provider_id)?;
        let result = provider.discover_models().await.map_err(sanitized);
        match result {
            Ok(mut models) => {
                models.sort_by(|left, right| left.display_name.cmp(&right.display_name));
                record_diagnostic(
                    &state.diagnostics,
                    "info",
                    "Provider models refreshed",
                    Some(&provider_id),
                    Some(&format!("{} streaming text models", models.len())),
                )
                .await;
                return Ok(models);
            }
            Err(error) => {
                record_diagnostic(
                    &state.diagnostics,
                    "warn",
                    "Provider model refresh failed",
                    Some(&provider_id),
                    error.diagnostic.as_deref().or(Some(&error.message)),
                )
                .await;
                return Err(error);
            }
        }
    }
    let (omlx, ollama) = futures_util::future::join(
        providers.omlx.discover_models(),
        providers.ollama.discover_models(),
    )
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
        let error = ProviderError::unavailable(
            "No local inference provider is available. Start oMLX or Ollama and try again.",
            (!errors.is_empty()).then(|| errors.join("; ")),
        );
        record_diagnostic(
            &state.diagnostics,
            "warn",
            "Provider discovery failed",
            None,
            error.diagnostic.as_deref(),
        )
        .await;
        Err(sanitized(error))
    } else {
        record_diagnostic(
            &state.diagnostics,
            "info",
            "Provider discovery completed",
            None,
            Some(&format!("{} streaming text models", models.len())),
        )
        .await;
        Ok(models)
    }
}

struct ChannelSink {
    run_id: String,
    channel: Channel<StreamEvent>,
}

impl inference::StreamSink for ChannelSink {
    /// Forwards one normalized text fragment to the WebView channel.
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

    /// Forwards one normalized reasoning fragment to the WebView channel.
    fn reasoning_delta(&self, delta: String) -> Result<(), ProviderError> {
        self.channel
            .send(StreamEvent::ReasoningDelta {
                run_id: self.run_id.clone(),
                delta,
            })
            .map_err(|error| {
                ProviderError::internal(
                    "The reasoning stream could not reach the interface.",
                    Some(error.to_string()),
                )
            })
    }

    /// Forwards normalized usage totals to the WebView channel.
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
/// Starts one cancellable provider-qualified chat generation.
async fn start_chat(
    state: State<'_, AppState>,
    request: ChatRequest,
    on_event: Channel<StreamEvent>,
) -> Result<ChatRun, ProviderError> {
    let providers = state.providers.read().await.clone();
    let provider = match request.provider_id.as_str() {
        "omlx" => LocalProvider::Omlx(providers.omlx),
        "ollama" => LocalProvider::Ollama(providers.ollama),
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
    let diagnostics = state.diagnostics.clone();
    let task_run_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        record_diagnostic(
            &diagnostics,
            "info",
            "Generation started",
            Some(provider.provider_id()),
            Some(&format!("run {task_run_id}")),
        )
        .await;
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
                record_diagnostic(
                    &diagnostics,
                    "info",
                    "Generation completed",
                    Some(provider.provider_id()),
                    Some(&format!("run {task_run_id}")),
                )
                .await;
            }
            Ok(Err(error)) => {
                let error = sanitized(error);
                let _ = on_event.send(StreamEvent::Failed {
                    run_id: task_run_id.clone(),
                    error: error.clone(),
                });
                record_diagnostic(
                    &diagnostics,
                    "error",
                    "Generation failed",
                    Some(provider.provider_id()),
                    error.diagnostic.as_deref().or(Some(&error.message)),
                )
                .await;
            }
            Err(_) => {
                let _ = on_event.send(StreamEvent::Cancelled {
                    run_id: task_run_id.clone(),
                });
                record_diagnostic(
                    &diagnostics,
                    "info",
                    "Generation cancelled",
                    Some(provider.provider_id()),
                    Some(&format!("run {task_run_id}")),
                )
                .await;
            }
        }
        runs.lock().await.remove(&task_run_id);
    });

    Ok(ChatRun { run_id })
}

#[tauri::command]
/// Cancels an active generation by its opaque run identity.
async fn cancel_chat(run_id: String, state: State<'_, AppState>) -> Result<bool, ProviderError> {
    if let Some(handle) = state.runs.lock().await.remove(&run_id) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Builds and runs the native Bottie application.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("providers.json");
            let settings = load_provider_settings(&settings_path).unwrap_or_default();
            let providers = ProviderSet::from_settings(&settings).unwrap_or_else(|_| ProviderSet {
                omlx: OmlxProvider::new().expect("the built-in oMLX configuration must be valid"),
                ollama: OllamaProvider::new()
                    .expect("the built-in Ollama configuration must be valid"),
                settings: ProviderSettings::default(),
            });
            app.manage(AppState {
                providers: tauri::async_runtime::RwLock::new(providers),
                settings_path,
                runs: Arc::new(tauri::async_runtime::Mutex::new(HashMap::new())),
                diagnostics: Diagnostics::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_provider_settings,
            update_provider_settings,
            remember_provider_selection,
            test_provider_connection,
            get_diagnostics,
            discover_models,
            start_chat,
            cancel_chat
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
