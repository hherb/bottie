//! Narrow Tauri commands for Localmail trust and authentication setup.

use std::time::Instant;

use tauri::State;

use crate::{
    AppState,
    credentials::LOCALMAIL_CREDENTIAL_ID,
    diagnostics::{record_diagnostic, sanitized},
    inference::ProviderError,
};

use super::*;

#[tauri::command]
/// Returns secret-free Localmail connection and token availability.
pub(crate) fn get_localmail_connection_status(
    state: State<'_, AppState>,
) -> Result<LocalmailConnectionStatus, ProviderError> {
    connection_status(&state.localmail_config_path, state.credentials.as_ref())
}

#[tauri::command]
/// Inspects one Localmail server identity and leaf certificate before user confirmation.
pub(crate) async fn probe_localmail_connection(
    draft: LocalmailProbeDraft,
    state: State<'_, AppState>,
) -> Result<LocalmailProbeResult, ProviderError> {
    let result = match inspect_server(&draft.origin).await {
        Ok(result) => result,
        Err(error) => {
            record_diagnostic(
                &state.diagnostics,
                "error",
                "Localmail certificate inspection failed",
                Some(LOCALMAIL_CREDENTIAL_ID),
                None,
            )
            .await;
            return Err(sanitized(error));
        }
    };
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Localmail certificate inspected",
        Some(LOCALMAIL_CREDENTIAL_ID),
        Some("Awaiting explicit certificate trust confirmation"),
    )
    .await;
    Ok(result)
}

#[tauri::command]
/// Saves a confirmed Localmail connection and optional vault-only bearer token update.
pub(crate) async fn update_localmail_connection(
    update: LocalmailConnectionUpdate,
    state: State<'_, AppState>,
) -> Result<LocalmailConnectionStatus, ProviderError> {
    let status = match update_connection(
        &state.localmail_config_path,
        state.credentials.as_ref(),
        update,
    ) {
        Ok(status) => status,
        Err(error) => {
            record_diagnostic(
                &state.diagnostics,
                "error",
                "Localmail connection save failed",
                Some(LOCALMAIL_CREDENTIAL_ID),
                None,
            )
            .await;
            return Err(sanitized(error));
        }
    };
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Localmail connection saved",
        Some(LOCALMAIL_CREDENTIAL_ID),
        Some(if status.credential_configured {
            "Certificate trust and vault credential are configured"
        } else {
            "Certificate trust is configured without a bearer credential"
        }),
    )
    .await;
    Ok(status)
}

#[tauri::command]
/// Tests the pinned Localmail identity and optional bearer authentication without reading email.
pub(crate) async fn test_localmail_connection(
    draft: LocalmailConnectionDraft,
    state: State<'_, AppState>,
) -> Result<LocalmailConnectionTest, ProviderError> {
    let started = Instant::now();
    let origin = normalize_origin(&draft.origin)?;
    let certificate_sha256 = normalize_certificate_sha256(&draft.certificate_sha256)?;
    let token = match draft
        .bearer_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        Some(value) => Some(normalize_bearer_token(value)?),
        None => state.credentials.get(LOCALMAIL_CREDENTIAL_ID)?,
    };
    let (client, _) = build_client(CertificateMode::Pinned(certificate_sha256))?;
    let result = async {
        let version = get_json::<VersionResponse>(&client, endpoint(&origin, "v1/version")?, None)
            .await
            .and_then(validate_version)?;
        let authenticated_as = if let Some(token) = token.as_deref() {
            let whoami = get_json::<WhoamiResponse>(
                &client,
                endpoint(&origin, "v1/auth/whoami")?,
                Some(token),
            )
            .await?;
            if whoami.username.trim().is_empty()
                || whoami.username.chars().count() > MAX_USERNAME_LENGTH
            {
                return Err(ProviderError::malformed(
                    "Localmail returned an invalid authenticated identity.",
                    None,
                ));
            }
            Some(whoami.username)
        } else {
            None
        };
        Ok((version, authenticated_as))
    }
    .await;
    let (version, authenticated_as) = match result {
        Ok(result) => result,
        Err(error) => {
            record_diagnostic(
                &state.diagnostics,
                "error",
                "Localmail connection test failed",
                Some(LOCALMAIL_CREDENTIAL_ID),
                None,
            )
            .await;
            return Err(sanitized(error));
        }
    };
    record_diagnostic(
        &state.diagnostics,
        "info",
        "Localmail connection test completed",
        Some(LOCALMAIL_CREDENTIAL_ID),
        Some(if authenticated_as.is_some() {
            "Server identity and bearer authentication verified"
        } else {
            "Server identity verified without bearer authentication"
        }),
    )
    .await;
    Ok(LocalmailConnectionTest {
        origin,
        server_version: version.server_version,
        authenticated_as,
        elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        message: if token.is_some() {
            "Localmail identity and bearer authentication verified.".into()
        } else {
            "Localmail identity verified; add a bearer token to verify authentication.".into()
        },
    })
}
