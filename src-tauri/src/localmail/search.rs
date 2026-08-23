//! Closed native request and inert response mapping for Localmail email search.

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use dom_query::Document;
use futures_util::StreamExt;
use reqwest::{
    Client, Method, Request, Response, StatusCode,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;

use crate::{
    credentials::{CredentialStore, LOCALMAIL_CREDENTIAL_ID},
    inference::ProviderError,
};

use super::{CertificateMode, build_client, endpoint, load_config, normalize_bearer_token};

/// Maximum Unicode scalar count accepted for one email search query.
pub(super) const MAX_EMAIL_QUERY_CHARS: usize = 500;
/// Maximum Unicode scalar count accepted for one textual search filter.
pub(super) const MAX_EMAIL_FILTER_CHARS: usize = 320;
/// Maximum number of summaries returned by one native email search.
pub(super) const MAX_EMAIL_RESULTS: u8 = 20;
/// Maximum Unicode scalar count retained from one Localmail message identity.
pub(super) const MAX_EMAIL_MESSAGE_ID_CHARS: usize = 128;
/// Maximum Unicode scalar count retained from one email subject.
pub(super) const MAX_EMAIL_SUBJECT_CHARS: usize = 500;
/// Maximum Unicode scalar count retained from one sender display name.
pub(super) const MAX_EMAIL_SENDER_NAME_CHARS: usize = 200;
/// Maximum Unicode scalar count retained from one sender address.
pub(super) const MAX_EMAIL_ADDRESS_CHARS: usize = 320;
/// Maximum Unicode scalar count retained from one inert result snippet.
pub(super) const MAX_EMAIL_SNIPPET_CHARS: usize = 1_200;
/// Maximum UTF-8 bytes accepted from Localmail's fixed search route.
pub(super) const MAX_EMAIL_SEARCH_RESPONSE_BYTES: usize = 128 * 1_024;

const NON_CONTENT_SELECTOR: &str =
    "script, style, template, noscript, svg, math, canvas, iframe, object, embed";

/// Closed filter set accepted by Bottie's first Localmail search boundary.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SearchEmailFilters {
    /// Optional sender-address or sender-name filter.
    pub(super) from: Option<String>,
    /// Optional recipient-address or recipient-name filter.
    pub(super) to: Option<String>,
    /// Optional subject-text filter.
    pub(super) subject: Option<String>,
    /// Optional strict lower date bound in `YYYY-MM-DD` form.
    pub(super) after: Option<String>,
    /// Optional strict upper date bound in `YYYY-MM-DD` form.
    pub(super) before: Option<String>,
    /// Optional attachment-presence filter.
    pub(super) has_attachments: Option<bool>,
}

/// One closed, bounded email-search request accepted from the WebView.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SearchEmailRequest {
    /// Required nonblank Localmail search query.
    pub(super) query: String,
    /// Optional fixed email metadata filters.
    #[serde(default)]
    pub(super) filters: SearchEmailFilters,
    /// Maximum number of inert summaries to return.
    pub(super) result_limit: u8,
}

/// Path-free email address metadata returned in one inert summary.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SearchEmailAddress {
    /// Bounded sender address when Localmail has one.
    pub(super) address: Option<String>,
    /// Bounded sender display name when Localmail has one.
    pub(super) name: Option<String>,
}

/// One path-free inert email summary returned by the native connector.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SearchEmailSummary {
    /// Opaque Localmail message identity reserved for a future explicit open contract.
    pub(super) message_id: String,
    /// Bounded plain subject text.
    pub(super) subject: Option<String>,
    /// Bounded sender metadata without account or folder internals.
    pub(super) sender: SearchEmailAddress,
    /// UTC RFC 3339 message date when Localmail returned one.
    pub(super) sent_at: Option<String>,
    /// Bounded plain-text snippet with all server markup removed.
    pub(super) snippet: Option<String>,
    /// Whether Localmail reports at least one attachment without exposing attachment metadata.
    pub(super) has_attachments: bool,
}

/// Bounded email-search response marked as untrusted external content.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchEmailResponse {
    /// Inert path-free message summaries in Localmail result order.
    pub(super) results: Vec<SearchEmailSummary>,
    /// Fixed marker requiring downstream callers to treat email content as untrusted.
    pub(super) untrusted: bool,
}

/// Exact fixed-route JSON request sent to Localmail after Bottie validation.
#[derive(Debug, Serialize)]
pub(super) struct LocalmailSearchRequest {
    /// Normalized search query.
    pub(super) query: String,
    /// Normalized fixed metadata filters.
    pub(super) filters: LocalmailSearchFilters,
    /// Bounded first-page result count.
    pub(super) limit: u8,
}

/// Exact Localmail wire filters retained by Bottie's smaller connector contract.
#[derive(Debug, Default, Serialize)]
pub(super) struct LocalmailSearchFilters {
    /// Optional sender filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) from: Option<String>,
    /// Optional recipient filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) to: Option<String>,
    /// Optional subject filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) subject: Option<String>,
    /// Optional strict lower date bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) after: Option<String>,
    /// Optional strict upper date bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) before: Option<String>,
    /// Optional attachment-presence filter under Localmail's wire name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) has_attachment: Option<bool>,
}

#[derive(Deserialize)]
struct RawSearchResponse {
    results: Vec<RawSearchResult>,
}

#[derive(Deserialize)]
struct RawSearchResult {
    message_id: String,
    subject: Option<String>,
    from: RawSearchAddress,
    date: Option<String>,
    snippet_html: Option<String>,
    has_attachments: bool,
}

#[derive(Deserialize)]
struct RawSearchAddress {
    address: Option<String>,
    name: Option<String>,
}

/// Executes one bounded search using only the saved pinned connection and native vault credential.
pub(super) async fn search_email_native(
    config_path: &Path,
    credentials: &dyn CredentialStore,
    request: SearchEmailRequest,
) -> Result<SearchEmailResponse, ProviderError> {
    let request = validate_search_email_request(request)?;
    let config = load_config(config_path)?.ok_or_else(missing_connection_error)?;
    let token = credentials
        .get(LOCALMAIL_CREDENTIAL_ID)?
        .ok_or_else(missing_credential_error)
        .and_then(|value| normalize_bearer_token(&value))?;
    let (client, _) = build_client(CertificateMode::Pinned(config.certificate_sha256))?;
    let endpoint = endpoint(&config.origin, "v1/search")?;
    execute_search_request(&client, endpoint, &token, &request).await
}

/// Executes the production request/response path against an isolated HTTP loopback fixture.
#[cfg(test)]
pub(super) async fn search_email_fixture(
    origin: &str,
    bearer_token: &str,
    request: SearchEmailRequest,
) -> Result<SearchEmailResponse, ProviderError> {
    let request = validate_search_email_request(request)?;
    let endpoint = Url::parse(origin)
        .and_then(|origin| origin.join("v1/search"))
        .map_err(|_| internal_search_error())?;
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| internal_search_error())?;
    execute_search_request(&client, endpoint, bearer_token, &request).await
}

/// Builds and sends one redirect-free fixed-route request before bounded response decoding.
async fn execute_search_request(
    client: &Client,
    endpoint: Url,
    bearer_token: &str,
    request: &LocalmailSearchRequest,
) -> Result<SearchEmailResponse, ProviderError> {
    let result_limit = request.limit;
    let request = build_search_http_request(client, endpoint, bearer_token, request)?;
    let response = client
        .execute(request)
        .await
        .map_err(|_| unavailable_search_error())?;
    let bytes = read_bounded_search_body(response).await?;
    decode_search_response(&bytes, result_limit)
}

/// Builds the sole Localmail search request with a sensitive bearer header and bounded JSON body.
pub(super) fn build_search_http_request(
    client: &Client,
    endpoint: Url,
    bearer_token: &str,
    request: &LocalmailSearchRequest,
) -> Result<Request, ProviderError> {
    let body = serde_json::to_vec(request).map_err(|_| internal_search_error())?;
    let mut authorization = HeaderValue::from_str(&format!("Bearer {bearer_token}"))
        .map_err(|_| internal_search_error())?;
    authorization.set_sensitive(true);
    client
        .request(Method::POST, endpoint)
        .header(AUTHORIZATION, authorization)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .build()
        .map_err(|_| internal_search_error())
}

/// Reads one successful search response under the connector-specific byte ceiling.
async fn read_bounded_search_body(response: Response) -> Result<Vec<u8>, ProviderError> {
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            return Err(ProviderError::invalid_request(
                "Localmail rejected the configured bearer token.",
            ));
        }
        status if !status.is_success() => {
            return Err(ProviderError::server(
                "Localmail returned an unsuccessful email search response.",
                None,
            ));
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| unavailable_search_error())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_EMAIL_SEARCH_RESPONSE_BYTES {
            return Err(malformed_search_response());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Validates and normalizes the complete request before any config, credential, or network work.
pub(super) fn validate_search_email_request(
    request: SearchEmailRequest,
) -> Result<LocalmailSearchRequest, ProviderError> {
    let query = required_inline(&request.query, MAX_EMAIL_QUERY_CHARS)?;
    if !(1..=MAX_EMAIL_RESULTS).contains(&request.result_limit) {
        return Err(invalid_search_request());
    }
    let filters = LocalmailSearchFilters {
        from: optional_filter(request.filters.from)?,
        to: optional_filter(request.filters.to)?,
        subject: optional_filter(request.filters.subject)?,
        after: optional_date(request.filters.after)?,
        before: optional_date(request.filters.before)?,
        has_attachment: request.filters.has_attachments,
    };
    if let (Some(after), Some(before)) = (&filters.after, &filters.before)
        && after > before
    {
        return Err(invalid_search_request());
    }
    Ok(LocalmailSearchRequest {
        query,
        filters,
        limit: request.result_limit,
    })
}

/// Decodes one bounded Localmail response into inert, path-free summary metadata.
pub(super) fn decode_search_response(
    bytes: &[u8],
    result_limit: u8,
) -> Result<SearchEmailResponse, ProviderError> {
    if bytes.len() > MAX_EMAIL_SEARCH_RESPONSE_BYTES {
        return Err(malformed_search_response());
    }
    let raw: RawSearchResponse =
        serde_json::from_slice(bytes).map_err(|_| malformed_search_response())?;
    let results = raw
        .results
        .into_iter()
        .take(usize::from(result_limit.min(MAX_EMAIL_RESULTS)))
        .map(map_search_result)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SearchEmailResponse {
        results,
        untrusted: true,
    })
}

/// Maps one server result while discarding scores, folders, accounts, recipients, and cursors.
fn map_search_result(raw: RawSearchResult) -> Result<SearchEmailSummary, ProviderError> {
    let message_id = normalize_message_id(&raw.message_id)?;
    let sent_at = raw
        .date
        .as_deref()
        .map(normalize_response_date)
        .transpose()?;
    Ok(SearchEmailSummary {
        message_id,
        subject: bounded_optional(raw.subject.as_deref(), MAX_EMAIL_SUBJECT_CHARS),
        sender: SearchEmailAddress {
            address: bounded_optional(raw.from.address.as_deref(), MAX_EMAIL_ADDRESS_CHARS),
            name: bounded_optional(raw.from.name.as_deref(), MAX_EMAIL_SENDER_NAME_CHARS),
        },
        sent_at,
        snippet: raw.snippet_html.as_deref().and_then(sanitize_snippet),
        has_attachments: raw.has_attachments,
    })
}

/// Requires the strict decimal string identity emitted by Localmail's bigint API boundary.
fn normalize_message_id(value: &str) -> Result<String, ProviderError> {
    if !is_valid_message_id(value) {
        return Err(malformed_search_response());
    }
    Ok(value.to_owned())
}

/// Recognizes the strict decimal string identity emitted and accepted by Localmail.
pub(super) fn is_valid_message_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_EMAIL_MESSAGE_ID_CHARS
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<i64>().is_ok()
}

/// Converts one server timestamp to a stable UTC RFC 3339 representation.
fn normalize_response_date(value: &str) -> Result<String, ProviderError> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|date| {
            date.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
        .map_err(|_| malformed_search_response())
}

/// Parses server snippet markup and retains bounded visible text only.
fn sanitize_snippet(value: &str) -> Option<String> {
    let source = format!("<body>{value}</body>");
    let document = Document::from(source.as_str());
    document.select(NON_CONTENT_SELECTOR).remove();
    let text = document
        .body()
        .map_or_else(String::new, |body| body.formatted_text().to_string());
    bounded_optional(Some(&text), MAX_EMAIL_SNIPPET_CHARS)
}

/// Normalizes one required inline field under a Unicode scalar ceiling.
fn required_inline(value: &str, limit: usize) -> Result<String, ProviderError> {
    let value = normalize_inline(value);
    if value.is_empty() || value.chars().count() > limit {
        return Err(invalid_search_request());
    }
    Ok(value)
}

/// Normalizes one optional nonblank textual filter.
fn optional_filter(value: Option<String>) -> Result<Option<String>, ProviderError> {
    value
        .map(|value| required_inline(&value, MAX_EMAIL_FILTER_CHARS))
        .transpose()
}

/// Validates one optional Localmail date filter.
fn optional_date(value: Option<String>) -> Result<Option<String>, ProviderError> {
    value
        .map(|value| {
            let value = value.trim();
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map(|_| value.to_owned())
                .map_err(|_| invalid_search_request())
        })
        .transpose()
}

/// Collapses whitespace and removes control-shaped formatting from one inline value.
fn normalize_inline(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() || character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns one optional normalized value truncated without splitting Unicode scalars.
fn bounded_optional(value: Option<&str>, limit: usize) -> Option<String> {
    let normalized = normalize_inline(value?);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.chars().take(limit).collect())
}

/// Returns the fixed path-free request-policy failure.
fn invalid_search_request() -> ProviderError {
    ProviderError::invalid_request(
        "Use a bounded Localmail email query, supported filters, and a result limit from 1 to 20.",
    )
}

/// Returns the fixed path-free response-contract failure.
pub(super) fn malformed_search_response() -> ProviderError {
    ProviderError::malformed("Localmail returned malformed email search metadata.", None)
}

/// Returns the fixed failure for a missing saved Localmail trust configuration.
fn missing_connection_error() -> ProviderError {
    ProviderError::invalid_request(
        "Configure and confirm a pinned Localmail HTTPS connection before searching email.",
    )
}

/// Returns the fixed failure for a missing or unavailable Localmail bearer token.
fn missing_credential_error() -> ProviderError {
    ProviderError::invalid_request(
        "Add and unlock a Localmail bearer token before searching email.",
    )
}

/// Returns the fixed path-free request-layer failure.
fn unavailable_search_error() -> ProviderError {
    ProviderError::unavailable(
        "Bottie could not complete the pinned Localmail email search.",
        None,
    )
}

/// Returns the fixed path-free request-construction failure.
fn internal_search_error() -> ProviderError {
    ProviderError::internal("Bottie could not prepare the Localmail email search.", None)
}
