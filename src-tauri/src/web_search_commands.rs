//! Narrow Tauri commands for native web-search credential verification.

use std::time::Instant;

use tauri::State;

use crate::{
    AppState,
    command_types::{WebSearchConnectionDraft, WebSearchConnectionTest},
    diagnostics::record_diagnostic,
    inference::ProviderError,
    web_search::{
        BRAVE_SEARCH_PROVIDER_ID, BraveSearchProvider, EXA_SEARCH_PROVIDER_ID, ExaSearchProvider,
        WebSearchError, WebSearchErrorCode, WebSearchProvider, connection_test_request,
    },
};

#[tauri::command]
/// Tests one supported fixed search route with a draft or saved native-vault credential.
pub(crate) async fn test_web_search_connection(
    draft: WebSearchConnectionDraft,
    state: State<'_, AppState>,
) -> Result<WebSearchConnectionTest, ProviderError> {
    if !matches!(
        draft.provider_id.as_str(),
        BRAVE_SEARCH_PROVIDER_ID | EXA_SEARCH_PROVIDER_ID
    ) {
        return Err(ProviderError::invalid_request(
            "Choose a supported web-search provider to test.",
        ));
    }
    let started = Instant::now();
    let provider_id = draft.provider_id;
    let api_key = draft
        .api_key
        .filter(|value| !value.trim().is_empty())
        .or(state.credentials.get(&provider_id)?)
        .ok_or_else(|| ProviderError::invalid_request(missing_test_credential(&provider_id)))?;
    let search_result = match provider_id.as_str() {
        BRAVE_SEARCH_PROVIDER_ID => {
            BraveSearchProvider::new(api_key)
                .map_err(map_web_search_error)?
                .search(connection_test_request())
                .await
        }
        EXA_SEARCH_PROVIDER_ID => {
            ExaSearchProvider::new(api_key)
                .map_err(map_web_search_error)?
                .search(connection_test_request())
                .await
        }
        _ => unreachable!("provider identity was validated above"),
    };
    if let Err(error) = search_result {
        let error = map_web_search_error(error);
        record_diagnostic(
            &state.diagnostics,
            "warn",
            "Web-search connection test failed",
            Some(&provider_id),
            Some(&error.message),
        )
        .await;
        return Err(error);
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let provider_name = search_provider_name(&provider_id);
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Web-search connection tested",
        Some(&provider_id),
        Some(&format!(
            "Fixed {provider_name} route responded in {elapsed_ms} ms"
        )),
    )
    .await;
    Ok(WebSearchConnectionTest {
        provider_id,
        elapsed_ms,
        message: format!("Connected to {provider_name}."),
    })
}

/// Returns the fixed route name for one validated search provider identity.
fn search_provider_name(provider_id: &str) -> &'static str {
    match provider_id {
        EXA_SEARCH_PROVIDER_ID => "Exa Search",
        _ => "Brave Search",
    }
}

/// Returns provider-specific guidance without including credential material.
fn missing_test_credential(provider_id: &str) -> &'static str {
    match provider_id {
        EXA_SEARCH_PROVIDER_ID => "Enter an Exa Search API key to test.",
        _ => "Enter a Brave Search API key to test.",
    }
}

/// Maps the web-search adapter's redacted categories into the existing command error envelope.
fn map_web_search_error(error: WebSearchError) -> ProviderError {
    match error.code {
        WebSearchErrorCode::InvalidRequest
        | WebSearchErrorCode::CredentialRequired
        | WebSearchErrorCode::CredentialRejected => ProviderError::invalid_request(error.message),
        WebSearchErrorCode::RateLimited => ProviderError::server(error.message, None),
        WebSearchErrorCode::Timeout | WebSearchErrorCode::Unavailable => {
            ProviderError::unavailable(error.message, None)
        }
        WebSearchErrorCode::MalformedResponse => ProviderError::malformed(error.message, None),
        WebSearchErrorCode::Internal => ProviderError::internal(error.message, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_errors_map_without_diagnostics_or_secret_material() {
        let mapped = map_web_search_error(WebSearchError {
            code: WebSearchErrorCode::CredentialRejected,
            message: "The web-search provider rejected the configured credential.".into(),
            retryable: false,
        });

        assert_eq!(
            mapped.message,
            "The web-search provider rejected the configured credential."
        );
        assert!(!mapped.retryable);
        assert!(mapped.diagnostic.is_none());
    }
}
