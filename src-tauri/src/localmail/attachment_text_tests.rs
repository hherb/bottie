//! Focused tests for bounded Localmail extracted attachment text.

use std::{collections::HashMap, path::Path, sync::Mutex};

use reqwest::{Client, header::AUTHORIZATION};

use super::attachment_text::*;
use super::open::MAX_EMAIL_ATTACHMENTS;
use super::{
    open::{OpenEmailRequest, open_email_native},
    search::{SearchEmailFilters, SearchEmailRequest, search_email_native},
};
use crate::{
    credentials::{CredentialStore, SystemCredentialStore},
    inference::{ProviderError, ProviderErrorCode},
};

#[derive(Default)]
struct AttachmentCredentialStore {
    values: Mutex<HashMap<String, String>>,
    reads: Mutex<usize>,
}

impl CredentialStore for AttachmentCredentialStore {
    fn configured(&self, provider_id: &str) -> Result<bool, ProviderError> {
        Ok(self.values.lock().unwrap().contains_key(provider_id))
    }

    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError> {
        self.configured(provider_id)
    }

    fn biometric_protected(&self) -> bool {
        false
    }

    fn get(&self, provider_id: &str) -> Result<Option<String>, ProviderError> {
        *self.reads.lock().unwrap() += 1;
        Ok(self.values.lock().unwrap().get(provider_id).cloned())
    }

    fn set(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError> {
        self.values
            .lock()
            .unwrap()
            .insert(provider_id.into(), api_key.into());
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), ProviderError> {
        self.values.lock().unwrap().remove(provider_id);
        Ok(())
    }
}

fn request(value: serde_json::Value) -> ReadEmailAttachmentRequest {
    serde_json::from_value(value).expect("valid attachment-text request")
}

#[test]
fn attachment_text_request_is_closed_and_bounded() {
    let normalized = validate_read_email_attachment_request(request(serde_json::json!({
        "messageId": "42",
        "attachmentNumber": 2
    })))
    .expect("bounded request");
    assert_eq!(normalized.message_id, "42");
    assert_eq!(normalized.attachment_number, 2);

    assert!(
        serde_json::from_value::<ReadEmailAttachmentRequest>(serde_json::json!({
            "messageId": "42",
            "attachmentNumber": 1,
            "mode": "bytes"
        }))
        .is_err()
    );
    for invalid in [0, MAX_EMAIL_ATTACHMENTS + 1] {
        let error = validate_read_email_attachment_request(request(serde_json::json!({
            "messageId": "42",
            "attachmentNumber": invalid
        })))
        .expect_err("invalid attachment number");
        assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
    }
}

#[test]
fn invalid_attachment_request_fails_before_config_or_credential_access() {
    let credentials = AttachmentCredentialStore::default();
    let error = tauri::async_runtime::block_on(read_email_attachment_native(
        Path::new("/does/not/exist/localmail.json"),
        &credentials,
        request(serde_json::json!({"messageId": "../42", "attachmentNumber": 1})),
    ))
    .expect_err("invalid request");

    assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
    assert_eq!(*credentials.reads.lock().unwrap(), 0);
}

#[test]
fn resolves_only_the_selected_message_attachment_without_exposing_its_hash() {
    let resolved = resolve_attachment(
        br#"{
          "id":"42",
          "attachments":[
            {
              "filename":"notes.txt",
              "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "content_type":"text/plain",
              "size":12
            },
            {
              "filename":"invoice.pdf",
              "sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              "content_type":"application/pdf",
              "size":48213
            }
          ]
        }"#,
        "42",
        2,
    )
    .expect("selected attachment");

    assert_eq!(resolved.sha256, "b".repeat(64));
    assert_eq!(resolved.filename.as_deref(), Some("invoice.pdf"));
    assert_eq!(resolved.content_type.as_deref(), Some("application/pdf"));
    assert_eq!(resolved.byte_size, Some(48_213));

    for (message_id, attachment_number) in [("41", 2), ("42", 3)] {
        assert!(
            resolve_attachment(
                concat!(
                    r#"{"id":"42","attachments":[{"sha256":"#,
                    r#"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"#,
                    r#""}]}"#,
                )
                .as_bytes(),
                message_id,
                attachment_number,
            )
            .is_err()
        );
    }
}

#[test]
fn attachment_text_request_is_fixed_authenticated_and_json_only() {
    let client = Client::new();
    let endpoint = url::Url::parse(&format!(
        "https://mail.example/v1/attachments/{}/text",
        "b".repeat(64)
    ))
    .unwrap();
    let request = build_attachment_text_http_request(&client, endpoint, "fixture-token")
        .expect("HTTP request");

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(
        request.headers()[AUTHORIZATION].to_str().unwrap(),
        "Bearer fixture-token"
    );
    assert!(request.headers()[AUTHORIZATION].is_sensitive());
    assert!(request.body().is_none());
}

#[test]
fn attachment_text_response_is_bounded_inert_and_path_free() {
    let response = decode_attachment_text_response(
        br#"{"text":"Invoice  2026\r\n\r\nTotal:\t$48.13"}"#,
        "42",
        2,
        ResolvedAttachment {
            sha256: "b".repeat(64),
            filename: Some("invoice.pdf".into()),
            content_type: Some("application/pdf".into()),
            byte_size: Some(48_213),
        },
    )
    .expect("bounded text");

    assert_eq!(response.message_id, "42");
    assert_eq!(response.attachment_number, 2);
    assert_eq!(response.text, "Invoice 2026\n\nTotal: $48.13");
    assert!(!response.truncated);
    assert!(response.untrusted);
    let serialized = serde_json::to_string(&response).unwrap();
    assert!(!serialized.contains(&"b".repeat(64)));
    assert!(!serialized.contains("sha256"));

    let oversized = serde_json::to_vec(&serde_json::json!({
        "text": "x".repeat(MAX_EMAIL_ATTACHMENT_TEXT_CHARS + 1)
    }))
    .unwrap();
    let response = decode_attachment_text_response(
        &oversized,
        "42",
        2,
        ResolvedAttachment {
            sha256: "b".repeat(64),
            filename: None,
            content_type: None,
            byte_size: None,
        },
    )
    .expect("truncated text");
    assert_eq!(
        response.text.chars().count(),
        MAX_EMAIL_ATTACHMENT_TEXT_CHARS
    );
    assert!(response.truncated);
}

#[test]
#[ignore = "requires the saved Bottie Localmail config, vault credential, and a live archive"]
fn live_localmail_attachment_text_uses_the_saved_pinned_connector() {
    let config_path = std::env::var_os("BOTTIE_LIVE_LOCALMAIL_CONFIG_PATH")
        .map(std::path::PathBuf::from)
        .expect("set BOTTIE_LIVE_LOCALMAIL_CONFIG_PATH to Bottie's saved localmail.json");
    let query = std::env::var("BOTTIE_LIVE_LOCALMAIL_QUERY").unwrap_or_else(|_| "invoice".into());
    let credentials = SystemCredentialStore::default();
    let search = tauri::async_runtime::block_on(search_email_native(
        &config_path,
        &credentials,
        SearchEmailRequest {
            query,
            filters: SearchEmailFilters {
                has_attachments: Some(true),
                ..SearchEmailFilters::default()
            },
            sort: Default::default(),
            sort_order: Default::default(),
            result_limit: 20,
        },
    ))
    .expect("saved pinned Localmail search should succeed");

    let mut messages_examined = 0_usize;
    let mut attachments_examined = 0_usize;
    for summary in search.results {
        messages_examined += 1;
        let Ok(opened) = tauri::async_runtime::block_on(open_email_native(
            &config_path,
            &credentials,
            OpenEmailRequest {
                message_id: summary.message_id,
            },
        )) else {
            continue;
        };
        for attachment in opened.attachments {
            attachments_examined += 1;
            let request = ReadEmailAttachmentRequest {
                message_id: opened.message_id.clone(),
                attachment_number: attachment.attachment_number,
            };
            let Ok(response) = tauri::async_runtime::block_on(read_email_attachment_native(
                &config_path,
                &credentials,
                request,
            )) else {
                continue;
            };
            println!(
                "live_localmail_attachment_text_ok messages_examined={} attachments_examined={} mime_class={} \
                 size_class={} text_characters={} truncated={}",
                messages_examined,
                attachments_examined,
                mime_class(response.content_type.as_deref()),
                size_class(response.byte_size),
                response.text.chars().count(),
                response.truncated,
            );
            return;
        }
    }
    panic!(
        "no ready extracted attachment text found in bounded live search; messages_examined={messages_examined} \
         attachments_examined={attachments_examined}"
    );
}

/// Reduces live attachment media metadata to a non-sensitive evidence class.
fn mime_class(content_type: Option<&str>) -> &'static str {
    match content_type {
        Some("application/pdf") => "pdf",
        Some(value) if value.starts_with("text/") => "text",
        Some(_) => "other",
        None => "unknown",
    }
}

/// Reduces live attachment byte size to a non-sensitive evidence class.
fn size_class(byte_size: Option<u64>) -> &'static str {
    const ONE_MIB: u64 = 1_024 * 1_024;
    match byte_size {
        Some(size) if size < ONE_MIB => "under_1_mib",
        Some(size) if size < 10 * ONE_MIB => "1_to_10_mib",
        Some(size) if size < 25 * ONE_MIB => "10_to_25_mib",
        Some(_) => "over_25_mib",
        None => "unknown",
    }
}
