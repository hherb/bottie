//! Native-only Localmail attachment identity resolution and extracted-text retrieval.

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
    open::{
        MAX_EMAIL_ATTACHMENT_CONTENT_TYPE_CHARS, MAX_EMAIL_ATTACHMENT_FILENAME_CHARS,
        MAX_EMAIL_ATTACHMENTS, bounded_body, bounded_inline, build_open_http_request,
        read_bounded_open_body,
    },
    search::is_valid_message_id,
};

/// Maximum Unicode scalar count returned from extracted attachment text.
pub(super) const MAX_EMAIL_ATTACHMENT_TEXT_CHARS: usize = 12 * 1_024;
/// Maximum UTF-8 bytes accepted from Localmail's extracted-text route before truncation.
const MAX_EMAIL_ATTACHMENT_TEXT_RESPONSE_BYTES: usize = 2 * 1_024 * 1_024;
const SHA256_HEX_CHARS: usize = 64;

/// Closed provider request for extracted text from one numbered message attachment.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ReadEmailAttachmentRequest {
    /// Exact opaque decimal identity previously returned by `search_email`.
    pub(super) message_id: String,
    /// Bounded 1-based position returned by `open_email`.
    pub(super) attachment_number: usize,
}

/// Validated path-safe request used by the native Localmail connector.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LocalmailAttachmentTextRequest {
    /// Exact Localmail decimal message identity.
    pub(super) message_id: String,
    /// Bounded 1-based attachment position.
    pub(super) attachment_number: usize,
}

/// Bounded inert extracted attachment text without its content-addressed identity.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadEmailAttachmentResponse {
    /// Exact message identity correlated with the requested attachment.
    pub(super) message_id: String,
    /// Exact 1-based attachment position correlated with `open_email`.
    pub(super) attachment_number: usize,
    /// Bounded safe filename when Localmail supplied one.
    pub(super) filename: Option<String>,
    /// Bounded declared media type when Localmail supplied one.
    pub(super) content_type: Option<String>,
    /// Original decoded byte size when Localmail supplied one.
    pub(super) byte_size: Option<u64>,
    /// Bounded normalized extracted plain text.
    pub(super) text: String,
    /// Whether Bottie truncated longer extracted text to its provider-tool ceiling.
    pub(super) truncated: bool,
    /// Fixed marker requiring downstream callers to treat attachment content as untrusted.
    pub(super) untrusted: bool,
}

#[derive(Deserialize)]
struct RawAttachmentMessage {
    id: String,
    attachments: Vec<RawAttachmentIdentity>,
}

#[derive(Deserialize)]
struct RawAttachmentIdentity {
    filename: Option<String>,
    sha256: Option<String>,
    content_type: Option<String>,
    size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttachmentTextResponse {
    text: String,
}

/// Native attachment identity retained only long enough to call the fixed text route.
pub(super) struct ResolvedAttachment {
    pub(super) sha256: String,
    pub(super) filename: Option<String>,
    pub(super) content_type: Option<String>,
    pub(super) byte_size: Option<u64>,
}

/// Executes one message-correlated extracted-text read through saved pinned Localmail trust.
pub(crate) async fn read_email_attachment_native(
    config_path: &Path,
    credentials: &dyn CredentialStore,
    request: ReadEmailAttachmentRequest,
) -> Result<ReadEmailAttachmentResponse, ProviderError> {
    let request = validate_read_email_attachment_request(request)?;
    let config = load_config(config_path)?.ok_or_else(missing_connection_error)?;
    let token = credentials
        .get(LOCALMAIL_CREDENTIAL_ID)?
        .ok_or_else(missing_credential_error)
        .and_then(|value| normalize_bearer_token(&value))?;
    let (client, _) = build_client(CertificateMode::Pinned(config.certificate_sha256))?;
    let detail_endpoint = endpoint(
        &config.origin,
        &format!("v1/messages/{}", request.message_id),
    )?;
    let detail_request = build_open_http_request(&client, detail_endpoint, &token)?;
    let detail_response = client
        .execute(detail_request)
        .await
        .map_err(|_| unavailable_attachment_error())?;
    let detail_bytes = read_bounded_open_body(detail_response).await?;
    let resolved = resolve_attachment(
        &detail_bytes,
        &request.message_id,
        request.attachment_number,
    )?;
    let text_endpoint = endpoint(
        &config.origin,
        &format!("v1/attachments/{}/text", resolved.sha256),
    )?;
    let text_request = build_attachment_text_http_request(&client, text_endpoint, &token)?;
    let text_response = client
        .execute(text_request)
        .await
        .map_err(|_| unavailable_attachment_error())?;
    let text_bytes = read_bounded_attachment_text_body(text_response).await?;
    decode_attachment_text_response(
        &text_bytes,
        &request.message_id,
        request.attachment_number,
        resolved,
    )
}

/// Validates the complete request before configuration, credential, or network work.
pub(crate) fn validate_read_email_attachment_request(
    request: ReadEmailAttachmentRequest,
) -> Result<LocalmailAttachmentTextRequest, ProviderError> {
    if !is_valid_message_id(&request.message_id)
        || !(1..=MAX_EMAIL_ATTACHMENTS).contains(&request.attachment_number)
    {
        return Err(invalid_attachment_request());
    }
    Ok(LocalmailAttachmentTextRequest {
        message_id: request.message_id,
        attachment_number: request.attachment_number,
    })
}

/// Resolves one bounded attachment number to its native-only content hash and safe metadata.
pub(super) fn resolve_attachment(
    bytes: &[u8],
    expected_message_id: &str,
    attachment_number: usize,
) -> Result<ResolvedAttachment, ProviderError> {
    let raw: RawAttachmentMessage =
        serde_json::from_slice(bytes).map_err(|_| malformed_attachment_response())?;
    if raw.id != expected_message_id || !is_valid_message_id(&raw.id) {
        return Err(malformed_attachment_response());
    }
    let attachment = raw
        .attachments
        .into_iter()
        .take(MAX_EMAIL_ATTACHMENTS)
        .nth(attachment_number - 1)
        .ok_or_else(unavailable_attachment_selection)?;
    let sha256 = attachment
        .sha256
        .filter(|value| is_lowercase_sha256(value))
        .ok_or_else(malformed_attachment_response)?;
    Ok(ResolvedAttachment {
        sha256,
        filename: bounded_inline(
            attachment.filename.as_deref(),
            MAX_EMAIL_ATTACHMENT_FILENAME_CHARS,
        ),
        content_type: bounded_inline(
            attachment.content_type.as_deref(),
            MAX_EMAIL_ATTACHMENT_CONTENT_TYPE_CHARS,
        ),
        byte_size: attachment.size,
    })
}

/// Builds the sole extracted-text request with a sensitive bearer and JSON-only response.
pub(super) fn build_attachment_text_http_request(
    client: &Client,
    endpoint: Url,
    bearer_token: &str,
) -> Result<Request, ProviderError> {
    let mut authorization = HeaderValue::from_str(&format!("Bearer {bearer_token}"))
        .map_err(|_| internal_attachment_error())?;
    authorization.set_sensitive(true);
    client
        .request(Method::GET, endpoint)
        .header(AUTHORIZATION, authorization)
        .header(ACCEPT, "application/json")
        .build()
        .map_err(|_| internal_attachment_error())
}

/// Reads one successful extracted-text response under a strict native byte ceiling.
async fn read_bounded_attachment_text_body(response: Response) -> Result<Vec<u8>, ProviderError> {
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            return Err(ProviderError::invalid_request(
                "Localmail rejected the configured bearer token.",
            ));
        }
        StatusCode::NOT_FOUND => return Err(unavailable_attachment_selection()),
        status if !status.is_success() => {
            return Err(ProviderError::server(
                "Localmail returned an unsuccessful attachment-text response.",
                None,
            ));
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| unavailable_attachment_error())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_EMAIL_ATTACHMENT_TEXT_RESPONSE_BYTES {
            return Err(malformed_attachment_response());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Converts Localmail JSON into normalized bounded text and omits its native content hash.
pub(super) fn decode_attachment_text_response(
    bytes: &[u8],
    message_id: &str,
    attachment_number: usize,
    resolved: ResolvedAttachment,
) -> Result<ReadEmailAttachmentResponse, ProviderError> {
    if bytes.len() > MAX_EMAIL_ATTACHMENT_TEXT_RESPONSE_BYTES {
        return Err(malformed_attachment_response());
    }
    let raw: RawAttachmentTextResponse =
        serde_json::from_slice(bytes).map_err(|_| malformed_attachment_response())?;
    let normalized = bounded_body(&raw.text).ok_or_else(unavailable_attachment_selection)?;
    let original_chars = normalized.chars().count();
    let text = normalized
        .chars()
        .take(MAX_EMAIL_ATTACHMENT_TEXT_CHARS)
        .collect();
    Ok(ReadEmailAttachmentResponse {
        message_id: message_id.into(),
        attachment_number,
        filename: resolved.filename,
        content_type: resolved.content_type,
        byte_size: resolved.byte_size,
        text,
        truncated: original_chars > MAX_EMAIL_ATTACHMENT_TEXT_CHARS,
        untrusted: true,
    })
}

/// Accepts only Localmail's lowercase content-addressed SHA-256 representation.
fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == SHA256_HEX_CHARS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_attachment_request() -> ProviderError {
    ProviderError::invalid_request(
        "Read attachment text using one exact message identity and attachment number from open_email.",
    )
}

fn malformed_attachment_response() -> ProviderError {
    ProviderError::malformed("Localmail returned malformed attachment content.", None)
}

fn unavailable_attachment_selection() -> ProviderError {
    ProviderError::invalid_request(
        "That Localmail attachment or its extracted text is not available.",
    )
}

fn missing_connection_error() -> ProviderError {
    ProviderError::invalid_request(
        "Configure and confirm a pinned Localmail HTTPS connection before reading attachments.",
    )
}

fn missing_credential_error() -> ProviderError {
    ProviderError::invalid_request(
        "Add and unlock a Localmail bearer token before reading attachments.",
    )
}

fn unavailable_attachment_error() -> ProviderError {
    ProviderError::unavailable(
        "Bottie could not complete the pinned Localmail attachment-text request.",
        None,
    )
}

fn internal_attachment_error() -> ProviderError {
    ProviderError::internal(
        "Bottie could not prepare the Localmail attachment-text request.",
        None,
    )
}
