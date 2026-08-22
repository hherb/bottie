//! Exa Search API adapter behind the provider-neutral native search contract.

use std::time::Duration;

use chrono::{SecondsFormat, TimeDelta, Utc};
use futures_util::StreamExt;
use reqwest::{
    Client, Response, StatusCode,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{
    MAX_WEB_SEARCH_RESULTS, WebSearchError, WebSearchFreshness, WebSearchProvider,
    WebSearchRequest, WebSearchResponse, WebSearchResult,
};

const PROVIDER_ID: &str = "exa";
const EXA_SEARCH_ENDPOINT: &str = "https://api.exa.ai/search";
const API_KEY_HEADER: &str = "x-api-key";
const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_RESPONSE_BYTES: usize = 2 * 1_024 * 1_024;
const MAX_RESULT_TITLE_CHARS: usize = 512;
const MAX_RESULT_URL_CHARS: usize = 4_096;
const MAX_RESULT_SNIPPET_CHARS: usize = 4_096;
const MAX_PUBLICATION_METADATA_CHARS: usize = 128;

/// Authenticated Rust-owned Exa Search adapter.
#[derive(Clone)]
pub struct ExaSearchProvider {
    client: Client,
    endpoint: Url,
    api_key: HeaderValue,
}

impl ExaSearchProvider {
    /// Builds the fixed-endpoint Exa adapter without exposing the supplied credential.
    pub fn new(api_key: impl Into<String>) -> Result<Self, WebSearchError> {
        let endpoint = Url::parse(EXA_SEARCH_ENDPOINT).map_err(|_| WebSearchError::internal())?;
        Self::build(endpoint, api_key.into())
    }

    #[cfg(test)]
    /// Builds a test-only adapter for an isolated loopback HTTP fixture.
    pub(super) fn for_loopback_fixture(
        base_url: &str,
        api_key: &str,
    ) -> Result<Self, WebSearchError> {
        let endpoint = Url::parse(base_url)
            .and_then(|base| base.join("search"))
            .map_err(|_| WebSearchError::internal())?;
        Self::build(endpoint, api_key.into())
    }

    /// Builds an adapter from an already-policy-validated endpoint.
    fn build(endpoint: Url, api_key: String) -> Result<Self, WebSearchError> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(WebSearchError::credential_required());
        }
        let mut api_key =
            HeaderValue::from_str(api_key).map_err(|_| WebSearchError::credential_rejected())?;
        api_key.set_sensitive(true);
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(SEARCH_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| WebSearchError::internal())?;
        Ok(Self {
            client,
            endpoint,
            api_key,
        })
    }

    /// Builds one authenticated JSON request with the API key confined to a sensitive header.
    fn request_builder(&self, request: &WebSearchRequest) -> reqwest::RequestBuilder {
        self.client
            .post(self.endpoint.clone())
            .header(ACCEPT, "application/json")
            .header(API_KEY_HEADER, self.api_key.clone())
            .json(&ExaSearchRequest::from_native(request))
            .timeout(SEARCH_TIMEOUT)
    }

    #[cfg(test)]
    /// Builds a request for protocol assertions without sending it.
    pub(super) fn fixture_request(
        &self,
        request: &WebSearchRequest,
    ) -> Result<reqwest::Request, WebSearchError> {
        self.request_builder(request)
            .build()
            .map_err(|_| WebSearchError::internal())
    }
}

impl WebSearchProvider for ExaSearchProvider {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    async fn search(&self, request: WebSearchRequest) -> Result<WebSearchResponse, WebSearchError> {
        let response = self
            .request_builder(&request)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(map_status(response.status()));
        }
        validate_content_type(&response)?;
        let bytes = read_bounded_body(response).await?;
        decode_response(&bytes, &request)
    }
}

/// Fixed provider request mapped only from Bottie's validated native contract.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchRequest<'a> {
    query: &'a str,
    num_results: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    include_domains: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exclude_domains: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_published_date: Option<String>,
    moderation: bool,
    contents: ExaContents,
}

impl<'a> ExaSearchRequest<'a> {
    /// Maps native bounds to Exa fields without provider-specific model arguments.
    fn from_native(request: &'a WebSearchRequest) -> Self {
        Self {
            query: request.query(),
            num_results: request.result_limit(),
            include_domains: request
                .include_domains()
                .iter()
                .map(String::as_str)
                .collect(),
            exclude_domains: request
                .exclude_domains()
                .iter()
                .map(String::as_str)
                .collect(),
            start_published_date: request.freshness().map(exa_start_published_date),
            moderation: true,
            contents: ExaContents { highlights: true },
        }
    }
}

/// Bounded Exa content request used to produce inert result snippets.
#[derive(Serialize)]
struct ExaContents {
    highlights: bool,
}

/// Converts a provider-independent recency window into an absolute UTC lower bound.
fn exa_start_published_date(freshness: WebSearchFreshness) -> String {
    let days = match freshness {
        WebSearchFreshness::Day => 1,
        WebSearchFreshness::Week => 7,
        WebSearchFreshness::Month => 30,
        WebSearchFreshness::Year => 365,
    };
    (Utc::now() - TimeDelta::days(days)).to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Minimal response envelope required to normalize Exa results.
#[derive(Deserialize)]
struct ExaSearchResponse {
    #[serde(default)]
    results: Vec<ExaSearchResult>,
}

/// Provider fields retained by Bottie's Exa adapter.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    highlights: Vec<String>,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    text: String,
    published_date: Option<String>,
}

/// Requires JSON before any provider bytes are decoded.
fn validate_content_type(response: &Response) -> Result<(), WebSearchError> {
    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"));
    if is_json {
        Ok(())
    } else {
        Err(WebSearchError::malformed_response())
    }
}

/// Reads one response through a strict aggregate byte ceiling.
async fn read_bounded_body(response: Response) -> Result<Vec<u8>, WebSearchError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(WebSearchError::malformed_response());
    }
    let mut output = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(map_request_error)?;
        if output.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(WebSearchError::malformed_response());
        }
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

/// Decodes and bounds Exa JSON while dropping unsafe or policy-mismatched URLs.
fn decode_response(
    bytes: &[u8],
    request: &WebSearchRequest,
) -> Result<WebSearchResponse, WebSearchError> {
    let decoded: ExaSearchResponse =
        serde_json::from_slice(bytes).map_err(|_| WebSearchError::malformed_response())?;
    let results = decoded
        .results
        .into_iter()
        .filter_map(normalize_result)
        .filter(|result| request.allows_result_url(result.url()))
        .take(request.result_limit().min(MAX_WEB_SEARCH_RESULTS))
        .collect();
    Ok(WebSearchResponse {
        provider_id: PROVIDER_ID.into(),
        results,
    })
}

/// Converts one untrusted Exa result into bounded inert metadata.
fn normalize_result(result: ExaSearchResult) -> Option<WebSearchResult> {
    let title = normalize_text(&result.title, MAX_RESULT_TITLE_CHARS);
    let url = normalize_url(&result.url)?;
    if title.is_empty() {
        return None;
    }
    let snippet = if result.highlights.is_empty() {
        if result.summary.trim().is_empty() {
            result.text
        } else {
            result.summary
        }
    } else {
        result.highlights.join(" ")
    };
    Some(WebSearchResult {
        title,
        url,
        snippet: normalize_text(&snippet, MAX_RESULT_SNIPPET_CHARS),
        published_at: result
            .published_date
            .map(|value| normalize_text(&value, MAX_PUBLICATION_METADATA_CHARS))
            .filter(|value| !value.is_empty()),
    })
}

/// Accepts only bounded absolute HTTP(S) URLs without embedded credentials.
fn normalize_url(value: &str) -> Option<String> {
    if value.chars().count() > MAX_RESULT_URL_CHARS {
        return None;
    }
    let mut url = Url::parse(value.trim()).ok()?;
    let is_safe = matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none();
    if !is_safe {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

/// Collapses whitespace and truncates provider display text at a Unicode boundary.
fn normalize_text(value: &str, max_chars: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

/// Maps request-layer failures without retaining URLs, queries, or credentials.
fn map_request_error(error: reqwest::Error) -> WebSearchError {
    if error.is_timeout() {
        WebSearchError::timeout()
    } else {
        WebSearchError::unavailable()
    }
}

/// Maps Exa status without reading or reflecting its response body.
fn map_status(status: StatusCode) -> WebSearchError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => WebSearchError::credential_rejected(),
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            WebSearchError::request_rejected()
        }
        StatusCode::TOO_MANY_REQUESTS => WebSearchError::rate_limited(),
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => WebSearchError::timeout(),
        _ => WebSearchError::unavailable(),
    }
}

#[cfg(test)]
/// Decodes provider JSON for socket-free protocol fixtures.
pub(super) fn decode_fixture_response(
    bytes: &[u8],
    result_limit: usize,
) -> Result<WebSearchResponse, WebSearchError> {
    let request = WebSearchRequest::new("fixture", result_limit)?;
    decode_response(bytes, &request)
}

#[cfg(test)]
/// Decodes provider JSON against one filtered request for policy fixtures.
pub(super) fn decode_filtered_fixture_response(
    bytes: &[u8],
    request: &WebSearchRequest,
) -> Result<WebSearchResponse, WebSearchError> {
    decode_response(bytes, request)
}

#[cfg(test)]
/// Maps a numeric fixture status through Exa's redacted policy.
pub(super) fn map_fixture_status(status: u16) -> WebSearchError {
    StatusCode::from_u16(status)
        .map(map_status)
        .unwrap_or_else(|_| WebSearchError::unavailable())
}
