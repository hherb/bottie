//! Bounded oMLX model and fixed-endpoint capability discovery.

use std::collections::HashMap;

use serde::Deserialize;

use super::{OmlxProvider, PROVIDER_ID, PROVIDER_NAME};
use crate::inference::types::{ModelLoadState, ProviderCapabilities};
use crate::inference::{ModelInfo, ProviderError};

/// Maximum fixed OpenAPI document accepted for fail-closed endpoint capability discovery.
const MAX_OPENAPI_DOCUMENT_BYTES: usize = 1_048_576;

#[derive(Deserialize)]
struct OmlxModelList {
    data: Vec<OmlxModel>,
}

#[derive(Deserialize)]
struct OmlxModel {
    id: String,
    max_model_len: Option<u64>,
    #[serde(default)]
    capabilities: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OmlxModelStatusList {
    models: Vec<OmlxModelStatus>,
}

#[derive(Deserialize)]
struct OmlxModelStatus {
    id: String,
    loaded: Option<bool>,
    engine_type: Option<String>,
    model_type: Option<String>,
}

#[derive(Clone, Copy)]
/// Explicit per-model VLM and residency metadata normalized from the status route.
pub(super) struct OmlxModelStatusMetadata {
    vision: bool,
    text: bool,
    embeddings: bool,
    load_state: ModelLoadState,
}

/// Performs catalogue, optional status, and fail-closed fixed OpenAPI discovery.
pub(super) async fn discover_models(
    provider: &OmlxProvider,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let mut models = decode_model_list(&provider.get("v1/models").await?)?;
    if let Ok(status_bytes) = provider.get("v1/models/status").await
        && let Ok(statuses) = decode_model_status(&status_bytes)
    {
        enrich_models(&mut models, &statuses);
    }
    let supports_tools = provider
        .get_bounded("openapi.json", MAX_OPENAPI_DOCUMENT_BYTES)
        .await
        .is_ok_and(|bytes| decode_openapi_tool_support(&bytes));
    enrich_tool_capabilities(&mut models, supports_tools);
    Ok(models)
}

/// Decodes and normalizes the oMLX model-list response.
pub(super) fn decode_model_list(bytes: &[u8]) -> Result<Vec<ModelInfo>, ProviderError> {
    let response: OmlxModelList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "oMLX returned an invalid model list.",
            Some(error.to_string()),
        )
    })?;
    let models = response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| {
            let capabilities = model.capabilities.unwrap_or_default();
            let vision = capabilities.iter().any(|capability| capability == "vision");
            let embeddings = capabilities
                .iter()
                .any(|capability| capability == "embeddings");
            ModelInfo {
                provider_id: PROVIDER_ID.into(),
                provider_name: PROVIDER_NAME.into(),
                display_name: model.id.replace("--", "/"),
                model_id: model.id,
                max_context_tokens: model.max_model_len,
                load_state: ModelLoadState::Unknown,
                capabilities: ProviderCapabilities {
                    text: !embeddings,
                    streaming: true,
                    vision,
                    embeddings,
                    ..ProviderCapabilities::default()
                },
            }
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        return Err(ProviderError::unavailable(
            "oMLX is running but has no models available.",
            None,
        ));
    }
    Ok(models)
}

/// Enables client-executed tools only for text models on an explicitly capable endpoint.
pub(super) fn enrich_tool_capabilities(models: &mut [ModelInfo], endpoint_supports_tools: bool) {
    for model in models {
        model.capabilities.tools = endpoint_supports_tools && model.capabilities.text;
    }
}

/// Decodes the fixed chat-completions OpenAPI request schema without external references.
pub(super) fn decode_openapi_tool_support(bytes: &[u8]) -> bool {
    if bytes.len() > MAX_OPENAPI_DOCUMENT_BYTES {
        return false;
    }
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    let Some(mut schema) = document.pointer(
        "/paths/~1v1~1chat~1completions/post/requestBody/content/application~1json/schema",
    ) else {
        return false;
    };
    if let Some(reference) = schema.get("$ref").and_then(serde_json::Value::as_str) {
        let Some(pointer) = reference.strip_prefix('#') else {
            return false;
        };
        let Some(resolved) = document.pointer(pointer) else {
            return false;
        };
        schema = resolved;
    }
    schema.get("type").and_then(serde_json::Value::as_str) == Some("object")
        && schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|properties| {
                properties.contains_key("tools") && properties.contains_key("tool_choice")
            })
}

/// Decodes explicit oMLX model type and residency metadata by model identity.
pub(super) fn decode_model_status(
    bytes: &[u8],
) -> Result<HashMap<String, OmlxModelStatusMetadata>, ProviderError> {
    let response: OmlxModelStatusList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "oMLX returned an invalid model status list.",
            Some(error.to_string()),
        )
    })?;
    Ok(response
        .models
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| {
            let vision = model.model_type.as_deref() == Some("vlm")
                || model.engine_type.as_deref() == Some("vlm");
            let embeddings = model.model_type.as_deref() == Some("embedding")
                || model.engine_type.as_deref() == Some("embedding");
            let text = !embeddings
                && matches!(
                    model.model_type.as_deref().or(model.engine_type.as_deref()),
                    Some("llm" | "vlm")
                );
            let load_state = match model.loaded {
                Some(true) => ModelLoadState::Loaded,
                Some(false) => ModelLoadState::Unloaded,
                None => ModelLoadState::Unknown,
            };
            (
                model.id,
                OmlxModelStatusMetadata {
                    vision,
                    text,
                    embeddings,
                    load_state,
                },
            )
        })
        .collect())
}

/// Applies status metadata without weakening catalogue capabilities.
pub(super) fn enrich_models(
    models: &mut [ModelInfo],
    statuses: &HashMap<String, OmlxModelStatusMetadata>,
) {
    for model in models {
        let Some(status) = statuses.get(&model.model_id) else {
            continue;
        };
        model.capabilities.vision |= status.vision;
        model.capabilities.text = status.text;
        model.capabilities.embeddings = status.embeddings;
        model.load_state = status.load_state;
    }
}
