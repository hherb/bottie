//! Durable user-configurable restrictions layered over Bottie's fixed public-Web baseline.

use serde::{Deserialize, Serialize};
use url::{Host, Url};

/// Maximum combined allowlisted and blocklisted domains in the durable Web policy.
pub(crate) const MAX_WEB_POLICY_DOMAINS: usize = 32;
const MAX_WEB_POLICY_DOMAIN_CHARS: usize = 253;
const INVALID_POLICY_MESSAGE: &str =
    "Use unique public DNS names for Web policy domains, within the saved limit.";

/// Secret-free Web destination restrictions persisted with provider settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebNetworkPolicy {
    /// Reject plaintext HTTP result and fetch destinations when enabled.
    #[serde(default = "default_https_only")]
    pub https_only: bool,
    /// Optional parent domains that constrain every accepted Web destination.
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    /// Parent domains rejected after allowlist evaluation.
    #[serde(default)]
    pub blocked_domains: Vec<String>,
}

impl Default for WebNetworkPolicy {
    fn default() -> Self {
        Self {
            https_only: true,
            allowed_domains: Vec::new(),
            blocked_domains: Vec::new(),
        }
    }
}

impl WebNetworkPolicy {
    /// Builds the fixed public HTTP(S) baseline used before a saved policy is applied.
    pub(crate) fn public_http_and_https() -> Self {
        Self {
            https_only: false,
            allowed_domains: Vec::new(),
            blocked_domains: Vec::new(),
        }
    }

    /// Normalizes IDNA domains, bounds the saved policy, and rejects exact conflicts.
    pub(crate) fn normalized(self) -> Result<Self, WebPolicyError> {
        let allowed_domains = normalize_domains(self.allowed_domains)?;
        let blocked_domains = normalize_domains(self.blocked_domains)?;
        if allowed_domains.len().saturating_add(blocked_domains.len()) > MAX_WEB_POLICY_DOMAINS
            || allowed_domains
                .iter()
                .any(|domain| blocked_domains.contains(domain))
        {
            return Err(WebPolicyError);
        }
        Ok(Self {
            https_only: self.https_only,
            allowed_domains,
            blocked_domains,
        })
    }

    /// Applies HTTPS, public-host, allowlist, and blocklist rules to one already parsed URL.
    pub(crate) fn allows_url(&self, url: &Url) -> bool {
        if !matches!(url.scheme(), "http" | "https") || (self.https_only && url.scheme() != "https")
        {
            return false;
        }
        let Some(Host::Domain(host)) = url.host() else {
            return false;
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if !valid_public_domain(&host) {
            return false;
        }
        let included = self.allowed_domains.is_empty()
            || self
                .allowed_domains
                .iter()
                .any(|domain| domain_matches(&host, domain));
        included
            && !self
                .blocked_domains
                .iter()
                .any(|domain| domain_matches(&host, domain))
    }
}

/// Fixed validation failure without reflecting a submitted domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebPolicyError;

impl WebPolicyError {
    /// Returns the stable user-facing validation summary.
    pub(crate) fn message(self) -> &'static str {
        INVALID_POLICY_MESSAGE
    }
}

/// Normalizes one ordered domain list without silently deduplicating user input.
fn normalize_domains(domains: Vec<String>) -> Result<Vec<String>, WebPolicyError> {
    let mut normalized = Vec::with_capacity(domains.len());
    for domain in domains {
        let domain = domain.trim().trim_end_matches('.');
        if domain.is_empty() || domain.chars().count() > MAX_WEB_POLICY_DOMAIN_CHARS {
            return Err(WebPolicyError);
        }
        let Host::Domain(domain) = Host::parse(domain).map_err(|_| WebPolicyError)? else {
            return Err(WebPolicyError);
        };
        let domain = domain.to_ascii_lowercase();
        if !valid_public_domain(&domain) || normalized.contains(&domain) {
            return Err(WebPolicyError);
        }
        normalized.push(domain);
    }
    Ok(normalized)
}

/// Rejects single-label, special-use, and malformed DNS names before policy matching.
fn valid_public_domain(domain: &str) -> bool {
    const FORBIDDEN_SUFFIXES: [&str; 11] = [
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
    domain.contains('.')
        && domain.len() <= MAX_WEB_POLICY_DOMAIN_CHARS
        && !FORBIDDEN_SUFFIXES
            .iter()
            .any(|suffix| domain == &suffix[1..] || domain.ends_with(suffix))
        && domain.split('.').all(valid_domain_label)
}

/// Applies Bottie's accepted ASCII DNS-label subset after URL/Host IDNA conversion.
fn valid_domain_label(label: &str) -> bool {
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

/// Matches an exact parent domain or any DNS-label-boundary subdomain.
fn domain_matches(host: &str, domain: &str) -> bool {
    host.eq_ignore_ascii_case(domain)
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

/// Safe default used by serde when older settings files have no policy field.
fn default_https_only() -> bool {
    true
}
