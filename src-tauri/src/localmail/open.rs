//! Closed native request and inert response mapping for opening one Localmail email.

use chrono::{DateTime, SecondsFormat, Utc};
use dom_query::Document;
use futures_util::StreamExt;
use reqwest::{
    Client, Method, Request, Response, StatusCode,
    header::{ACCEPT, AUTHORIZATION, HeaderValue},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;

use crate::{
    credentials::{CredentialStore, LOCALMAIL_CREDENTIAL_ID},
    inference::ProviderError,
};

use super::{
    CertificateMode, build_client, endpoint, load_config, normalize_bearer_token,
    search::{
        MAX_EMAIL_ADDRESS_CHARS, MAX_EMAIL_SENDER_NAME_CHARS, MAX_EMAIL_SUBJECT_CHARS,
        SearchEmailAddress, is_valid_message_id,
    },
};

/// Maximum number of To or Cc entries retained from one opened email.
pub(super) const MAX_EMAIL_HEADER_ADDRESSES: usize = 50;
/// Maximum Unicode scalar count retained from one opened email body.
pub(super) const MAX_EMAIL_BODY_CHARS: usize = 32 * 1_024;
/// Maximum number of attachment entries exposed from one opened email.
pub(crate) const MAX_EMAIL_ATTACHMENTS: usize = 50;
/// Maximum Unicode scalar count retained from one attachment filename.
pub(super) const MAX_EMAIL_ATTACHMENT_FILENAME_CHARS: usize = 255;
/// Maximum Unicode scalar count retained from one attachment media type.
pub(super) const MAX_EMAIL_ATTACHMENT_CONTENT_TYPE_CHARS: usize = 127;
/// Maximum UTF-8 bytes accepted from Localmail's fixed message-detail route.
pub(super) const MAX_EMAIL_OPEN_RESPONSE_BYTES: usize = 512 * 1_024;

const NON_CONTENT_SELECTOR: &str =
    "script, style, template, noscript, svg, math, canvas, iframe, object, embed";

/// One closed email-open request accepting only a Localmail search-result identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OpenEmailRequest {
    /// Exact opaque decimal message identity returned by `search_email`.
    pub(super) message_id: String,
}

/// Validated path-safe identity used to construct the fixed Localmail detail route.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LocalmailOpenRequest {
    /// Exact Localmail decimal message identity.
    pub(super) message_id: String,
}

/// Bounded inert content for one exact Localmail search result.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenEmailResponse {
    /// Exact response identity correlated with the requested search result.
    pub(super) message_id: String,
    /// Bounded plain subject text.
    pub(super) subject: Option<String>,
    /// Bounded sender metadata without account internals.
    pub(super) sender: SearchEmailAddress,
    /// Bounded To recipients in server order.
    pub(super) to: Vec<SearchEmailAddress>,
    /// Bounded Cc recipients in server order.
    pub(super) cc: Vec<SearchEmailAddress>,
    /// UTC RFC 3339 message date when Localmail returned one.
    pub(super) sent_at: Option<String>,
    /// Bounded inert plain body text, optionally derived from sanitized HTML.
    pub(super) body: Option<String>,
    /// Whether Localmail reports attachments without exposing their metadata or bytes.
    pub(super) has_attachments: bool,
    /// Bounded safe metadata used to select extracted text without exposing content hashes.
    pub(super) attachments: Vec<OpenEmailAttachment>,
    /// Fixed marker requiring downstream callers to treat email content as untrusted.
    pub(super) untrusted: bool,
}

/// Safe attachment metadata correlated by a bounded 1-based message-local number.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenEmailAttachment {
    /// Stable position within the opened message's attachment list.
    pub(super) attachment_number: usize,
    /// Bounded display filename when Localmail supplied one.
    pub(super) filename: Option<String>,
    /// Bounded declared media type when Localmail supplied one.
    pub(super) content_type: Option<String>,
    /// Original decoded byte size when Localmail supplied one.
    pub(super) byte_size: Option<u64>,
}

#[derive(Deserialize)]
struct RawOpenResponse {
    id: String,
    subject: Option<String>,
    from: RawOpenAddress,
    to: Vec<RawOpenAddress>,
    cc: Vec<RawOpenAddress>,
    date: Option<String>,
    body_text: Option<String>,
    body_html: Option<String>,
    attachments: Vec<RawOpenAttachment>,
}

#[derive(Deserialize)]
struct RawOpenAttachment {
    filename: Option<String>,
    content_type: Option<String>,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct RawOpenAddress {
    address: Option<String>,
    name: Option<String>,
}

/// Executes one bounded open using only the saved pinned connection and native vault credential.
pub(crate) async fn open_email_native(
    config_path: &Path,
    credentials: &dyn CredentialStore,
    request: OpenEmailRequest,
) -> Result<OpenEmailResponse, ProviderError> {
    let request = validate_open_email_request(request)?;
    let config = load_config(config_path)?.ok_or_else(missing_connection_error)?;
    let token = credentials
        .get(LOCALMAIL_CREDENTIAL_ID)?
        .ok_or_else(missing_credential_error)
        .and_then(|value| normalize_bearer_token(&value))?;
    let (client, _) = build_client(CertificateMode::Pinned(config.certificate_sha256))?;
    let endpoint = endpoint(
        &config.origin,
        &format!("v1/messages/{}", request.message_id),
    )?;
    execute_open_request(&client, endpoint, &token, &request.message_id).await
}

/// Executes the production request/response path against an isolated HTTP loopback fixture.
#[cfg(test)]
pub(super) async fn open_email_fixture(
    origin: &str,
    bearer_token: &str,
    request: OpenEmailRequest,
) -> Result<OpenEmailResponse, ProviderError> {
    let request = validate_open_email_request(request)?;
    let endpoint = Url::parse(origin)
        .and_then(|origin| origin.join(&format!("v1/messages/{}", request.message_id)))
        .map_err(|_| internal_open_error())?;
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| internal_open_error())?;
    execute_open_request(&client, endpoint, bearer_token, &request.message_id).await
}

/// Builds and sends one redirect-free fixed-route request before bounded response decoding.
async fn execute_open_request(
    client: &Client,
    endpoint: Url,
    bearer_token: &str,
    message_id: &str,
) -> Result<OpenEmailResponse, ProviderError> {
    let request = build_open_http_request(client, endpoint, bearer_token)?;
    let response = client
        .execute(request)
        .await
        .map_err(|_| unavailable_open_error())?;
    let bytes = read_bounded_open_body(response).await?;
    decode_open_response(&bytes, message_id)
}

/// Builds the sole Localmail detail request with compact headers and external images disabled.
pub(super) fn build_open_http_request(
    client: &Client,
    mut endpoint: Url,
    bearer_token: &str,
) -> Result<Request, ProviderError> {
    endpoint
        .query_pairs_mut()
        .append_pair("headers", "compact")
        .append_pair("external_images", "false");
    let mut authorization = HeaderValue::from_str(&format!("Bearer {bearer_token}"))
        .map_err(|_| internal_open_error())?;
    authorization.set_sensitive(true);
    client
        .request(Method::GET, endpoint)
        .header(AUTHORIZATION, authorization)
        .header(ACCEPT, "application/json")
        .build()
        .map_err(|_| internal_open_error())
}

/// Reads one successful detail response under the connector-specific byte ceiling.
pub(super) async fn read_bounded_open_body(response: Response) -> Result<Vec<u8>, ProviderError> {
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            return Err(ProviderError::invalid_request(
                "Localmail rejected the configured bearer token.",
            ));
        }
        StatusCode::NOT_FOUND => {
            return Err(ProviderError::invalid_request(
                "That Localmail email is no longer available.",
            ));
        }
        status if !status.is_success() => {
            return Err(ProviderError::server(
                "Localmail returned an unsuccessful email detail response.",
                None,
            ));
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| unavailable_open_error())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_EMAIL_OPEN_RESPONSE_BYTES {
            return Err(malformed_open_response());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Validates the complete request before any config, credential, or network work.
pub(crate) fn validate_open_email_request(
    request: OpenEmailRequest,
) -> Result<LocalmailOpenRequest, ProviderError> {
    if !is_valid_message_id(&request.message_id) {
        return Err(invalid_open_request());
    }
    Ok(LocalmailOpenRequest {
        message_id: request.message_id,
    })
}

/// Decodes one bounded Localmail detail response into inert, path-free content.
pub(super) fn decode_open_response(
    bytes: &[u8],
    expected_message_id: &str,
) -> Result<OpenEmailResponse, ProviderError> {
    if bytes.len() > MAX_EMAIL_OPEN_RESPONSE_BYTES {
        return Err(malformed_open_response());
    }
    let raw: RawOpenResponse =
        serde_json::from_slice(bytes).map_err(|_| malformed_open_response())?;
    if raw.id != expected_message_id || !is_valid_message_id(&raw.id) {
        return Err(malformed_open_response());
    }
    let sent_at = raw
        .date
        .as_deref()
        .map(normalize_response_date)
        .transpose()?;
    let body = raw
        .body_text
        .as_deref()
        .and_then(bounded_body)
        .or_else(|| raw.body_html.as_deref().and_then(html_fallback));
    let has_attachments = !raw.attachments.is_empty();
    Ok(OpenEmailResponse {
        message_id: raw.id,
        subject: bounded_inline(raw.subject.as_deref(), MAX_EMAIL_SUBJECT_CHARS),
        sender: map_address(raw.from),
        to: map_addresses(raw.to),
        cc: map_addresses(raw.cc),
        sent_at,
        body,
        has_attachments,
        attachments: map_attachments(raw.attachments),
        untrusted: true,
    })
}

/// Maps a bounded prefix of Localmail attachment metadata while excluding content hashes.
fn map_attachments(values: Vec<RawOpenAttachment>) -> Vec<OpenEmailAttachment> {
    values
        .into_iter()
        .take(MAX_EMAIL_ATTACHMENTS)
        .enumerate()
        .map(|(index, value)| OpenEmailAttachment {
            attachment_number: index + 1,
            filename: bounded_inline(
                value.filename.as_deref(),
                MAX_EMAIL_ATTACHMENT_FILENAME_CHARS,
            ),
            content_type: bounded_inline(
                value.content_type.as_deref(),
                MAX_EMAIL_ATTACHMENT_CONTENT_TYPE_CHARS,
            ),
            byte_size: value.size,
        })
        .collect()
}

/// Maps one bounded number of address entries while discarding extra server fields.
fn map_addresses(values: Vec<RawOpenAddress>) -> Vec<SearchEmailAddress> {
    values
        .into_iter()
        .take(MAX_EMAIL_HEADER_ADDRESSES)
        .map(map_address)
        .collect()
}

/// Maps one path-free address entry under the shared search-result text limits.
fn map_address(value: RawOpenAddress) -> SearchEmailAddress {
    SearchEmailAddress {
        address: bounded_inline(value.address.as_deref(), MAX_EMAIL_ADDRESS_CHARS),
        name: bounded_inline(value.name.as_deref(), MAX_EMAIL_SENDER_NAME_CHARS),
    }
}

/// Converts one server timestamp to a stable UTC RFC 3339 representation.
fn normalize_response_date(value: &str) -> Result<String, ProviderError> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|date| {
            date.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
        .map_err(|_| malformed_open_response())
}

/// Converts sanitized server HTML into bounded inert visible text when plain text is absent.
fn html_fallback(value: &str) -> Option<String> {
    let source = format!("<body>{value}</body>");
    let document = Document::from(source.as_str());
    document.select(NON_CONTENT_SELECTOR).remove();
    document
        .body()
        .and_then(|body| bounded_body(body.formatted_text().as_ref()))
}

/// Normalizes multiline body content while retaining at most one blank separator line.
pub(super) fn bounded_body(value: &str) -> Option<String> {
    let value = value.replace("\r\n", "\n").replace('\r', "\n");
    let mut normalized = String::new();
    let mut blank_lines = 0_u8;
    for line in value.lines() {
        let line = normalize_inline(line);
        if line.is_empty() {
            if !normalized.is_empty() {
                blank_lines = 1;
            }
            continue;
        }
        if !normalized.is_empty() {
            normalized.push('\n');
            if blank_lines > 0 {
                normalized.push('\n');
            }
        }
        normalized.push_str(&line);
        blank_lines = 0;
    }
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.chars().take(MAX_EMAIL_BODY_CHARS).collect())
}

/// Returns one optional normalized inline value truncated without splitting Unicode scalars.
pub(super) fn bounded_inline(value: Option<&str>, limit: usize) -> Option<String> {
    let normalized = normalize_inline(value?);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.chars().take(limit).collect())
}

/// Collapses whitespace and control-shaped formatting from one inline header value.
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

/// Returns the fixed path-free request-policy failure.
fn invalid_open_request() -> ProviderError {
    ProviderError::invalid_request(
        "Open email using one exact message identity returned by Localmail search.",
    )
}

/// Returns the fixed path-free response-contract failure.
fn malformed_open_response() -> ProviderError {
    ProviderError::malformed("Localmail returned malformed email detail content.", None)
}

/// Returns the fixed failure for a missing saved Localmail trust configuration.
fn missing_connection_error() -> ProviderError {
    ProviderError::invalid_request(
        "Configure and confirm a pinned Localmail HTTPS connection before opening email.",
    )
}

/// Returns the fixed failure for a missing or unavailable Localmail bearer token.
fn missing_credential_error() -> ProviderError {
    ProviderError::invalid_request("Add and unlock a Localmail bearer token before opening email.")
}

/// Returns the fixed path-free request-layer failure.
fn unavailable_open_error() -> ProviderError {
    ProviderError::unavailable(
        "Bottie could not complete the pinned Localmail email detail request.",
        None,
    )
}

/// Returns the fixed path-free request-construction failure.
fn internal_open_error() -> ProviderError {
    ProviderError::internal(
        "Bottie could not prepare the Localmail email detail request.",
        None,
    )
}
