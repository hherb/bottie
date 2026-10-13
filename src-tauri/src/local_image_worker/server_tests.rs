//! Contracts for reusing the explicitly advertised local MLX image server.

use super::{
    IMAGE_MODEL_ID, LocalImageServer, LocalImageServerError, decode_response, select_image_model,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
use tokio::sync::watch;

#[test]
fn requires_the_exact_installed_pack_and_explicit_image_capability() {
    let mut model = json!({"id": IMAGE_MODEL_ID, "owned_by": "mlx-serve", "loaded": true,
        "state": "ready", "capabilities": ["image"]});
    assert!(select_image_model(&json!({"data": [model.clone()]})).is_ok());
    model["capabilities"] = json!(["text"]);
    assert!(select_image_model(&json!({"data": [model.clone()]})).is_err());
    model["capabilities"] = json!(["image"]);
    model["id"] = json!("other-model");
    assert!(select_image_model(&json!({"data": [model]})).is_err());
}

#[tokio::test]
async fn sends_one_seeded_image_request_without_credentials_or_path_fields() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/", listener.local_addr().unwrap());
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        assert!(request.starts_with("POST /v1/images/generations "));
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        let body: serde_json::Value =
            serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"model": IMAGE_MODEL_ID, "prompt": "A blue circle", "size": "512x512",
            "n": 1, "steps": 40, "seed": 42, "response_format": "b64_json", "stream": false})
        );
        let body = r#"{"data":[{"b64_json":"iVBORw0KGgo="}]}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let server = LocalImageServer::for_fixture(&endpoint).unwrap();
    let (_cancel, cancellation) = watch::channel(false);
    assert_eq!(
        server
            .generate("A blue circle", (512, 512), 42, cancellation, || {})
            .await
            .unwrap(),
        b"\x89PNG\r\n\x1a\n"
    );
    peer.join().unwrap();
}

#[tokio::test]
async fn cancellation_closes_the_inflight_local_connection_without_returning_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/", listener.local_addr().unwrap());
    let (started, received) = tokio::sync::oneshot::channel();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        started.send(()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let server = LocalImageServer::for_fixture(&endpoint).unwrap();
    let (cancel, cancellation) = watch::channel(false);
    let run = tokio::spawn(async move {
        server
            .generate("A blue circle", (512, 512), 42, cancellation, || {})
            .await
    });
    received.await.unwrap();
    cancel.send(true).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), run)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err(),
        LocalImageServerError::Cancelled
    );
    peer.join().unwrap();
}

/// Reads one bounded fixture body, preserving the actual wire request for assertions.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0; 4096];
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        assert!(bytes.len() < 64 * 1024);
        let text = String::from_utf8(bytes.clone()).unwrap();
        if let Some((headers, body)) = text.split_once("\r\n\r\n") {
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            if body.len() == length {
                return text;
            }
        }
    }
}

#[test]
fn refuses_remote_image_references_extra_outputs_and_invalid_base64() {
    for response in [
        json!({"data": [{"url": "https://example.com/image.png"}]}),
        json!({"data": []}),
        json!({"data": [{"b64_json": "wrong"}]}),
        json!({"data": [{"b64_json": "iVBORw0KGgo="}, {"b64_json": "iVBORw0KGgo="}]}),
        json!({"data": [{"b64_json": "iVBORw0KGgo=", "url": "http://127.0.0.1/"}]}),
    ] {
        assert_eq!(
            decode_response(&serde_json::to_vec(&response).unwrap()).unwrap_err(),
            LocalImageServerError::InvalidResponse
        );
    }
}

#[test]
fn local_server_never_accepts_remote_hosts_redirect_targets_or_url_credentials() {
    for endpoint in [
        "https://example.com/",
        "http://localhost.evil/",
        "http://user@127.0.0.1:11234/",
        "http://127.0.0.1:11234/path",
        "http://192.168.1.1:11234/",
    ] {
        assert!(LocalImageServer::for_fixture(endpoint).is_err());
    }
    assert!(LocalImageServer::for_fixture("http://127.0.0.1:11234/").is_ok());
}
