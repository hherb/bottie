//! Brave Search API adapter behind the provider-neutral native search contract.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{
    Client, Response, StatusCode,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use serde::Deserialize;
use url::Url;

use super::{
    MAX_WEB_SEARCH_RESULTS, WebSearchError, WebSearchFreshness, WebSearchProvider,
    WebSearchRequest, WebSearchResponse, WebSearchResult,
};

const PROVIDER_ID: &str = "brave";
const BRAVE_SEARCH_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";
const SUBSCRIPTION_TOKEN_HEADER: &str = "x-subscription-token";
const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_RESPONSE_BYTES: usize = 2 * 1_024 * 1_024;
const MAX_RESULT_TITLE_CHARS: usize = 512;
const MAX_RESULT_URL_CHARS: usize = 4_096;
const MAX_RESULT_SNIPPET_CHARS: usize = 4_096;
const MAX_PUBLICATION_METADATA_CHARS: usize = 128;

/// Authenticated Rust-owned Brave Search adapter.
#[derive(Clone)]
pub struct BraveSearchProvider {
    client: Client,
    endpoint: Url,
    api_key: HeaderValue,
}

impl BraveSearchProvider {
    /// Builds the fixed-endpoint Brave adapter without exposing the supplied credential.
    pub fn new(api_key: impl Into<String>) -> Result<Self, WebSearchError> {
        let endpoint = Url::parse(BRAVE_SEARCH_ENDPOINT).map_err(|_| WebSearchError::internal())?;
        Self::build(endpoint, api_key.into())
    }

    #[cfg(test)]
    /// Builds a test-only adapter for an isolated loopback HTTP fixture.
    pub(super) fn for_loopback_fixture(
        base_url: &str,
        api_key: &str,
    ) -> Result<Self, WebSearchError> {
        let endpoint = Url::parse(base_url)
            .and_then(|base| base.join("res/v1/web/search"))
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
}

impl WebSearchProvider for BraveSearchProvider {
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

impl BraveSearchProvider {
    /// Builds one authenticated request with credential material confined to a sensitive header.
    fn request_builder(&self, request: &WebSearchRequest) -> reqwest::RequestBuilder {
        self.client
            .get(search_endpoint(&self.endpoint, request))
            .header(ACCEPT, "application/json")
            .header(SUBSCRIPTION_TOKEN_HEADER, self.api_key.clone())
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

/// Appends the bounded provider-native query without putting credentials in the URL.
fn search_endpoint(base: &Url, request: &WebSearchRequest) -> Url {
    let mut endpoint = base.clone();
    endpoint
        .query_pairs_mut()
        .append_pair("q", &request.provider_query())
        .append_pair("count", &request.result_limit().to_string())
        .append_pair("result_filter", "web")
        .append_pair("safesearch", "strict")
        .append_pair("text_decorations", "false");
    if let Some(freshness) = request.freshness() {
        endpoint
            .query_pairs_mut()
            .append_pair("freshness", brave_freshness(freshness));
    }
    endpoint
}

/// Maps Bottie's provider-independent recency windows to Brave's fixed parameter values.
fn brave_freshness(freshness: WebSearchFreshness) -> &'static str {
    match freshness {
        WebSearchFreshness::Day => "pd",
        WebSearchFreshness::Week => "pw",
        WebSearchFreshness::Month => "pm",
        WebSearchFreshness::Year => "py",
    }
}

/// Minimal response envelope required to normalize standard web results.
#[derive(Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveWebResults>,
}

/// Ordered standard web results from a Brave response.
#[derive(Deserialize)]
struct BraveWebResults {
    results: Vec<BraveWebResult>,
}

/// Provider fields retained by Bottie's first web-search boundary.
#[derive(Deserialize)]
struct BraveWebResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
    page_age: Option<String>,
}

/// Requires a JSON response before any provider bytes are decoded.
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

/// Decodes and bounds provider JSON while dropping unsafe result URLs.
fn decode_response(
    bytes: &[u8],
    request: &WebSearchRequest,
) -> Result<WebSearchResponse, WebSearchError> {
    let decoded: BraveSearchResponse =
        serde_json::from_slice(bytes).map_err(|_| WebSearchError::malformed_response())?;
    let results = decoded
        .web
        .map(|web| web.results)
        .unwrap_or_default()
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

/// Converts one untrusted provider result into bounded inert metadata.
fn normalize_result(result: BraveWebResult) -> Option<WebSearchResult> {
    let title = normalize_text(&result.title, MAX_RESULT_TITLE_CHARS);
    let url = normalize_url(&result.url)?;
    if title.is_empty() {
        return None;
    }
    Some(WebSearchResult {
        title,
        url,
        snippet: normalize_text(&result.description, MAX_RESULT_SNIPPET_CHARS),
        published_at: result
            .page_age
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

/// Maps provider HTTP status without reading or reflecting the response body.
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
/// Maps a numeric fixture status through the provider's redacted policy.
pub(super) fn map_fixture_status(status: u16) -> WebSearchError {
    StatusCode::from_u16(status)
        .map(map_status)
        .unwrap_or_else(|_| WebSearchError::unavailable())
}
