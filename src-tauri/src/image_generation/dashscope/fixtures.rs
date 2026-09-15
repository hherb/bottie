//! Loopback-only construction for real DashScope adapter request fixtures.

use super::{DashScopeQwenImageProvider, ProviderError, Url};

impl DashScopeQwenImageProvider {
    /// Builds a loopback-only HTTP adapter that cannot compile into production.
    pub(in crate::image_generation) fn for_http_fixture(
        base_url: &str,
        api_key: &str,
    ) -> Result<Self, ProviderError> {
        let base_url = Url::parse(base_url)
            .map_err(|_| ProviderError::invalid_request("The fixture API root is malformed."))?;
        let is_loopback = base_url
            .host_str()
            .is_some_and(|host| matches!(host, "127.0.0.1" | "::1" | "localhost"));
        if base_url.scheme() != "http"
            || !is_loopback
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || base_url.path() != "/api/v1/"
        {
            return Err(ProviderError::invalid_request(
                "The fixture API root must be an exact loopback HTTP /api/v1/ root.",
            ));
        }
        Self::build(base_url, api_key.to_owned())
    }
}
