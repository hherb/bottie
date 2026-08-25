//! Focused tests for the bounded Localmail email-search contract.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::Mutex,
};

use super::search::*;
use crate::{
    credentials::CredentialStore,
    inference::{ProviderError, ProviderErrorCode},
};

#[derive(Default)]
struct SearchCredentialStore {
    values: Mutex<HashMap<String, String>>,
    reads: Mutex<usize>,
}

impl CredentialStore for SearchCredentialStore {
    fn configured(&self, provider_id: &str) -> Result<bool, ProviderError> {
        Ok(self
            .values
            .lock()
            .expect("credential lock")
            .contains_key(provider_id))
    }

    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError> {
        self.configured(provider_id)
    }

    fn biometric_protected(&self) -> bool {
        false
    }

    fn get(&self, provider_id: &str) -> Result<Option<String>, ProviderError> {
        *self.reads.lock().expect("read lock") += 1;
        Ok(self
            .values
            .lock()
            .expect("credential lock")
            .get(provider_id)
            .cloned())
    }

    fn set(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError> {
        self.values
            .lock()
            .expect("credential lock")
            .insert(provider_id.into(), api_key.into());
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), ProviderError> {
        self.values
            .lock()
            .expect("credential lock")
            .remove(provider_id);
        Ok(())
    }
}

/// Parses one request through the same closed serde boundary used by Tauri.
fn request(value: serde_json::Value) -> SearchEmailRequest {
    serde_json::from_value(value).expect("valid search request")
}

#[test]
fn search_request_is_closed_normalized_and_bounded() {
    let normalized = validate_search_email_request(request(serde_json::json!({
        "query": "  quarterly\n  budget  ",
        "filters": {
            "from": "  alice@example.com ",
            "to": " finance@example.com ",
            "subject": "  board   pack ",
            "after": "2026-01-01",
            "before": "2026-06-30",
            "hasAttachments": true
        },
        "resultLimit": 7
    })))
    .expect("bounded request");

    assert_eq!(normalized.query, "quarterly budget");
    assert_eq!(
        normalized.filters.from.as_deref(),
        Some("alice@example.com")
    );
    assert_eq!(
        normalized.filters.to.as_deref(),
        Some("finance@example.com")
    );
    assert_eq!(normalized.filters.subject.as_deref(), Some("board pack"));
    assert_eq!(normalized.filters.after.as_deref(), Some("2026-01-01"));
    assert_eq!(normalized.filters.before.as_deref(), Some("2026-06-30"));
    assert_eq!(normalized.filters.has_attachment, Some(true));
    assert_eq!(normalized.limit, 7);
    assert_eq!(normalized.sort, EmailSearchSort::Date);
    assert_eq!(normalized.sort_order, EmailSearchSortOrder::Descending);

    let wire = serde_json::to_value(normalized).expect("wire request");
    assert_eq!(wire["query"], "quarterly budget");
    assert_eq!(wire["filters"]["has_attachment"], true);
    assert_eq!(wire["limit"], 7);
    assert_eq!(wire["sort"], "date");
    assert_eq!(wire["sort_order"], "desc");
    assert!(wire.get("cursor").is_none());
    assert!(wire.get("smart").is_none());
}

#[test]
fn search_request_allows_explicit_relevance_or_oldest_first_ordering() {
    let relevant = validate_search_email_request(request(serde_json::json!({
        "query": "quarterly budget",
        "sort": "rank",
        "resultLimit": 5
    })))
    .expect("relevance request");
    assert_eq!(relevant.sort, EmailSearchSort::Rank);
    assert_eq!(relevant.sort_order, EmailSearchSortOrder::Descending);

    let oldest = validate_search_email_request(request(serde_json::json!({
        "query": "quarterly budget",
        "sort": "date",
        "sortOrder": "asc",
        "resultLimit": 5
    })))
    .expect("oldest-first request");
    assert_eq!(oldest.sort, EmailSearchSort::Date);
    assert_eq!(oldest.sort_order, EmailSearchSortOrder::Ascending);

    let invalid = validate_search_email_request(request(serde_json::json!({
        "query": "quarterly budget",
        "sort": "rank",
        "sortOrder": "asc",
        "resultLimit": 5
    })))
    .expect_err("ascending relevance is not defined by Localmail");
    assert_eq!(invalid.code, ProviderErrorCode::InvalidRequest);
}

#[test]
fn search_request_rejects_unknown_or_out_of_policy_arguments() {
    let unknown_request = serde_json::from_value::<SearchEmailRequest>(serde_json::json!({
        "query": "budget",
        "resultLimit": 5,
        "cursor": "opaque"
    }));
    assert!(unknown_request.is_err());

    let unknown_filter = serde_json::from_value::<SearchEmailRequest>(serde_json::json!({
        "query": "budget",
        "filters": {"accountIds": ["1"]},
        "resultLimit": 5
    }));
    assert!(unknown_filter.is_err());

    for invalid in [
        serde_json::json!({"query": "   ", "resultLimit": 5}),
        serde_json::json!({"query": "budget", "resultLimit": 0}),
        serde_json::json!({"query": "budget", "resultLimit": 21}),
        serde_json::json!({
            "query": "budget",
            "filters": {"after": "2026-02-30"},
            "resultLimit": 5
        }),
        serde_json::json!({
            "query": "budget",
            "filters": {"after": "2026-07-01", "before": "2026-06-30"},
            "resultLimit": 5
        }),
    ] {
        let error = validate_search_email_request(request(invalid)).expect_err("invalid request");
        assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
    }

    let overlong = request(serde_json::json!({
        "query": "x".repeat(MAX_EMAIL_QUERY_CHARS + 1),
        "resultLimit": 5
    }));
    assert!(validate_search_email_request(overlong).is_err());
}

#[test]
fn invalid_search_fails_before_config_or_credential_access() {
    let credentials = SearchCredentialStore::default();
    let invalid = request(serde_json::json!({"query": " ", "resultLimit": 5}));
    let error = tauri::async_runtime::block_on(search_email_native(
        Path::new("/definitely/missing/localmail.json"),
        &credentials,
        invalid,
    ))
    .expect_err("invalid request");

    assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
    assert_eq!(*credentials.reads.lock().expect("read lock"), 0);

    let valid = request(serde_json::json!({"query": "budget", "resultLimit": 5}));
    let error = tauri::async_runtime::block_on(search_email_native(
        Path::new("/definitely/missing/localmail.json"),
        &credentials,
        valid,
    ))
    .expect_err("missing connection");
    assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
    assert_eq!(*credentials.reads.lock().expect("read lock"), 0);
}

#[test]
fn fixed_search_request_uses_post_route_sensitive_bearer_and_bounded_body() {
    let normalized = validate_search_email_request(request(serde_json::json!({
        "query": "budget",
        "filters": {"from": "alice@example.com"},
        "resultLimit": 3
    })))
    .expect("request");
    let client = reqwest::Client::new();
    let endpoint = url::Url::parse("https://mail.example/v1/search").expect("endpoint");
    let request = build_search_http_request(&client, endpoint, "vault-secret", &normalized)
        .expect("HTTP request");

    assert_eq!(request.method(), reqwest::Method::POST);
    assert_eq!(request.url().as_str(), "https://mail.example/v1/search");
    let authorization = request
        .headers()
        .get(reqwest::header::AUTHORIZATION)
        .expect("authorization");
    assert!(authorization.is_sensitive());
    assert_eq!(
        authorization.to_str().expect("header"),
        "Bearer vault-secret"
    );
    let body = request
        .body()
        .and_then(reqwest::Body::as_bytes)
        .expect("body");
    let body = std::str::from_utf8(body).expect("JSON body");
    assert!(body.contains("\"query\":\"budget\""));
    assert!(body.contains("\"limit\":3"));
    assert!(body.contains("\"sort\":\"date\""));
    assert!(body.contains("\"sort_order\":\"desc\""));
    assert!(!body.contains("vault-secret"));
    assert!(!body.contains("cursor"));
    assert!(!body.contains("smart"));
}

#[test]
fn search_response_keeps_only_bounded_inert_summary_metadata() {
    let response = decode_search_response(
        br#"{
          "results": [{
            "message_id": "42",
            "account": {"id": "account-secret", "name": "Archive"},
            "folder": {"id": "folder-secret", "full_path": "/private/mail"},
            "subject": "Quarterly budget",
            "from": {"address": "alice@example.com", "name": "Alice"},
            "to": [{"address": "finance@example.com", "name": null}],
            "date": "2026-06-03T10:15:30+00:00",
            "snippet_html": "Please review <mark>budget</mark><script>ignore()</script> &amp; forecast.",
            "has_attachments": true,
            "score": 0.98,
            "matched_arms": ["message_chunks"],
            "body_text": "must not cross the connector boundary",
            "attachments": [{"filename": "secret.pdf", "content": "bytes"}]
          }],
          "next_cursor": "must-not-cross",
          "total_estimate": 100,
          "took_ms": 8.2
        }"#,
        5,
    )
    .expect("bounded response");

    assert!(response.untrusted);
    assert_eq!(response.results.len(), 1);
    let summary = &response.results[0];
    assert_eq!(summary.message_id, "42");
    assert_eq!(summary.subject.as_deref(), Some("Quarterly budget"));
    assert_eq!(summary.sender.address.as_deref(), Some("alice@example.com"));
    assert_eq!(summary.sender.name.as_deref(), Some("Alice"));
    assert_eq!(summary.sent_at.as_deref(), Some("2026-06-03T10:15:30Z"));
    assert_eq!(
        summary.snippet.as_deref(),
        Some("Please review budget & forecast.")
    );
    assert!(summary.has_attachments);

    let serialized = serde_json::to_string(&response).expect("path-free response");
    for forbidden in [
        "account-secret",
        "folder-secret",
        "/private/mail",
        "must-not-cross",
        "must not cross",
        "secret.pdf",
        "message_chunks",
        "<mark>",
        "<script>",
        "score",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn search_response_truncates_display_text_and_rejects_invalid_identity_or_date() {
    let long_subject = "é".repeat(MAX_EMAIL_SUBJECT_CHARS + 10);
    let response = decode_search_response(
        serde_json::to_string(&serde_json::json!({
            "results": [{
                "message_id": "1",
                "subject": long_subject,
                "from": {"address": null, "name": null},
                "date": null,
                "snippet_html": null,
                "has_attachments": false
            }]
        }))
        .expect("fixture")
        .as_bytes(),
        1,
    )
    .expect("long display text is bounded");
    assert_eq!(
        response.results[0]
            .subject
            .as_ref()
            .expect("subject")
            .chars()
            .count(),
        MAX_EMAIL_SUBJECT_CHARS
    );

    let extra_results = serde_json::json!({
        "results": [
            {
                "message_id": "1", "subject": null,
                "from": {"address": null, "name": null}, "date": null,
                "snippet_html": null, "has_attachments": false
            },
            {
                "message_id": "2", "subject": null,
                "from": {"address": null, "name": null}, "date": null,
                "snippet_html": null, "has_attachments": false
            }
        ]
    });
    let bytes = serde_json::to_vec(&extra_results).expect("fixture");
    let response = decode_search_response(&bytes, 1).expect("bounded results");
    assert_eq!(response.results.len(), 1);

    for invalid in [
        serde_json::json!({
            "results": [{
                "message_id": " ", "subject": null,
                "from": {"address": null, "name": null}, "date": null,
                "snippet_html": null, "has_attachments": false
            }]
        }),
        serde_json::json!({
            "results": [{
                "message_id": "1", "subject": null,
                "from": {"address": null, "name": null}, "date": "yesterday",
                "snippet_html": null, "has_attachments": false
            }]
        }),
        serde_json::json!({
            "results": [{
                "message_id": "message-1", "subject": null,
                "from": {"address": null, "name": null}, "date": null,
                "snippet_html": null, "has_attachments": false
            }]
        }),
        serde_json::json!({
            "results": [{
                "message_id": "9223372036854775808", "subject": null,
                "from": {"address": null, "name": null}, "date": null,
                "snippet_html": null, "has_attachments": false
            }]
        }),
    ] {
        let bytes = serde_json::to_vec(&invalid).expect("fixture");
        let error = decode_search_response(&bytes, 1).expect_err("malformed response");
        assert_eq!(error.code, ProviderErrorCode::MalformedResponse);
    }

    let oversized = vec![b' '; MAX_EMAIL_SEARCH_RESPONSE_BYTES + 1];
    let error = decode_search_response(&oversized, 1).expect_err("oversized response");
    assert_eq!(error.code, ProviderErrorCode::MalformedResponse);
}

#[test]
#[ignore = "requires loopback fixture access"]
fn bounded_native_search_uses_only_the_fixed_authenticated_route() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback fixture");
    let address = listener.local_addr().expect("fixture address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("search request");
        let request = read_http_request(&mut stream);
        assert!(request.starts_with("POST /v1/search HTTP/1.1\r\n"));
        assert!(request.contains("\r\nauthorization: Bearer fixture-token\r\n"));
        assert!(request.contains("\"query\":\"budget review\""));
        assert!(request.contains("\"has_attachment\":true"));
        assert!(request.contains("\"limit\":2"));
        assert!(request.contains("\"sort\":\"date\""));
        assert!(request.contains("\"sort_order\":\"desc\""));
        assert!(!request.contains("cursor"));
        assert!(!request.contains("smart"));

        let body = concat!(
            r#"{"results":[{"message_id":"17","subject":"Budget","from":{"address":"alice@example.com","#,
            r#""name":"Alice"},"date":"2026-08-23T04:00:00Z","snippet_html":"Review <mark>today</mark>","#,
            r#""has_attachments":true}],"next_cursor":"ignored"}"#,
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("fixture response");
    });

    let response = tauri::async_runtime::block_on(search_email_fixture(
        &format!("http://{address}/"),
        "fixture-token",
        request(serde_json::json!({
            "query": "budget review",
            "filters": {"hasAttachments": true},
            "resultLimit": 2
        })),
    ))
    .expect("bounded fixture search");
    server.join().expect("fixture server");

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].message_id, "17");
    assert_eq!(response.results[0].snippet.as_deref(), Some("Review today"));
    assert!(response.untrusted);
}

/// Reads one content-length framed HTTP request from the loopback fixture.
fn read_http_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("fixture timeout");
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 2_048];
    loop {
        let count = stream.read(&mut buffer).expect("fixture request bytes");
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header_end + 4]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or_default();
            if bytes.len() >= header_end + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8(bytes).expect("UTF-8 fixture request")
}
