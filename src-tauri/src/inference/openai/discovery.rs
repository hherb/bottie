//! Pure OpenAI-compatible model-list normalization.

use serde::Deserialize;

use super::{PROVIDER_ID, PROVIDER_NAME};
use crate::inference::types::{ModelInfo, ModelLoadState, ProviderCapabilities, ProviderError};

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelRecord>,
}

#[derive(Deserialize)]
struct ModelRecord {
    id: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

/// Decodes non-empty models and only the capabilities an endpoint explicitly advertises.
pub(super) fn decode_model_list(bytes: &[u8]) -> Result<Vec<ModelInfo>, ProviderError> {
    let response: ModelList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "The OpenAI-compatible provider returned an invalid model list.",
            Some(error.to_string()),
        )
    })?;
    let models = response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| {
            let has = |name: &str| {
                model
                    .capabilities
                    .iter()
                    .any(|capability| capability == name)
            };
            ModelInfo {
                provider_id: PROVIDER_ID.into(),
                provider_name: PROVIDER_NAME.into(),
                display_name: model.id.clone(),
                model_id: model.id,
                max_context_tokens: None,
                load_state: ModelLoadState::Unknown,
                capabilities: ProviderCapabilities {
                    text: true,
                    streaming: true,
                    tools: has("tools"),
                    vision: has("vision"),
                    audio: has("audio"),
                    ..Default::default()
                },
            }
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        Err(ProviderError::unavailable(
            "The OpenAI-compatible provider reported no models.",
            None,
        ))
    } else {
        Ok(models)
    }
}
