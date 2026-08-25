//! First-party Localmail trust, bearer authentication, and bounded inert email reading.

mod commands;
mod config;
mod open;
mod search;
mod search_order;
mod tls;

use std::{path::Path, time::Duration};

use futures_util::StreamExt;
use reqwest::{Client, Response, StatusCode, header::HeaderValue, redirect::Policy};
use rustls::ClientConfig;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use crate::{
    credentials::{CredentialStore, LOCALMAIL_CREDENTIAL_ID},
    inference::ProviderError,
};
pub(crate) use commands::{
    get_localmail_connection_status, open_email, probe_localmail_connection, search_email,
    test_localmail_connection, update_localmail_connection,
};
use config::{LocalmailConfig, load_config, save_config};
pub(crate) use open::validate_open_email_request;
pub(crate) use open::{OpenEmailRequest, open_email_native};
pub(crate) use search::{
    MAX_EMAIL_FILTER_CHARS, MAX_EMAIL_MESSAGE_ID_CHARS, MAX_EMAIL_QUERY_CHARS, MAX_EMAIL_RESULTS,
    SearchEmailRequest, search_email_native, validate_search_email_request,
};
use tls::{CertificateMode, CertificateVerifier};

const LOCALMAIL_API_MAJOR: u32 = 1;
const MAX_ORIGIN_LENGTH: usize = 2_048;
const CERTIFICATE_SHA256_HEX_LENGTH: usize = 64;
const MAX_BEARER_TOKEN_LENGTH: usize = 4_096;
const MAX_RESPONSE_BYTES: usize = 32 * 1_024;
const MAX_SERVER_VERSION_LENGTH: usize = 128;
const MAX_USERNAME_LENGTH: usize = 200;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Candidate Localmail HTTPS origin submitted for certificate inspection.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalmailProbeDraft {
    origin: String,
}

/// Path-free server identity and certificate metadata returned for explicit confirmation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalmailProbeResult {
    origin: String,
    api_major: u32,
    api_minor: u32,
    server_version: String,
    certificate_sha256: String,
}

/// Draft connection and optional replacement token used by a bounded native test.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalmailConnectionDraft {
    origin: String,
    certificate_sha256: String,
    bearer_token: Option<String>,
}

/// Confirmed non-secret connection plus the requested vault-token mutation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalmailConnectionUpdate {
    origin: String,
    certificate_sha256: String,
    bearer_token: Option<String>,
    remove_token: bool,
}

/// Secret-free Localmail configuration and vault availability returned to the WebView.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalmailConnectionStatus {
    pub(crate) origin: Option<String>,
    pub(crate) certificate_sha256: Option<String>,
    pub(crate) credential_configured: bool,
    credential_unlocked: bool,
    biometric_protected: bool,
}

/// Bounded result of testing Localmail identity and optional bearer authentication.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalmailConnectionTest {
    origin: String,
    server_version: String,
    authenticated_as: Option<String>,
    elapsed_ms: u64,
    message: String,
}

#[derive(Deserialize)]
struct VersionResponse {
    api_major: u32,
    api_minor: u32,
    server_version: String,
}

#[derive(Deserialize)]
struct WhoamiResponse {
    username: String,
}

/// Normalizes an HTTPS origin while rejecting credentials and request-specific URL parts.
pub(super) fn normalize_origin(value: &str) -> Result<String, ProviderError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_ORIGIN_LENGTH {
        return Err(ProviderError::invalid_request(
            "Enter a bounded Localmail HTTPS origin.",
        ));
    }
    let mut origin = Url::parse(value)
        .map_err(|_| ProviderError::invalid_request("Enter a valid Localmail HTTPS origin."))?;
    if origin.scheme() != "https"
        || origin.host().is_none()
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.path() != "/"
    {
        return Err(ProviderError::invalid_request(
            "Localmail requires an HTTPS origin without credentials, a path, query, or fragment.",
        ));
    }
    origin.set_path("/");
    Ok(origin.to_string())
}

/// Validates and normalizes one lowercase SHA-256 certificate fingerprint.
pub(super) fn normalize_certificate_sha256(value: &str) -> Result<String, ProviderError> {
    let value = value.trim();
    if value.len() != CERTIFICATE_SHA256_HEX_LENGTH
        || !value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err(ProviderError::invalid_request(
            "Inspect the Localmail certificate before confirming trust.",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

/// Validates a bounded bearer token before it can enter an HTTP header or native vault.
fn normalize_bearer_token(value: &str) -> Result<String, ProviderError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_BEARER_TOKEN_LENGTH {
        return Err(ProviderError::invalid_request(
            "Enter a non-empty bounded Localmail bearer token.",
        ));
    }
    HeaderValue::from_str(value).map_err(|_| {
        ProviderError::invalid_request(
            "The Localmail bearer token contains unsupported characters.",
        )
    })?;
    Ok(value.into())
}

/// Builds a proxy-free, redirect-free client with the explicit certificate policy.
fn build_client(
    mode: CertificateMode,
) -> Result<(Client, std::sync::Arc<CertificateVerifier>), ProviderError> {
    let verifier = CertificateVerifier::new(mode);
    let crypto_provider = rustls::crypto::ring::default_provider();
    let tls_config = ClientConfig::builder_with_provider(crypto_provider.into())
        .with_safe_default_protocol_versions()
        .map_err(|_| localmail_internal_error())?
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    let client = Client::builder()
        .use_preconfigured_tls(tls_config)
        .no_proxy()
        .redirect(Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| localmail_internal_error())?;
    Ok((client, verifier))
}

/// Reads one successful JSON response without retaining an unbounded provider body.
async fn read_bounded_json<T: DeserializeOwned>(
    response: Response,
    bearer_present: bool,
) -> Result<T, ProviderError> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(ProviderError::invalid_request(if bearer_present {
            "Localmail rejected the bearer token."
        } else {
            "Localmail did not expose the required public server-identity route."
        }));
    }
    if !status.is_success() {
        return Err(ProviderError::server(
            "Localmail returned an unsuccessful response.",
            None,
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| localmail_unavailable_error())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ProviderError::malformed(
                "Localmail returned more connection metadata than Bottie accepts.",
                None,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        ProviderError::malformed("Localmail returned malformed connection metadata.", None)
    })
}

/// Sends one bounded GET with an optional bearer credential confined to a sensitive header.
async fn get_json<T: DeserializeOwned>(
    client: &Client,
    endpoint: Url,
    bearer_token: Option<&str>,
) -> Result<T, ProviderError> {
    let mut request = client.get(endpoint);
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| localmail_unavailable_error())?;
    read_bounded_json(response, bearer_token.is_some()).await
}

/// Appends one fixed Localmail API path to a previously validated origin.
fn endpoint(origin: &str, path: &str) -> Result<Url, ProviderError> {
    Url::parse(origin)
        .and_then(|origin| origin.join(path))
        .map_err(|_| localmail_internal_error())
}

/// Validates the fixed version contract and returns its bounded server version.
fn validate_version(version: VersionResponse) -> Result<VersionResponse, ProviderError> {
    if version.api_major != LOCALMAIL_API_MAJOR {
        return Err(ProviderError::invalid_request(
            "This Localmail server uses an incompatible API version.",
        ));
    }
    if version.server_version.trim().is_empty()
        || version.server_version.chars().count() > MAX_SERVER_VERSION_LENGTH
    {
        return Err(ProviderError::malformed(
            "Localmail returned an invalid server identity.",
            None,
        ));
    }
    Ok(version)
}

/// Produces secret-free status from native configuration and credential-vault metadata.
pub(crate) fn connection_status(
    path: &Path,
    credentials: &dyn CredentialStore,
) -> Result<LocalmailConnectionStatus, ProviderError> {
    let config = load_config(path)?;
    Ok(LocalmailConnectionStatus {
        origin: config.as_ref().map(|value| value.origin.clone()),
        certificate_sha256: config.map(|value| value.certificate_sha256),
        credential_configured: credentials.configured(LOCALMAIL_CREDENTIAL_ID)?,
        credential_unlocked: credentials.unlocked(LOCALMAIL_CREDENTIAL_ID)?,
        biometric_protected: credentials.biometric_protected(),
    })
}

/// Persists a confirmed connection and applies one explicit token mutation.
fn update_connection(
    path: &Path,
    credentials: &dyn CredentialStore,
    update: LocalmailConnectionUpdate,
) -> Result<LocalmailConnectionStatus, ProviderError> {
    if update.remove_token
        && update
            .bearer_token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(ProviderError::invalid_request(
            "Choose either a replacement Localmail token or token removal.",
        ));
    }
    let config = LocalmailConfig {
        origin: normalize_origin(&update.origin)?,
        certificate_sha256: normalize_certificate_sha256(&update.certificate_sha256)?,
    };
    let token = update
        .bearer_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(normalize_bearer_token)
        .transpose()?;
    save_config(path, &config)?;
    if update.remove_token {
        credentials.delete(LOCALMAIL_CREDENTIAL_ID)?;
    } else if let Some(token) = token {
        credentials.set(LOCALMAIL_CREDENTIAL_ID, &token)?;
    }
    connection_status(path, credentials)
}

/// Inspects the fixed Localmail version route and captures its presented leaf certificate.
async fn inspect_server(origin: &str) -> Result<LocalmailProbeResult, ProviderError> {
    let origin = normalize_origin(origin)?;
    let (client, verifier) = build_client(CertificateMode::Inspect)?;
    let version = get_json::<VersionResponse>(&client, endpoint(&origin, "v1/version")?, None)
        .await
        .and_then(validate_version)?;
    let certificate_sha256 = verifier
        .captured_fingerprint()
        .ok_or_else(localmail_unavailable_error)?;
    Ok(LocalmailProbeResult {
        origin,
        api_major: version.api_major,
        api_minor: version.api_minor,
        server_version: version.server_version,
        certificate_sha256,
    })
}

/// Returns a fixed path-free native failure for request-layer problems.
fn localmail_unavailable_error() -> ProviderError {
    ProviderError::unavailable(
        "Bottie could not reach Localmail with the selected HTTPS origin and certificate.",
        None,
    )
}

/// Returns a fixed path-free native failure for client construction problems.
fn localmail_internal_error() -> ProviderError {
    ProviderError::internal("Bottie could not prepare the Localmail connection.", None)
}

#[cfg(test)]
mod open_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod tests;
