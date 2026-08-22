use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use super::brave::{decode_fixture_response, map_fixture_status};
use super::{
    BraveSearchProvider, MAX_WEB_SEARCH_QUERY_CHARS, MAX_WEB_SEARCH_QUERY_WORDS,
    MAX_WEB_SEARCH_RESULTS, WebSearchErrorCode, WebSearchProvider, WebSearchRequest,
    connection_test_request,
};

#[test]
fn request_contract_normalizes_and_bounds_queries() {
    let request = WebSearchRequest::new("  rust   native search  ", 5).unwrap();
    assert_eq!(request.query(), "rust native search");
    assert_eq!(request.result_limit(), 5);

    assert!(WebSearchRequest::new("", 5).is_err());
    assert!(WebSearchRequest::new("   ", 5).is_err());
    assert!(WebSearchRequest::new("word ".repeat(MAX_WEB_SEARCH_QUERY_WORDS + 1), 5).is_err());
    assert!(WebSearchRequest::new("x".repeat(MAX_WEB_SEARCH_QUERY_CHARS + 1), 5).is_err());
    assert!(WebSearchRequest::new("valid", 0).is_err());
    assert!(WebSearchRequest::new("valid", MAX_WEB_SEARCH_RESULTS + 1).is_err());
}

#[test]
fn brave_requires_a_non_empty_native_credential() {
    let error = match BraveSearchProvider::new("   ") {
        Ok(_) => panic!("an empty credential must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, WebSearchErrorCode::CredentialRequired);
    assert!(!error.message.contains("   "));
}

#[test]
fn brave_builds_a_bounded_web_only_request_with_header_authentication() {
    let provider =
        BraveSearchProvider::for_loopback_fixture("http://127.0.0.1:9/", "fixture-secret").unwrap();
    let request = provider
        .fixture_request(&WebSearchRequest::new("bottie rust", 3).unwrap())
        .unwrap();

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(request.url().path(), "/res/v1/web/search");
    let query = request.url().query().unwrap();
    assert!(query.contains("q=bottie+rust"));
    assert!(query.contains("count=3"));
    assert!(query.contains("result_filter=web"));
    assert!(query.contains("safesearch=strict"));
    assert!(query.contains("text_decorations=false"));
    assert!(!request.url().as_str().contains("fixture-secret"));
    assert_eq!(
        request.headers().get("x-subscription-token").unwrap(),
        "fixture-secret"
    );
    assert!(
        request
            .headers()
            .get("x-subscription-token")
            .unwrap()
            .is_sensitive()
    );
}

#[test]
fn brave_production_adapter_owns_the_fixed_https_endpoint() {
    let provider = BraveSearchProvider::new("fixture-secret").unwrap();
    let request = provider
        .fixture_request(&WebSearchRequest::new("bottie", 1).unwrap())
        .unwrap();

    assert_eq!(request.url().scheme(), "https");
    assert_eq!(request.url().host_str(), Some("api.search.brave.com"));
    assert_eq!(request.url().path(), "/res/v1/web/search");
}

#[test]
fn connection_test_uses_one_fixed_bounded_probe() {
    let request = connection_test_request();

    assert_eq!(request.query(), "Bottie connection test");
    assert_eq!(request.result_limit(), 1);
}

#[test]
fn brave_decodes_only_bounded_safe_web_results() {
    let response_body = serde_json::json!({
        "web": {
            "results": [
                {
                    "title": "  Bottie   project  ",
                    "url": "https://example.com/bottie#provider-fragment",
                    "description": "  A native   search boundary. ",
                    "page_age": "2026-08-22T00:00:00Z"
                },
                {
                    "title": "Unsafe",
                    "url": "javascript:alert(1)",
                    "description": "must not cross the boundary"
                },
                {
                    "title": "Embedded credential",
                    "url": "https://user:secret@example.com/private",
                    "description": "must not cross the boundary"
                }
            ]
        }
    })
    .to_string();
    let response = decode_fixture_response(response_body.as_bytes(), 3).unwrap();

    assert_eq!(response.provider_id(), "brave");
    assert_eq!(response.results().len(), 1);
    assert_eq!(response.results()[0].title(), "Bottie project");
    assert_eq!(response.results()[0].url(), "https://example.com/bottie");
    assert_eq!(response.results()[0].snippet(), "A native search boundary.");
    assert_eq!(
        response.results()[0].published_at(),
        Some("2026-08-22T00:00:00Z")
    );
}

#[test]
fn brave_status_mapping_is_stable_and_redacted() {
    let rate_limit = map_fixture_status(429);
    assert_eq!(rate_limit.code, WebSearchErrorCode::RateLimited);
    assert!(rate_limit.retryable);
    assert_eq!(
        rate_limit.message,
        "The web-search provider is temporarily rate limited."
    );

    let credential = map_fixture_status(401);
    assert_eq!(credential.code, WebSearchErrorCode::CredentialRejected);
    assert!(!credential.retryable);

    let rejected = map_fixture_status(422);
    assert_eq!(rejected.code, WebSearchErrorCode::InvalidRequest);
    assert!(!rejected.retryable);
}

#[test]
#[ignore = "requires loopback fixture access"]
fn brave_normalizes_fixture_results_and_keeps_the_key_in_a_header() {
    let response_body = serde_json::json!({
        "web": {
            "results": [
                {
                    "title": "  Bottie   project  ",
                    "url": "https://example.com/bottie",
                    "description": "  A native   search boundary. ",
                    "page_age": "2026-08-22T00:00:00Z"
                },
                {
                    "title": "Unsafe",
                    "url": "javascript:alert(1)",
                    "description": "must not cross the boundary"
                }
            ]
        }
    })
    .to_string();
    let (endpoint, request, server) = fixture_server(200, &response_body);
    let provider = BraveSearchProvider::for_loopback_fixture(&endpoint, "fixture-secret").unwrap();
    let response = tauri::async_runtime::block_on(
        provider.search(WebSearchRequest::new("bottie rust", 3).unwrap()),
    )
    .unwrap();
    server.join().unwrap();

    assert_eq!(provider.provider_id(), "brave");
    assert_eq!(response.provider_id(), "brave");
    assert_eq!(response.results().len(), 1);
    assert_eq!(response.results()[0].title(), "Bottie project");
    assert_eq!(response.results()[0].url(), "https://example.com/bottie");
    assert_eq!(response.results()[0].snippet(), "A native search boundary.");
    assert_eq!(
        response.results()[0].published_at(),
        Some("2026-08-22T00:00:00Z")
    );

    let request = request.join().unwrap();
    assert!(request.starts_with("GET /res/v1/web/search?"));
    assert!(request.contains("q=bottie+rust"));
    assert!(request.contains("count=3"));
    assert!(request.contains("result_filter=web"));
    assert!(request.contains("safesearch=strict"));
    assert!(request.contains("text_decorations=false"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("x-subscription-token: fixture-secret")
    );
    assert!(!request.lines().next().unwrap().contains("fixture-secret"));
}

#[test]
#[ignore = "requires loopback fixture access"]
fn brave_maps_provider_failures_without_reflecting_body_or_query() {
    let (endpoint, _, server) = fixture_server(429, "secret query provider details");
    let provider = BraveSearchProvider::for_loopback_fixture(&endpoint, "fixture-secret").unwrap();
    let error = tauri::async_runtime::block_on(
        provider.search(WebSearchRequest::new("private query", 5).unwrap()),
    )
    .unwrap_err();
    server.join().unwrap();

    assert_eq!(error.code, WebSearchErrorCode::RateLimited);
    assert!(error.retryable);
    assert!(!error.message.contains("private query"));
    assert!(!error.message.contains("provider details"));
    assert!(!error.message.contains("fixture-secret"));
}

/// Starts one isolated HTTP fixture and returns its captured request.
fn fixture_server(
    status: u16,
    body: &str,
) -> (String, thread::JoinHandle<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_owned();
    let (request_sender, request_receiver) = std::sync::mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        request_sender.send(request).unwrap();
        let reason = if status == 200 { "OK" } else { "Error" };
        write!(
            stream,
            concat!(
                "HTTP/1.1 {} {}\r\n",
                "Content-Type: application/json\r\n",
                "Content-Length: {}\r\n",
                "Connection: close\r\n\r\n{}"
            ),
            status,
            reason,
            body.len(),
            body
        )
        .unwrap();
    });
    let request = thread::spawn(move || request_receiver.recv().unwrap());
    (format!("http://{address}/"), request, server)
}

/// Reads one header-only GET request from the fixture connection.
fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1_024];
    loop {
        let read = stream.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}
