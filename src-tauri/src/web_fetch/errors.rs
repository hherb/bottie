//! Stable redacted failure categories for native public-web retrieval.

/// Stable native web-fetch failure categories with no URL or response reflection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WebFetchErrorCode {
    /// The requested URL failed the closed public-network contract.
    InvalidRequest,
    /// DNS resolved to a non-public destination or no usable address.
    BlockedAddress,
    /// The redirect chain exceeded the native ceiling or contained an invalid target.
    RedirectRejected,
    /// The complete native operation exceeded its time limit.
    Timeout,
    /// The response media type or charset is outside the initial page-source contract.
    UnsupportedContentType,
    /// The response body exceeded the native byte ceiling.
    ResponseTooLarge,
    /// The destination was unreachable or returned an unsuccessful status.
    Unavailable,
    /// The response body was not valid UTF-8 page source.
    MalformedResponse,
    /// A native client could not be initialized.
    Internal,
}

/// Redacted native web-fetch failure that never contains the requested URL or response body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebFetchError {
    /// Stable machine-readable category used by common dispatcher mapping.
    pub(crate) code: WebFetchErrorCode,
    /// Fixed explanation without network, URL, or response details.
    pub(crate) message: &'static str,
}

impl WebFetchError {
    /// Builds a fixed unavailable-destination fixture or runtime failure.
    pub(crate) fn unavailable() -> Self {
        Self::new(
            WebFetchErrorCode::Unavailable,
            "The public web destination is unavailable.",
        )
    }

    /// Builds one fixed redacted failure.
    fn new(code: WebFetchErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }
}

/// Builds one fixed invalid public-URL error.
pub(super) fn invalid_request() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::InvalidRequest,
        "Use a bounded public HTTP or HTTPS URL without credentials.",
    )
}

/// Builds one fixed user-policy rejection without reflecting the destination.
pub(crate) fn blocked_by_user_policy() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::InvalidRequest,
        "The web destination is blocked by the saved Web network policy.",
    )
}

/// Builds one fixed blocked destination-address error.
pub(super) fn blocked_address() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::BlockedAddress,
        "The web destination is outside Bottie's public-network policy.",
    )
}

/// Builds one fixed rejected redirect-chain error.
pub(super) fn redirect_error() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::RedirectRejected,
        "The web redirect chain was rejected by native policy.",
    )
}

/// Builds one fixed total-operation timeout error.
pub(super) fn timeout() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::Timeout,
        "The public web fetch timed out.",
    )
}

/// Builds one fixed unsupported page-source media error.
pub(super) fn unsupported_content_type() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::UnsupportedContentType,
        "The web response is not supported UTF-8 page source.",
    )
}

/// Builds one fixed response-byte ceiling error.
pub(super) fn response_too_large() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::ResponseTooLarge,
        "The web response exceeded Bottie's native size limit.",
    )
}

/// Builds one fixed invalid UTF-8 response error.
pub(super) fn malformed_response() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::MalformedResponse,
        "The web response was not valid UTF-8 page source.",
    )
}

/// Builds one fixed native client initialization error.
pub(super) fn internal_error() -> WebFetchError {
    WebFetchError::new(
        WebFetchErrorCode::Internal,
        "Bottie could not initialize the native web fetcher.",
    )
}

/// Maps request failures without forwarding host, URL, proxy, or TLS diagnostics.
pub(super) fn map_request_error(error: reqwest::Error) -> WebFetchError {
    if error.is_timeout() {
        timeout()
    } else {
        WebFetchError::unavailable()
    }
}
