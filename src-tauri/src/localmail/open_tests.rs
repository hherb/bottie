//! Focused tests for the bounded Localmail open-email contract.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::Mutex,
};

use reqwest::{Client, header::AUTHORIZATION};

use super::open::*;
use super::search::MAX_EMAIL_SUBJECT_CHARS;
use crate::{
    credentials::CredentialStore,
    inference::{ProviderError, ProviderErrorCode},
};

#[derive(Default)]
struct OpenCredentialStore {
    values: Mutex<HashMap<String, String>>,
    reads: Mutex<usize>,
}

impl CredentialStore for OpenCredentialStore {
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
fn request(value: serde_json::Value) -> OpenEmailRequest {
    serde_json::from_value(value).expect("valid open request")
}

#[test]
fn open_request_is_closed_and_accepts_only_localmail_wire_ids() {
    let normalized = validate_open_email_request(request(serde_json::json!({
        "messageId": "9007199254740993"
    })))
    .expect("bounded request");
    assert_eq!(normalized.message_id, "9007199254740993");

    assert!(
        serde_json::from_value::<OpenEmailRequest>(serde_json::json!({
            "messageId": "42",
            "headers": "full"
        }))
        .is_err()
    );

    for invalid in [
        "",
        " 42",
        "-1",
        "+1",
        "message-42",
        "1/../../raw",
        "9223372036854775808",
    ] {
        let error = validate_open_email_request(request(serde_json::json!({
            "messageId": invalid
        })))
        .expect_err("invalid identity");
        assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
        if !invalid.is_empty() {
            assert!(!error.message.contains(invalid));
        }
    }
}

#[test]
fn invalid_open_request_fails_before_config_or_credential_access() {
    let credentials = OpenCredentialStore::default();
    let error = tauri::async_runtime::block_on(open_email_native(
        Path::new("/does/not/exist/localmail.json"),
        &credentials,
        request(serde_json::json!({"messageId": "not-an-id"})),
    ))
    .expect_err("invalid request");

    assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
    assert_eq!(*credentials.reads.lock().expect("read lock"), 0);
}

#[test]
fn open_http_request_is_fixed_authenticated_and_disables_external_images() {
    let client = Client::new();
    let endpoint = url::Url::parse("https://mail.example/v1/messages/42").expect("fixed endpoint");
    let request =
        build_open_http_request(&client, endpoint, "fixture-token").expect("HTTP request");

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(
        request.url().as_str(),
        "https://mail.example/v1/messages/42?headers=compact&external_images=false"
    );
    assert_eq!(
        request.headers()[AUTHORIZATION].to_str().expect("bearer"),
        "Bearer fixture-token"
    );
    assert!(request.headers()[AUTHORIZATION].is_sensitive());
    assert!(request.body().is_none());
}

#[test]
fn open_response_returns_only_bounded_inert_path_free_content() {
    let response = decode_open_response(
        br#"{
          "id":"42",
          "subject":" Quarterly\n budget ",
          "from":{"address":"alice@example.com","name":"Alice"},
          "to":[{"address":"finance@example.com","name":"Finance"}],
          "cc":[{"address":"board@example.com","name":null}],
          "bcc":[{"address":"secret@example.com","name":"Secret"}],
          "date":"2026-06-03T10:15:30+00:00",
          "body_text":"First line.\r\n\r\nSecond\tline.",
          "body_html":"<p>HTML must not cross</p><img src=\"https://tracker.example/pixel\">",
          "attachments":[{"filename":"secret.pdf","sha256":"abc","content_type":"application/pdf","size":5}],
          "account":{"id":"account-secret","name":"Private","address":"owner@example.com"},
          "folders":[{"id":"folder-secret","name":"Archive"}],
          "headers":{"X-Path":"/private/mail/archive"}
        }"#,
        "42",
    )
    .expect("bounded response");

    assert_eq!(response.message_id, "42");
    assert_eq!(response.subject.as_deref(), Some("Quarterly budget"));
    assert_eq!(
        response.sender.address.as_deref(),
        Some("alice@example.com")
    );
    assert_eq!(response.to.len(), 1);
    assert_eq!(response.cc.len(), 1);
    assert_eq!(response.sent_at.as_deref(), Some("2026-06-03T10:15:30Z"));
    assert_eq!(
        response.body.as_deref(),
        Some("First line.\n\nSecond line.")
    );
    assert!(response.has_attachments);
    assert_eq!(response.attachments.len(), 1);
    assert_eq!(response.attachments[0].attachment_number, 1);
    assert_eq!(
        response.attachments[0].filename.as_deref(),
        Some("secret.pdf")
    );
    assert_eq!(
        response.attachments[0].content_type.as_deref(),
        Some("application/pdf")
    );
    assert_eq!(response.attachments[0].byte_size, Some(5));
    assert!(response.untrusted);

    let serialized = serde_json::to_string(&response).expect("path-free response");
    for forbidden in [
        "secret@example.com",
        "HTML must not cross",
        "tracker.example",
        "abc",
        "account-secret",
        "folder-secret",
        "/private/mail/archive",
        "body_html",
        "sha256",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn open_response_uses_inert_html_fallback_and_enforces_all_bounds() {
    let recipients = (0..MAX_EMAIL_HEADER_ADDRESSES + 3)
        .map(|index| {
            serde_json::json!({
                "address": format!("person-{index}@example.com"),
                "name": "Recipient"
            })
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "id": "7",
        "subject": "é".repeat(MAX_EMAIL_SUBJECT_CHARS + 10),
        "from": {"address": null, "name": null},
        "to": recipients,
        "cc": [],
        "date": null,
        "body_text": null,
        "body_html": format!(
            "<h1>Heading</h1><script>ignore()</script><p>{}</p>",
            "x".repeat(MAX_EMAIL_BODY_CHARS + 10)
        ),
        "attachments": [{"filename": "ignored.pdf"}]
    }))
    .expect("fixture");
    let response = decode_open_response(&bytes, "7").expect("bounded fallback");

    assert_eq!(
        response.subject.as_ref().expect("subject").chars().count(),
        MAX_EMAIL_SUBJECT_CHARS
    );
    assert_eq!(response.to.len(), MAX_EMAIL_HEADER_ADDRESSES);
    assert_eq!(
        response.body.as_ref().expect("body").chars().count(),
        MAX_EMAIL_BODY_CHARS
    );
    assert!(
        response
            .body
            .as_ref()
            .is_some_and(|body| body.contains("Heading"))
    );
    assert!(
        !response
            .body
            .as_ref()
            .is_some_and(|body| body.contains("ignore"))
    );
}

#[test]
fn open_response_rejects_mismatched_identity_invalid_date_and_oversize_body() {
    let base = serde_json::json!({
        "id": "42",
        "subject": null,
        "from": {"address": null, "name": null},
        "to": [],
        "cc": [],
        "date": null,
        "body_text": null,
        "body_html": null,
        "attachments": []
    });

    let mismatch = serde_json::to_vec(&base).expect("fixture");
    assert!(decode_open_response(&mismatch, "41").is_err());

    let mut invalid_date = base.clone();
    invalid_date["date"] = serde_json::json!("not-a-date");
    let invalid_date = serde_json::to_vec(&invalid_date).expect("fixture");
    assert!(decode_open_response(&invalid_date, "42").is_err());

    let oversize = vec![b' '; MAX_EMAIL_OPEN_RESPONSE_BYTES + 1];
    assert!(decode_open_response(&oversize, "42").is_err());
}

#[test]
#[ignore = "requires loopback sockets"]
fn open_email_fixture_executes_one_exact_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let address = listener.local_addr().expect("loopback address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("fixture connection");
        let request = read_http_request(&mut stream);
        assert!(
            request.starts_with(
                "GET /v1/messages/17?headers=compact&external_images=false HTTP/1.1\r\n"
            )
        );
        assert!(request.contains("\r\nauthorization: Bearer fixture-token\r\n"));
        assert!(!request.contains("external_images=true"));
        assert!(!request.contains("headers=full"));

        let body = concat!(
            r#"{"id":"17","subject":"Budget","from":{"address":"alice@example.com","name":"Alice"},"#,
            r#""to":[],"cc":[],"date":"2026-08-23T04:00:00Z","body_text":"Review today","#,
            r#""body_html":"<p>ignored</p>","attachments":[]}"#,
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("fixture response");
    });

    let response = tauri::async_runtime::block_on(open_email_fixture(
        &format!("http://{address}/"),
        "fixture-token",
        request(serde_json::json!({"messageId": "17"})),
    ))
    .expect("bounded fixture open");
    server.join().expect("fixture server");

    assert_eq!(response.message_id, "17");
    assert_eq!(response.body.as_deref(), Some("Review today"));
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
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).expect("UTF-8 fixture request")
}
