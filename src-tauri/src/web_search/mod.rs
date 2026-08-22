//! Provider-neutral native web-search boundary and concrete adapters.

mod brave;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use url::{Host, Url};

pub use brave::BraveSearchProvider;

/// Maximum Unicode-scalar length accepted for one provider-neutral search query.
pub const MAX_WEB_SEARCH_QUERY_CHARS: usize = 400;
/// Maximum whitespace-delimited terms accepted for one provider-neutral search query.
pub const MAX_WEB_SEARCH_QUERY_WORDS: usize = 50;
/// Maximum number of normalized web results accepted from one provider call.
pub const MAX_WEB_SEARCH_RESULTS: usize = 20;
/// Default result ceiling used by the model-visible web-search tool.
pub(crate) const DEFAULT_WEB_SEARCH_TOOL_RESULTS: usize = 5;
/// Maximum results returned by one model-visible web-search tool call.
pub(crate) const MAX_WEB_SEARCH_TOOL_RESULTS: usize = 10;
/// Maximum combined allowlisted and blocklisted domain filters per search.
pub(crate) const MAX_WEB_SEARCH_FILTER_DOMAINS: usize = 5;
/// Maximum Unicode-scalar length accepted for one normalized domain filter.
pub(crate) const MAX_WEB_SEARCH_DOMAIN_CHARS: usize = 253;
/// Stable identity of the first configured native web-search provider.
pub const BRAVE_SEARCH_PROVIDER_ID: &str = "brave";
/// Stable name of the provider-independent web-search tool contract.
pub(crate) const WEB_SEARCH_TOOL_NAME: &str = "web_search";

const CONNECTION_TEST_QUERY: &str = "Bottie connection test";
const CONNECTION_TEST_RESULT_LIMIT: usize = 1;

/// Provider-independent recency windows exposed by the native tool contract.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WebSearchFreshness {
    /// Limit results to roughly the previous 24 hours.
    Day,
    /// Limit results to roughly the previous seven days.
    Week,
    /// Limit results to roughly the previous month.
    Month,
    /// Limit results to roughly the previous year.
    Year,
}

/// Exact typed arguments accepted by Bottie's provider-independent web-search tool.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebSearchArguments {
    /// Natural-language terms sent only to the configured native search provider.
    pub(crate) query: String,
    /// Optional provider-independent recency window.
    pub(crate) freshness: Option<WebSearchFreshness>,
    /// Optional exact domains or parent domains allowed in returned URLs.
    #[serde(default)]
    pub(crate) include_domains: Vec<String>,
    /// Optional exact domains or parent domains removed from returned URLs.
    #[serde(default)]
    pub(crate) exclude_domains: Vec<String>,
    /// Optional model-selected result ceiling.
    pub(crate) limit: Option<usize>,
}

impl WebSearchArguments {
    /// Converts validated typed arguments into the native provider request contract.
    pub(crate) fn into_request(self) -> Result<WebSearchRequest, WebSearchError> {
        WebSearchRequest::with_filters(
            self.query,
            self.limit.unwrap_or(DEFAULT_WEB_SEARCH_TOOL_RESULTS),
            self.freshness,
            self.include_domains,
            self.exclude_domains,
        )
    }
}

/// One validated provider-neutral web-search request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSearchRequest {
    query: String,
    result_limit: usize,
    freshness: Option<WebSearchFreshness>,
    include_domains: Vec<String>,
    exclude_domains: Vec<String>,
}

impl WebSearchRequest {
    /// Validates and normalizes one query and result limit before provider routing.
    pub fn new(query: impl Into<String>, result_limit: usize) -> Result<Self, WebSearchError> {
        Self::with_filters(query, result_limit, None, Vec::new(), Vec::new())
    }

    /// Validates one query plus provider-independent freshness and domain filters.
    pub(crate) fn with_filters(
        query: impl Into<String>,
        result_limit: usize,
        freshness: Option<WebSearchFreshness>,
        include_domains: Vec<String>,
        exclude_domains: Vec<String>,
    ) -> Result<Self, WebSearchError> {
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
        let include_domains = normalize_domains(include_domains)?;
        let exclude_domains = normalize_domains(exclude_domains)?;
        if include_domains.len().saturating_add(exclude_domains.len())
            > MAX_WEB_SEARCH_FILTER_DOMAINS
            || include_domains
                .iter()
                .any(|domain| exclude_domains.contains(domain))
        {
            return Err(WebSearchError::invalid_request(
                "Web-search domain filters conflict or exceed their limit.",
            ));
        }
        let request = Self {
            query,
            result_limit,
            freshness,
            include_domains,
            exclude_domains,
        };
        request.validate_provider_query()?;
        Ok(request)
    }

    /// Returns the normalized query retained only inside native provider work.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the bounded number of results requested from the provider.
    pub fn result_limit(&self) -> usize {
        self.result_limit
    }

    /// Returns the optional provider-independent recency window.
    pub(crate) fn freshness(&self) -> Option<WebSearchFreshness> {
        self.freshness
    }

    /// Returns normalized exact or parent domains allowed in result URLs.
    #[cfg(test)]
    pub(crate) fn include_domains(&self) -> &[String] {
        &self.include_domains
    }

    /// Returns normalized exact or parent domains removed from result URLs.
    #[cfg(test)]
    pub(crate) fn exclude_domains(&self) -> &[String] {
        &self.exclude_domains
    }

    /// Builds the bounded provider query with model arguments separated from native operators.
    pub(crate) fn provider_query(&self) -> String {
        let mut parts = vec![self.query.clone()];
        if self.include_domains.len() == 1 {
            parts.push(format!("site:{}", self.include_domains[0]));
        } else if !self.include_domains.is_empty() {
            let alternatives = self
                .include_domains
                .iter()
                .map(|domain| format!("site:{domain}"))
                .collect::<Vec<_>>()
                .join(" OR ");
            parts.push(format!("({alternatives})"));
        }
        parts.extend(
            self.exclude_domains
                .iter()
                .map(|domain| format!("NOT site:{domain}")),
        );
        parts.join(" ")
    }

    /// Rechecks provider output against native include and exclude domain policy.
    pub(crate) fn allows_result_url(&self, value: &str) -> bool {
        let Some(host) = Url::parse(value)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
        else {
            return false;
        };
        let included = self.include_domains.is_empty()
            || self
                .include_domains
                .iter()
                .any(|domain| domain_matches(&host, domain));
        included
            && !self
                .exclude_domains
                .iter()
                .any(|domain| domain_matches(&host, domain))
    }

    /// Ensures native-added operators stay within the provider's complete query limits.
    fn validate_provider_query(&self) -> Result<(), WebSearchError> {
        let query = self.provider_query();
        if query.chars().count() > MAX_WEB_SEARCH_QUERY_CHARS
            || query.split_whitespace().count() > MAX_WEB_SEARCH_QUERY_WORDS
        {
            Err(WebSearchError::invalid_request(
                "The web-search query and filters exceed the provider request limit.",
            ))
        } else {
            Ok(())
        }
    }
}

/// Normalizes and validates exact public DNS domain filters without accepting URLs or IP addresses.
fn normalize_domains(domains: Vec<String>) -> Result<Vec<String>, WebSearchError> {
    let mut normalized = Vec::with_capacity(domains.len());
    for domain in domains {
        let domain = domain.trim().trim_end_matches('.');
        if domain.is_empty()
            || domain.chars().count() > MAX_WEB_SEARCH_DOMAIN_CHARS
            || !domain.contains('.')
        {
            return Err(WebSearchError::invalid_request(
                "Use bounded public DNS names for web-search domain filters.",
            ));
        }
        let Host::Domain(domain) = Host::parse(domain).map_err(|_| {
            WebSearchError::invalid_request(
                "Use bounded public DNS names for web-search domain filters.",
            )
        })?
        else {
            return Err(WebSearchError::invalid_request(
                "Use bounded public DNS names for web-search domain filters.",
            ));
        };
        let domain = domain.to_ascii_lowercase();
        if !domain.split('.').all(valid_domain_label) {
            return Err(WebSearchError::invalid_request(
                "Use bounded public DNS names for web-search domain filters.",
            ));
        }
        if normalized.contains(&domain) {
            return Err(WebSearchError::invalid_request(
                "Web-search domain filters must be unique.",
            ));
        }
        normalized.push(domain);
    }
    Ok(normalized)
}

/// Applies the DNS hostname label subset accepted by Bottie's model-visible filter contract.
fn valid_domain_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && label
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && label
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

/// Returns whether one result host is the selected domain or one of its subdomains.
fn domain_matches(host: &str, domain: &str) -> bool {
    host.eq_ignore_ascii_case(domain)
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
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

#[cfg(test)]
/// Builds one bounded provider-neutral result for dispatcher fixtures.
pub(crate) fn fixture_web_search_response() -> WebSearchResponse {
    WebSearchResponse {
        provider_id: "fixture".into(),
        results: vec![WebSearchResult {
            title: "Fixture result".into(),
            url: "https://example.com/result".into(),
            snippet: "Bounded fixture content.".into(),
            published_at: None,
        }],
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

/// Builds the fixed bounded request used to verify Brave connectivity without exposing results.
pub(crate) fn connection_test_request() -> WebSearchRequest {
    WebSearchRequest::new(CONNECTION_TEST_QUERY, CONNECTION_TEST_RESULT_LIMIT)
        .expect("the fixed web-search connection probe must remain valid")
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
