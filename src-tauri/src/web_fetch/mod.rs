//! Bounded native retrieval of public UTF-8 web page source.

mod errors;
#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use reqwest::{
    Client, Response, StatusCode,
    header::{ACCEPT, CONTENT_TYPE, LOCATION},
};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

pub(crate) use errors::{WebFetchError, WebFetchErrorCode};
use errors::{
    blocked_address, internal_error, invalid_request, malformed_response, map_request_error,
    redirect_error, response_too_large, unsupported_content_type,
};

/// Stable name of the provider-independent native web-fetch tool.
pub(crate) const WEB_FETCH_TOOL_NAME: &str = "web_fetch";
/// Maximum Unicode-scalar length accepted for one requested public URL.
pub(crate) const MAX_WEB_FETCH_URL_CHARS: usize = 4_096;
/// Maximum response bytes retained before common-envelope serialization.
pub(crate) const MAX_WEB_FETCH_RESPONSE_BYTES: usize = 48 * 1_024;
/// Maximum number of explicitly revalidated HTTP redirects.
pub(crate) const MAX_WEB_FETCH_REDIRECTS: usize = 3;

const WEB_FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const WEB_FETCH_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);
const WEB_FETCH_ACCEPT: &str = "text/html, text/plain, application/xhtml+xml";

/// Exact typed arguments accepted by Bottie's provider-independent web-fetch tool.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct WebFetchArguments {
    /// Absolute public HTTP(S) URL selected by the model.
    pub(crate) url: String,
}

impl WebFetchArguments {
    /// Converts validated arguments into a normalized native request.
    pub(crate) fn into_request(self) -> Result<WebFetchRequest, WebFetchError> {
        WebFetchRequest::new(self.url)
    }
}

/// One normalized public URL accepted by the native fetch boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebFetchRequest {
    url: Url,
}

impl WebFetchRequest {
    /// Validates a public HTTP(S) URL, removes its fragment, and forbids embedded authority.
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, WebFetchError> {
        normalize_public_url(&value.into()).map(|url| Self { url })
    }

    #[cfg(test)]
    /// Returns the normalized URL retained only for native network work and dispatcher fixtures.
    pub(crate) fn url(&self) -> &str {
        self.url.as_str()
    }

    #[cfg(test)]
    /// Builds a loopback-only request for isolated native HTTP fixtures.
    fn for_loopback_fixture(value: &str) -> Self {
        let mut url = Url::parse(value).expect("fixture URL should parse");
        url.set_fragment(None);
        Self { url }
    }
}

/// Bounded page source returned after every redirect and response check succeeds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebFetchResponse {
    final_url: String,
    content_type: String,
    content: String,
    untrusted: bool,
}

#[cfg(test)]
/// Builds one bounded response for provider-neutral dispatcher fixtures.
pub(crate) fn fixture_web_fetch_response() -> WebFetchResponse {
    WebFetchResponse {
        final_url: "https://www.iana.org/release".into(),
        content_type: "text/html".into(),
        content: "<p>Bounded fixture page.</p>".into(),
        untrusted: true,
    }
}

/// Native page-source client with production public-network enforcement.
#[derive(Clone, Debug)]
pub(crate) struct NativeWebFetch {
    total_timeout: Duration,
    connect_timeout: Duration,
    #[cfg(test)]
    allow_loopback_fixture: bool,
}

impl Default for NativeWebFetch {
    fn default() -> Self {
        Self {
            total_timeout: WEB_FETCH_TOTAL_TIMEOUT,
            connect_timeout: WEB_FETCH_CONNECT_TIMEOUT,
            #[cfg(test)]
            allow_loopback_fixture: false,
        }
    }
}

impl NativeWebFetch {
    /// Creates a public-network-only fetcher with no credentials, cookies, proxy, or redirects.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    /// Creates an isolated loopback fixture client unavailable to production builds.
    fn for_loopback_fixture() -> Self {
        Self {
            total_timeout: WEB_FETCH_TOTAL_TIMEOUT,
            connect_timeout: WEB_FETCH_CONNECT_TIMEOUT,
            allow_loopback_fixture: true,
        }
    }

    #[cfg(test)]
    /// Creates a loopback client with one short deterministic fixture deadline.
    fn for_timeout_fixture(timeout: Duration) -> Self {
        Self {
            total_timeout: timeout,
            connect_timeout: timeout,
            allow_loopback_fixture: true,
        }
    }

    /// Returns whether test-only private destinations are allowed.
    fn allow_non_public(&self) -> bool {
        #[cfg(test)]
        {
            self.allow_loopback_fixture
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    /// Sends one redirect-disabled request after resolving and pinning all accepted addresses.
    async fn send(&self, url: &Url, deadline: Instant) -> Result<Response, WebFetchError> {
        let addresses = resolve_addresses(url, self.allow_non_public(), deadline).await?;
        let remaining = remaining_timeout(deadline)?;
        let host = url.host_str().ok_or_else(invalid_request)?;
        let client = Client::builder()
            .connect_timeout(self.connect_timeout.min(remaining))
            .read_timeout(remaining)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| internal_error())?;
        client
            .get(url.clone())
            .header(ACCEPT, WEB_FETCH_ACCEPT)
            .timeout(remaining)
            .send()
            .await
            .map_err(map_request_error)
    }
}

/// Pluggable fetch boundary used by the common native dispatcher and deterministic tests.
pub(crate) trait WebFetchProvider: Clone + Send + Sync + 'static {
    /// Fetches one already validated request under native redirect and body policy.
    fn fetch(
        &self,
        request: WebFetchRequest,
    ) -> impl std::future::Future<Output = Result<WebFetchResponse, WebFetchError>> + Send;
}

impl WebFetchProvider for NativeWebFetch {
    async fn fetch(&self, request: WebFetchRequest) -> Result<WebFetchResponse, WebFetchError> {
        let deadline = Instant::now() + self.total_timeout;
        let mut current = request.url;
        for followed in 0..=MAX_WEB_FETCH_REDIRECTS {
            let response = self.send(&current, deadline).await?;
            if is_redirect(response.status()) {
                if followed == MAX_WEB_FETCH_REDIRECTS {
                    return Err(redirect_error());
                }
                current = redirect_target(&current, &response, self.allow_non_public())?;
                continue;
            }
            if !response.status().is_success() {
                return Err(WebFetchError::unavailable());
            }
            let content_type = accepted_content_type(&response)?;
            let bytes = read_bounded_body(response).await?;
            let content = String::from_utf8(bytes)
                .map_err(|_| malformed_response())?
                .trim_start_matches('\u{feff}')
                .to_owned();
            return Ok(WebFetchResponse {
                final_url: current.to_string(),
                content_type,
                content,
                untrusted: true,
            });
        }
        Err(redirect_error())
    }
}

/// Parses one URL under the model-visible public-web contract.
fn normalize_public_url(value: &str) -> Result<Url, WebFetchError> {
    if value.trim().is_empty() || value.chars().count() > MAX_WEB_FETCH_URL_CHARS {
        return Err(invalid_request());
    }
    let mut url = Url::parse(value.trim()).map_err(|_| invalid_request())?;
    let Host::Domain(host) = url.host().ok_or_else(invalid_request)? else {
        return Err(invalid_request());
    };
    let default_port = match url.scheme() {
        "http" => 80,
        "https" => 443,
        _ => return Err(invalid_request()),
    };
    if !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != default_port)
        || !valid_public_dns_name(host)
    {
        return Err(invalid_request());
    }
    url.set_fragment(None);
    Ok(url)
}

/// Rejects special-use names before DNS and requires ordinary multi-label DNS syntax.
fn valid_public_dns_name(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let forbidden_names = ["example.com", "example.net", "example.org"];
    let forbidden_suffixes = [
        ".alt",
        ".arpa",
        ".example",
        ".home",
        ".internal",
        ".invalid",
        ".lan",
        ".local",
        ".localhost",
        ".onion",
        ".test",
    ];
    host.contains('.')
        && host.len() <= 253
        && !forbidden_names.contains(&host.as_str())
        && !forbidden_suffixes
            .iter()
            .any(|suffix| host == &suffix[1..] || host.ends_with(suffix))
        && host.split('.').all(valid_dns_label)
}

/// Validates one ASCII DNS label after URL parsing has applied IDNA conversion.
fn valid_dns_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
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

/// Resolves once, rejects any non-public answer, and returns a deduplicated pinned set.
async fn resolve_addresses(
    url: &Url,
    allow_non_public: bool,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, WebFetchError> {
    let host = url.host_str().ok_or_else(invalid_request)?;
    let port = url.port_or_known_default().ok_or_else(invalid_request)?;
    let mut seen = HashSet::new();
    let addresses = tokio::time::timeout(
        remaining_timeout(deadline)?,
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| errors::timeout())?
    .map_err(|_| WebFetchError::unavailable())?
    .filter(|address| seen.insert(*address))
    .collect::<Vec<_>>();
    if addresses.is_empty()
        || (!allow_non_public && addresses.iter().any(|address| !is_public_ip(address.ip())))
    {
        return Err(blocked_address());
    }
    Ok(addresses)
}

/// Fail-closed public-address classification for resolved IPv4 and IPv6 destinations.
fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

/// Rejects private, loopback, link-local, documentation, carrier, multicast, and reserved IPv4.
fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

/// Rejects local, mapped, translation, documentation, transition, multicast, and protocol IPv6.
fn is_public_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && address.to_ipv4_mapped().is_none()
        && segments[..6].iter().any(|segment| *segment != 0)
        && (segments[0] & 0xfe00) != 0xfc00
        && (segments[0] & 0xffc0) != 0xfe80
        && (segments[0] & 0xffc0) != 0xfec0
        && !(segments[0] == 0x0100
            && segments[1] == 0
            && segments[2] == 0
            && matches!(segments[3], 0 | 1))
        && !(segments[0] == 0x2001 && segments[1] < 0x0200)
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
        && segments[0] != 0x2002
        && !(segments[0] == 0x3fff && (segments[1] & 0xf000) == 0)
        && segments[0] != 0x5f00
        && !(segments[0] == 0x0064 && segments[1] == 0xff9b)
}

/// Returns whether a response requires one explicitly validated redirect hop.
fn is_redirect(status: StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}

/// Resolves a relative redirect and reapplies the complete URL policy before the next request.
fn redirect_target(
    current: &Url,
    response: &Response,
    allow_non_public: bool,
) -> Result<Url, WebFetchError> {
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(redirect_error)?;
    let target = current.join(location).map_err(|_| redirect_error())?;
    if allow_non_public {
        let mut target = target;
        if !matches!(target.scheme(), "http" | "https")
            || target.host().is_none()
            || !target.username().is_empty()
            || target.password().is_some()
        {
            return Err(redirect_error());
        }
        target.set_fragment(None);
        Ok(target)
    } else {
        normalize_public_url(target.as_str()).map_err(|_| redirect_error())
    }
}

/// Accepts only UTF-8 HTML, XHTML, or plain text and returns the normalized media type.
fn accepted_content_type(response: &Response) -> Result<String, WebFetchError> {
    let raw = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(unsupported_content_type)?;
    let mut parts = raw.split(';');
    let media_type = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    if !matches!(
        media_type.as_str(),
        "text/html" | "text/plain" | "application/xhtml+xml"
    ) {
        return Err(unsupported_content_type());
    }
    for parameter in parts {
        let Some((name, value)) = parameter.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("charset")
            && !matches!(
                value.trim().trim_matches('"').to_ascii_lowercase().as_str(),
                "utf-8" | "utf8"
            )
        {
            return Err(unsupported_content_type());
        }
    }
    Ok(media_type)
}

/// Streams the response through the pre-serialization byte ceiling.
async fn read_bounded_body(response: Response) -> Result<Vec<u8>, WebFetchError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WEB_FETCH_RESPONSE_BYTES as u64)
    {
        return Err(response_too_large());
    }
    let mut output = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(map_request_error)?;
        if output.len().saturating_add(chunk.len()) > MAX_WEB_FETCH_RESPONSE_BYTES {
            return Err(response_too_large());
        }
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

/// Returns the remaining total operation time or a fixed timeout error.
fn remaining_timeout(deadline: Instant) -> Result<Duration, WebFetchError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|value| !value.is_zero())
        .ok_or_else(errors::timeout)
}
