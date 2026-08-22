//! Provider-neutral native web-search boundary and concrete adapters.

mod brave;

#[cfg(test)]
mod tests;

use serde::Serialize;

pub use brave::BraveSearchProvider;

/// Maximum Unicode-scalar length accepted for one provider-neutral search query.
pub const MAX_WEB_SEARCH_QUERY_CHARS: usize = 400;
/// Maximum whitespace-delimited terms accepted for one provider-neutral search query.
pub const MAX_WEB_SEARCH_QUERY_WORDS: usize = 50;
/// Maximum number of normalized web results accepted from one provider call.
pub const MAX_WEB_SEARCH_RESULTS: usize = 20;

/// One validated provider-neutral web-search request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSearchRequest {
    query: String,
    result_limit: usize,
}

impl WebSearchRequest {
    /// Validates and normalizes one query and result limit before provider routing.
    pub fn new(query: impl Into<String>, result_limit: usize) -> Result<Self, WebSearchError> {
        let query = normalize_whitespace(&query.into());
        if query.is_empty() {
            return Err(WebSearchError::invalid_request(
                "Enter a non-empty web-search query.",
            ));
        }
        if query.chars().count() > MAX_WEB_SEARCH_QUERY_CHARS {
            return Err(WebSearchError::invalid_request(format!(
                "Web-search queries are limited to {MAX_WEB_SEARCH_QUERY_CHARS} characters."
            )));
        }
        if query.split_whitespace().count() > MAX_WEB_SEARCH_QUERY_WORDS {
            return Err(WebSearchError::invalid_request(format!(
                "Web-search queries are limited to {MAX_WEB_SEARCH_QUERY_WORDS} words."
            )));
        }
        if !(1..=MAX_WEB_SEARCH_RESULTS).contains(&result_limit) {
            return Err(WebSearchError::invalid_request(format!(
                "Web-search result limits must be between 1 and {MAX_WEB_SEARCH_RESULTS}."
            )));
        }
        Ok(Self {
            query,
            result_limit,
        })
    }

    /// Returns the normalized query retained only inside native provider work.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the bounded number of results requested from the provider.
    pub fn result_limit(&self) -> usize {
        self.result_limit
    }
}

/// One normalized search result retained behind the native boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResult {
    title: String,
    url: String,
    snippet: String,
    published_at: Option<String>,
}

impl WebSearchResult {
    /// Returns the provider-supplied result title after bounded normalization.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the absolute HTTP(S) source URL accepted by native policy.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the provider-supplied inert result excerpt after bounded normalization.
    pub fn snippet(&self) -> &str {
        &self.snippet
    }

    /// Returns optional provider publication metadata without interpreting it as trusted.
    pub fn published_at(&self) -> Option<&str> {
        self.published_at.as_deref()
    }
}

/// One normalized response from a concrete native search provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResponse {
    provider_id: String,
    results: Vec<WebSearchResult>,
}

impl WebSearchResponse {
    /// Returns the stable provider identity that produced this result set.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the ordered bounded normalized web results.
    pub fn results(&self) -> &[WebSearchResult] {
        &self.results
    }
}

/// Stable native failure categories shared by web-search provider adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchErrorCode {
    /// The native caller supplied an invalid bounded request.
    InvalidRequest,
    /// The selected provider requires a credential that is not configured.
    CredentialRequired,
    /// The provider rejected the configured credential.
    CredentialRejected,
    /// The provider refused the request because a quota or rate limit was reached.
    RateLimited,
    /// The provider request exceeded its native time limit.
    Timeout,
    /// The provider could not be reached or was temporarily unable to serve the request.
    Unavailable,
    /// The provider returned an invalid or unsafe response.
    MalformedResponse,
    /// Native provider initialization failed before a request could be sent.
    Internal,
}

/// Stable redacted web-search failure that never contains a query, credential, or provider body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchError {
    /// Stable machine-readable failure category.
    pub code: WebSearchErrorCode,
    /// Fixed user-readable summary without raw provider material.
    pub message: String,
    /// Whether repeating the same request later may succeed.
    pub retryable: bool,
}

impl WebSearchError {
    /// Builds a fixed invalid-request failure.
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: WebSearchErrorCode::InvalidRequest,
            message: message.into(),
            retryable: false,
        }
    }

    /// Builds a fixed unavailable-provider failure.
    fn unavailable() -> Self {
        Self {
            code: WebSearchErrorCode::Unavailable,
            message: "The web-search provider is unavailable.".into(),
            retryable: true,
        }
    }

    /// Builds a fixed missing-credential failure.
    fn credential_required() -> Self {
        Self {
            code: WebSearchErrorCode::CredentialRequired,
            message: "Configure a web-search provider credential before using web search.".into(),
            retryable: false,
        }
    }

    /// Builds a fixed rejected-credential failure.
    fn credential_rejected() -> Self {
        Self {
            code: WebSearchErrorCode::CredentialRejected,
            message: "The web-search provider rejected the configured credential.".into(),
            retryable: false,
        }
    }

    /// Builds a fixed provider-rate-limit failure.
    fn rate_limited() -> Self {
        Self {
            code: WebSearchErrorCode::RateLimited,
            message: "The web-search provider is temporarily rate limited.".into(),
            retryable: true,
        }
    }

    /// Builds a fixed provider-timeout failure.
    fn timeout() -> Self {
        Self {
            code: WebSearchErrorCode::Timeout,
            message: "The web-search provider timed out.".into(),
            retryable: true,
        }
    }

    /// Builds a fixed malformed-response failure.
    fn malformed_response() -> Self {
        Self {
            code: WebSearchErrorCode::MalformedResponse,
            message: "The web-search provider returned an invalid response.".into(),
            retryable: false,
        }
    }

    /// Builds a fixed provider-request-rejection failure.
    fn request_rejected() -> Self {
        Self {
            code: WebSearchErrorCode::InvalidRequest,
            message: "The web-search provider rejected the bounded request.".into(),
            retryable: false,
        }
    }

    /// Builds a fixed native-initialization failure.
    fn internal() -> Self {
        Self {
            code: WebSearchErrorCode::Internal,
            message: "Bottie could not initialize the web-search provider.".into(),
            retryable: false,
        }
    }
}

/// Collapses provider-neutral query whitespace without changing non-whitespace content.
fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pluggable native contract implemented by each concrete web-search provider.
pub trait WebSearchProvider: Clone + Send + Sync + 'static {
    /// Returns the stable provider routing identity.
    fn provider_id(&self) -> &'static str;

    /// Executes one validated provider-neutral search and normalizes its result set.
    fn search(
        &self,
        request: WebSearchRequest,
    ) -> impl std::future::Future<Output = Result<WebSearchResponse, WebSearchError>> + Send;
}
