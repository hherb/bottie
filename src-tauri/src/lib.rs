#![deny(missing_docs)]
//! Native application commands and lifecycle for Bottie's Tauri desktop shell.

mod attachment_garbage_collector;
mod attachment_preview_protocol;
mod attachment_processor;
mod command_types;
mod credential_session;
mod credentials;
mod diagnostics;
mod generation;
mod generation_context;
mod generation_localmail_tools;
mod generation_tool_audit;
mod generation_tools;
mod generation_usage;
mod generation_web_tools;
mod inference;
mod localmail;
mod microphone;
mod provider_registry;
mod run_cancellation;
mod semantic_indexer;
mod speech;
mod storage;
mod storage_commands;
mod stream_channel;
mod tool_contract;
mod tool_dispatch;
mod tool_loop;
mod tool_policy;
#[cfg(desktop)]
mod updater;
mod web_fetch;
mod web_policy;
pub mod web_search;
mod web_search_commands;

#[cfg(test)]
mod generation_tools_tests;
#[cfg(test)]
mod localmail_tool_tests;
#[cfg(test)]
mod security_policy_tests;
#[cfg(test)]
mod tool_contract_tests;
#[cfg(test)]
mod tool_dispatch_tests;
#[cfg(test)]
mod tool_loop_tests;
#[cfg(test)]
mod tool_policy_tests;
#[cfg(test)]
mod updater_tests;
#[cfg(test)]
mod web_policy_tests;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};

use attachment_processor::AttachmentProcessor;
use command_types::{
    AppInfo, ProviderConnectionDraft, ProviderConnectionTest, ProviderCredentialStatus,
    ProviderCredentialUpdate, ProviderSelection,
};
use credential_session::schedule_session_unlock;
use credentials::{
    CredentialStore, SystemCredentialStore, provider_credential_status,
    provider_credential_statuses,
};
use diagnostics::{DiagnosticEntry, Diagnostics, export_diagnostics, record_diagnostic, sanitized};
use generation::start_chat;
use inference::{
    AnthropicProvider, InferenceProvider, ModelInfo, OllamaProvider, OmlxProvider, OpenAiProvider,
    ProviderError, ProviderSettings, load_provider_settings, persist_completed_first_run_setup,
    save_provider_settings,
};
use localmail::{
    get_localmail_connection_status, open_email, probe_localmail_connection, search_email,
    test_localmail_connection, update_localmail_connection,
};
use microphone::{MicrophoneController, MicrophoneStatus, TranscriptCorrectionError};
use provider_registry::{ProviderSet, RoutedProvider, routed_provider};
use run_cancellation::{ActiveRuns, cancel_all_chats, cancel_chat};
use semantic_indexer::SemanticIndexer;
use speech::{SpeechCommandError, SpeechController, SpeechStatus, SpeechVoice};
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
use web_search_commands::test_web_search_connection;

struct AppState {
    providers: tauri::async_runtime::RwLock<ProviderSet>,
    settings_path: PathBuf,
    localmail_config_path: PathBuf,
    microphone: MicrophoneController,
    speech: SpeechController,
    runs: ActiveRuns,
    voice_interaction: tauri::async_runtime::Mutex<()>,
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
/// Returns secret-free availability for each native provider credential.
async fn get_provider_credential_status(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderCredentialStatus>, ProviderError> {
    provider_credential_statuses(state.credentials.as_ref())
}

#[tauri::command]
/// Stores or removes one native provider API key in the operating-system credential vault.
async fn update_provider_credential(
    update: ProviderCredentialUpdate,
    state: State<'_, AppState>,
) -> Result<ProviderCredentialStatus, ProviderError> {
    if update.remove {
        state.credentials.delete(&update.provider_id)?;
    } else if let Some(api_key) = update.api_key {
        state.credentials.set(&update.provider_id, &api_key)?;
    }
    let status = provider_credential_status(state.credentials.as_ref(), &update.provider_id)?;
    record_diagnostic(
        &state.diagnostics,
        "info",
        if status.configured {
            "Remote credential saved"
        } else {
            "Remote credential removed"
        },
        Some(&update.provider_id),
        Some("Credential material remained in the operating-system vault"),
    )
    .await;
    Ok(status)
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
        "Provider settings saved",
        None,
        Some(concat!(
            "Non-secret provider routes, Web policy, and remembered tool preferences were updated; ",
            "no credentials stored",
        )),
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
/// Persists completion of the first-run provider and privacy disclosure.
async fn complete_first_run_setup(
    state: State<'_, AppState>,
) -> Result<ProviderSettings, ProviderError> {
    let settings = state.providers.read().await.settings();
    let settings = persist_completed_first_run_setup(&state.settings_path, settings)?;
    state.providers.write().await.settings = settings.clone();
    record_diagnostic(
        &state.diagnostics,
        "info",
        "First-run setup completed",
        settings.last_provider_id.as_deref(),
        Some("Provider route and privacy boundaries acknowledged; no credentials stored"),
    )
    .await;
    Ok(settings)
}

#[tauri::command]
/// Returns bounded native microphone state without samples or device identity.
fn get_microphone_status(state: State<'_, AppState>) -> MicrophoneStatus {
    state.microphone.status()
}

#[tauri::command]
/// Interrupts Bottie's active output, then starts session-only capture after explicit user action.
async fn start_microphone_capture(
    state: State<'_, AppState>,
) -> Result<MicrophoneStatus, SpeechCommandError> {
    let _voice_guard = state.voice_interaction.lock().await;
    cancel_all_chats(&state.runs).await;
    state.speech.stop_before_microphone_capture()?;
    Ok(state.microphone.start())
}

#[tauri::command]
/// Stops native capture while retaining only its bounded in-memory PCM buffer.
fn stop_microphone_capture(state: State<'_, AppState>) -> MicrophoneStatus {
    state.microphone.stop()
}

#[tauri::command]
/// Discards the current native PCM buffer without persisting or forwarding it.
fn discard_microphone_capture(state: State<'_, AppState>) -> MicrophoneStatus {
    state.microphone.discard()
}

#[tauri::command]
/// Replaces one final local transcript turn in bounded session-only native memory.
fn correct_microphone_transcript(
    turn_index: usize,
    text: String,
    state: State<'_, AppState>,
) -> Result<MicrophoneStatus, TranscriptCorrectionError> {
    state.microphone.correct_transcript(turn_index, &text)
}

#[tauri::command]
/// Returns current local speech state without utterance text or backend detail.
fn get_speech_status(state: State<'_, AppState>) -> SpeechStatus {
    state.speech.status()
}

#[tauri::command]
/// Lazily lists bounded local voices without device or filesystem identity.
fn list_speech_voices(state: State<'_, AppState>) -> Result<Vec<SpeechVoice>, SpeechCommandError> {
    state.speech.list_voices()
}

#[tauri::command]
/// Selects one exact engine-provided voice for this process lifetime.
fn select_speech_voice(
    voice_id: String,
    state: State<'_, AppState>,
) -> Result<SpeechStatus, SpeechCommandError> {
    state.speech.select_voice(&voice_id)
}

#[tauri::command]
/// Plays one bounded text payload through the selected local voice after explicit user action.
fn speak_text(
    text: String,
    state: State<'_, AppState>,
) -> Result<SpeechStatus, SpeechCommandError> {
    if state.microphone.is_capturing() {
        return Err(SpeechCommandError::MicrophoneActive);
    }
    state.speech.speak(&text)
}

#[tauri::command]
/// Stops only Bottie's current local speech playback.
fn stop_speech(state: State<'_, AppState>) -> SpeechStatus {
    state.speech.stop()
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
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.manage(updater::UpdaterState::default());
            }
            let settings_path = app.path().app_config_dir()?.join("providers.json");
            let localmail_config_path = app.path().app_config_dir()?.join("localmail.json");
            let database_path = app.path().app_data_dir()?.join("bottie.sqlite3");
            let embedding_cache_path = app.path().app_data_dir()?.join("embedding-models");
            let speech_model_cache_path = app.path().app_data_dir()?.join("speech-models");
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
            let credentials = Arc::new(SystemCredentialStore::default());
            schedule_session_unlock(credentials.clone(), diagnostics.clone());
            app.manage(AppState {
                providers: tauri::async_runtime::RwLock::new(providers),
                settings_path,
                localmail_config_path,
                microphone: MicrophoneController::new(speech_model_cache_path),
                speech: SpeechController::default(),
                runs: Arc::new(tauri::async_runtime::Mutex::new(HashMap::new())),
                voice_interaction: tauri::async_runtime::Mutex::new(()),
                diagnostics,
                credentials,
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
            complete_first_run_setup,
            get_microphone_status,
            start_microphone_capture,
            stop_microphone_capture,
            discard_microphone_capture,
            correct_microphone_transcript,
            get_speech_status,
            list_speech_voices,
            select_speech_voice,
            speak_text,
            stop_speech,
            test_provider_connection,
            test_web_search_connection,
            get_localmail_connection_status,
            probe_localmail_connection,
            test_localmail_connection,
            update_localmail_connection,
            search_email,
            open_email,
            get_diagnostics,
            export_diagnostics,
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
            cancel_chat,
            #[cfg(desktop)]
            updater::check_for_update,
            #[cfg(desktop)]
            updater::install_update,
            #[cfg(desktop)]
            updater::cancel_update_operation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
