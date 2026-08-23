//! Conservative redaction for diagnostics crossing UI or portable-file boundaries.

/// Removes credential-, path-, and content-shaped values before a diagnostic crosses a trust boundary.
pub(crate) fn redact_diagnostic(value: &str) -> String {
    let mut redacted = value.to_owned();
    for marker in ["api_key=", "apikey=", "token=", "access_token="] {
        redacted = redact_after_marker(&redacted, marker);
    }
    redacted = redact_bearer_tokens(&redacted);
    for marker in [
        "request_body=",
        "request body=",
        "response_body=",
        "response body=",
        "tool_arguments=",
        "tool_result=",
        "database_content=",
        "attachment_content=",
        "path=",
    ] {
        redacted = redact_sensitive_tail(&redacted, marker);
    }
    for marker in ["file://", "/Users/", "/home/", "/private/", "/tmp/"] {
        redacted = redact_path_tail(&redacted, marker);
    }
    redact_windows_path_tail(&redacted)
}

/// Conservatively removes a path- or content-shaped field and everything after it.
fn redact_sensitive_tail(value: &str, marker: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let Some(start) = lower.find(marker) else {
        return value.to_owned();
    };
    format!("{}[redacted]", &value[..start + marker.len()])
}

/// Removes one absolute native path including its platform-specific prefix.
fn redact_path_tail(value: &str, marker: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let Some(start) = lower.find(&marker.to_ascii_lowercase()) else {
        return value.to_owned();
    };
    format!("{}[redacted]", &value[..start])
}

/// Removes one absolute Windows path regardless of its drive letter.
fn redact_windows_path_tail(value: &str) -> String {
    let Some(start) = value.as_bytes().windows(3).position(|characters| {
        characters[0].is_ascii_alphabetic()
            && characters[1] == b':'
            && matches!(characters[2], b'\\' | b'/')
    }) else {
        return value.to_owned();
    };
    format!("{}[redacted]", &value[..start])
}

/// Redacts the value following one case-insensitive credential marker.
fn redact_after_marker(value: &str, marker: &str) -> String {
    let mut result = value.to_owned();
    let mut search_from = 0;
    loop {
        let lower = result[search_from..].to_ascii_lowercase();
        let Some(relative_start) = lower.find(marker) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = result[value_start..]
            .find(['&', ' ', '\n', '\r'])
            .map(|offset| value_start + offset)
            .unwrap_or(result.len());
        result.replace_range(value_start..value_end, "[redacted]");
        search_from = value_start + "[redacted]".len();
    }
    result
}

/// Redacts bearer credential values without exposing them to the WebView.
fn redact_bearer_tokens(value: &str) -> String {
    let marker = "bearer ";
    let mut result = value.to_owned();
    let mut search_from = 0;
    loop {
        let lower = result[search_from..].to_ascii_lowercase();
        let Some(relative_start) = lower.find(marker) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = result[value_start..]
            .find([' ', '\n', '\r', ','])
            .map(|offset| value_start + offset)
            .unwrap_or(result.len());
        result.replace_range(value_start..value_end, "[redacted]");
        search_from = value_start + "[redacted]".len();
    }
    result
}
