#![deny(missing_docs)]
//! Native application commands and lifecycle for Bottie's Tauri desktop shell.

mod attachment_garbage_collector;
mod attachment_preview_protocol;
mod attachment_processor;
mod command_types;
mod credentials;
mod diagnostics;
mod generation;
mod generation_tools;
mod inference;
mod provider_registry;
mod semantic_indexer;
mod storage;
mod storage_commands;
mod stream_channel;
mod tool_contract;
mod tool_dispatch;
mod tool_loop;

#[cfg(test)]
mod generation_tools_tests;
#[cfg(test)]
mod tool_contract_tests;
#[cfg(test)]
mod tool_dispatch_tests;
#[cfg(test)]
mod tool_loop_tests;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};

use attachment_processor::AttachmentProcessor;
use command_types::{
    AppInfo, ProviderConnectionDraft, ProviderConnectionTest, ProviderCredentialStatus,
    ProviderCredentialUpdate, ProviderSelection,
};
use credentials::{CredentialStore, REMOTE_PROVIDER_IDS, SystemCredentialStore};
use diagnostics::{DiagnosticEntry, Diagnostics, record_diagnostic, sanitized};
use futures_util::future::AbortHandle;
use generation::{cancel_chat, start_chat};
use inference::{
    AnthropicProvider, InferenceProvider, ModelInfo, OllamaProvider, OmlxProvider, OpenAiProvider,
    ProviderError, ProviderSettings, load_provider_settings, save_provider_settings,
};
use provider_registry::{ProviderSet, RoutedProvider, routed_provider};
use semantic_indexer::SemanticIndexer;
use storage::ConversationStore;
use storage_commands::{
    add_conversation_attachments, append_conversation_message, backup_conversation_store,
    branch_conversation_message, clear_last_open_conversation, create_conversation,
    delete_conversation, export_conversation_batch_json, export_conversation_json,
    export_conversation_markdown, forget_conversation, get_conversation_retention_policy,
    get_semantic_index_progress, get_storage_recovery_status, ingest_attachments,
    list_conversations, load_conversation, load_last_open_conversation, rate_conversation_response,
    reindex_semantic_memory, remove_conversation_attachment,
    remove_conversation_message_attachment, rename_conversation, restore_conversation,
    restore_conversation_store, restore_latest_automatic_backup, search_conversations,
    select_conversation_branch, set_conversation_archived, set_conversation_memory_excluded,
    set_conversation_retention_period,
};
use tauri::{Manager, State};
use tool_loop::ToolLoopCancellation;

/// Cancellation handles shared by provider I/O and native tool work for one accepted generation.
struct ActiveRun {
    abort_handle: AbortHandle,
    tool_cancellation: ToolLoopCancellation,
}

type ActiveRuns = Arc<tauri::async_runtime::Mutex<HashMap<String, ActiveRun>>>;
struct AppState {
    providers: tauri::async_runtime::RwLock<ProviderSet>,
    settings_path: PathBuf,
    runs: ActiveRuns,
    diagnostics: Diagnostics,
    credentials: Arc<dyn CredentialStore>,
    conversations: ConversationStore,
    attachment_processing: AttachmentProcessor,
    semantic_indexing: SemanticIndexer,
    storage_management: tauri::async_runtime::Mutex<()>,
}

/// Starts the non-blocking startup rotation and records only path-redacted session diagnostics.
fn schedule_automatic_backup(conversations: ConversationStore, diagnostics: Diagnostics) {
    tauri::async_runtime::spawn(async move {
        let rotation =
            tauri::async_runtime::spawn_blocking(move || conversations.rotate_automatic_backups())
                .await;
        let (level, event, detail) = match rotation {
            Ok(Ok(outcome)) if outcome.created => (
                "info",
                "Automatic backup created",
                format!(
                    "{} verified snapshot(s) retained; {} expired snapshot(s) removed",
                    outcome.retained, outcome.pruned
                ),
            ),
            Ok(Ok(outcome)) => (
                "info",
                "Automatic backups current",
                format!(
                    "{} verified snapshot(s) retained; snapshots run at most once every 24 hours",
                    outcome.retained
                ),
            ),
            Ok(Err(error)) => ("error", "Automatic backup failed", error.message),
            Err(_) => (
                "error",
                "Automatic backup failed",
                "The background backup worker stopped unexpectedly.".into(),
            ),
        };
        record_diagnostic(&diagnostics, level, event, None, Some(&detail)).await;
    });
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
/// Returns the active non-secret provider settings.
async fn get_provider_settings(
    state: State<'_, AppState>,
) -> Result<ProviderSettings, ProviderError> {
    Ok(state.providers.read().await.settings())
}

#[tauri::command]
/// Returns secret-free availability for each remote provider credential.
async fn get_provider_credential_status(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderCredentialStatus>, ProviderError> {
    REMOTE_PROVIDER_IDS
        .into_iter()
        .map(|provider_id| {
            Ok(ProviderCredentialStatus {
                provider_id: provider_id.into(),
                configured: state.credentials.configured(provider_id)?,
                unlocked: state.credentials.unlocked(provider_id)?,
                biometric_protected: state.credentials.biometric_protected(),
            })
        })
        .collect()
}

#[tauri::command]
/// Stores or removes one remote API key in the operating-system credential vault.
async fn update_provider_credential(
    update: ProviderCredentialUpdate,
    state: State<'_, AppState>,
) -> Result<ProviderCredentialStatus, ProviderError> {
    if update.remove {
        state.credentials.delete(&update.provider_id)?;
    } else if let Some(api_key) = update.api_key {
        state.credentials.set(&update.provider_id, &api_key)?;
    }
    let configured = state.credentials.configured(&update.provider_id)?;
    let unlocked = state.credentials.unlocked(&update.provider_id)?;
    record_diagnostic(
        &state.diagnostics,
        "info",
        if configured {
            "Remote credential saved"
        } else {
            "Remote credential removed"
        },
        Some(&update.provider_id),
        Some("Credential material remained in the operating-system vault"),
    )
    .await;
    Ok(ProviderCredentialStatus {
        provider_id: update.provider_id,
        configured,
        unlocked,
        biometric_protected: state.credentials.biometric_protected(),
    })
}

#[tauri::command]
/// Validates, persists, and activates provider settings.
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
            ("oMLX", base_url, RoutedProvider::Omlx(provider))
        }
        "ollama" => {
            let provider = OllamaProvider::with_base_url(&draft.base_url)?;
            let base_url = provider.base_url().to_owned();
            ("Ollama", base_url, RoutedProvider::Ollama(provider))
        }
        "openai" => {
            let key = draft
                .api_key
                .filter(|value| !value.trim().is_empty())
                .or(state.credentials.get("openai")?)
                .ok_or_else(|| {
                    ProviderError::invalid_request("Enter an OpenAI-compatible API key to test.")
                })?;
            let provider = OpenAiProvider::new(&draft.base_url, key)?;
            let base_url = provider.base_url().to_owned();
            (
                "OpenAI-compatible",
                base_url,
                RoutedProvider::OpenAi(provider),
            )
        }
        "anthropic" => {
            let key = draft
                .api_key
                .filter(|value| !value.trim().is_empty())
                .or(state.credentials.get("anthropic")?)
                .ok_or_else(|| {
                    ProviderError::invalid_request("Enter an Anthropic-compatible API key to test.")
                })?;
            let provider = AnthropicProvider::new(&draft.base_url, key)?;
            let base_url = provider.base_url().to_owned();
            (
                "Anthropic-compatible",
                base_url,
                RoutedProvider::Anthropic(provider),
            )
        }
        _ => {
            return Err(ProviderError::invalid_request(
                "Choose a supported provider to test.",
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
/// Discovers models for one provider, or both local providers when no identity is supplied.
async fn discover_models(
    provider_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let providers = state.providers.read().await.clone();
    if let Some(provider_id) = provider_id {
        let provider = routed_provider(&provider_id, &providers, state.credentials.as_ref())?;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Builds and runs the native Bottie application.
pub fn run() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("bottie-attachment", |context, request| {
            if context.webview_label() != "main" {
                return tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .expect("static preview response should build");
            }
            let state = context.app_handle().state::<AppState>();
            attachment_preview_protocol::response(&state.conversations, &request)
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("providers.json");
            let database_path = app.path().app_data_dir()?.join("bottie.sqlite3");
            let embedding_cache_path = app.path().app_data_dir()?.join("embedding-models");
            let startup = ConversationStore::initialize_for_app(database_path)
                .map_err(|error| std::io::Error::other(error.message))?;
            let diagnostics = Diagnostics::default();
            let conversations = startup.store;
            if !startup.recovery_required {
                attachment_garbage_collector::collect_at_startup(
                    &conversations,
                    diagnostics.clone(),
                );
            }
            let semantic_indexing = SemanticIndexer::start(
                embedding_cache_path,
                conversations.clone(),
                diagnostics.clone(),
            );
            let attachment_processing = AttachmentProcessor::start(
                app.handle().clone(),
                conversations.clone(),
                diagnostics.clone(),
                semantic_indexing.clone(),
            );
            if startup.recovery_required {
                let recovery_diagnostics = diagnostics.clone();
                tauri::async_runtime::spawn(async move {
                    record_diagnostic(
                        &recovery_diagnostics,
                        "error",
                        "Local data recovery required",
                        None,
                        Some("SQLite integrity failed; conversation access is paused"),
                    )
                    .await;
                });
            } else {
                schedule_automatic_backup(conversations.clone(), diagnostics.clone());
                attachment_processing.wake();
                semantic_indexing.wake();
            }
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
                diagnostics,
                credentials: Arc::new(SystemCredentialStore::default()),
                conversations,
                attachment_processing,
                semantic_indexing,
                storage_management: tauri::async_runtime::Mutex::new(()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_provider_settings,
            get_provider_credential_status,
            update_provider_credential,
            update_provider_settings,
            remember_provider_selection,
            test_provider_connection,
            get_diagnostics,
            get_storage_recovery_status,
            get_conversation_retention_policy,
            get_semantic_index_progress,
            reindex_semantic_memory,
            ingest_attachments,
            list_conversations,
            search_conversations,
            create_conversation,
            load_conversation,
            export_conversation_markdown,
            export_conversation_json,
            export_conversation_batch_json,
            backup_conversation_store,
            restore_conversation_store,
            restore_latest_automatic_backup,
            load_last_open_conversation,
            clear_last_open_conversation,
            append_conversation_message,
            add_conversation_attachments,
            remove_conversation_attachment,
            remove_conversation_message_attachment,
            branch_conversation_message,
            select_conversation_branch,
            rate_conversation_response,
            rename_conversation,
            set_conversation_archived,
            set_conversation_memory_excluded,
            set_conversation_retention_period,
            delete_conversation,
            restore_conversation,
            forget_conversation,
            discover_models,
            start_chat,
            cancel_chat
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
