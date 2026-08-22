//! Native public-web request and network-policy tests.

use super::*;

#[test]
fn request_normalizes_public_urls_and_removes_fragments() {
    let request = WebFetchRequest::new("  https://WWW.IANA.ORG/release?q=rust#notes  ").unwrap();
    assert_eq!(request.url(), "https://www.iana.org/release?q=rust");

    for value in [
        "",
        "file:///private/secret",
        "http://127.0.0.1/private",
        "http://[::1]/private",
        "http://localhost/private",
        "https://device.local/private",
        "https://user:secret@www.iana.org/private",
        "https://www.iana.org:8443/private",
        "https://-bad.iana.org/private",
        "https://example.com/private",
        "https://resolver.arpa/private",
    ] {
        assert!(
            WebFetchRequest::new(value).is_err(),
            "unsafe URL unexpectedly passed: {value}"
        );
    }
    assert!(
        WebFetchRequest::new(format!(
            "https://www.iana.org/{}",
            "x".repeat(MAX_WEB_FETCH_URL_CHARS)
        ))
        .is_err()
    );
}

#[test]
fn public_ip_policy_rejects_local_reserved_and_transition_ranges() {
    for address in [
        "0.0.0.0",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.1.1",
        "172.16.0.1",
        "192.0.2.1",
        "192.88.99.1",
        "192.168.1.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "::",
        "::1",
        "::8.8.8.8",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
        "100::1",
        "100:0:0:1::1",
        "2001:db8::1",
        "2002:0808:0808::1",
        "3fff::1",
        "5f00::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
    ] {
        assert!(
            !is_public_ip(address.parse().unwrap()),
            "non-public address unexpectedly passed: {address}"
        );
    }
    for address in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        assert!(
            is_public_ip(address.parse().unwrap()),
            "public address unexpectedly failed: {address}"
        );
    }
}

#[test]
fn fixed_error_categories_never_reflect_urls_or_response_material() {
    for error in [
        invalid_request(),
        blocked_address(),
        redirect_error(),
        unsupported_content_type(),
        response_too_large(),
        malformed_response(),
        WebFetchError::unavailable(),
        internal_error(),
    ] {
        assert!(!error.message.contains("example.com"));
        assert!(!error.message.contains("private"));
        assert!(!error.message.contains("response body"));
    }
}

#[test]
#[ignore = "requires loopback fixture access"]
fn follows_bounded_relative_redirects_and_returns_only_utf8_page_source() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for response in [
            concat!(
                "HTTP/1.1 302 Found\r\n",
                "Location: /final#section\r\n",
                "Content-Length: 0\r\n",
                "Connection: close\r\n\r\n"
            )
            .to_owned(),
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/html; charset=utf-8\r\n",
                "Content-Length: 31\r\n",
                "Connection: close\r\n\r\n",
                "<main>Untrusted fixture.</main>"
            )
            .to_owned(),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2_048];
            let _ = stream.read(&mut request).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    let request = WebFetchRequest::for_loopback_fixture(&format!("http://{address}/start"));
    let response =
        tauri::async_runtime::block_on(NativeWebFetch::for_loopback_fixture().fetch(request))
            .unwrap();
    server.join().unwrap();

    assert_eq!(response.final_url, format!("http://{address}/final"));
    assert_eq!(response.content_type, "text/html");
    assert_eq!(response.content, "<main>Untrusted fixture.</main>");
    assert!(response.untrusted);
}

#[test]
#[ignore = "requires loopback fixture access"]
fn rejects_unsupported_or_oversized_loopback_responses() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    for (content_type, length, expected) in [
        (
            "application/pdf",
            0,
            WebFetchErrorCode::UnsupportedContentType,
        ),
        (
            "text/html; charset=iso-8859-1",
            0,
            WebFetchErrorCode::UnsupportedContentType,
        ),
        (
            "text/plain",
            MAX_WEB_FETCH_RESPONSE_BYTES + 1,
            WebFetchErrorCode::ResponseTooLarge,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2_048];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: {}\r\n",
                    "Content-Length: {}\r\n",
                    "Connection: close\r\n\r\n"
                ),
                content_type, length
            )
            .unwrap();
        });
        let request = WebFetchRequest::for_loopback_fixture(&format!("http://{address}/page"));
        let error =
            tauri::async_runtime::block_on(NativeWebFetch::for_loopback_fixture().fetch(request))
                .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.code, expected);
    }
}

#[test]
#[ignore = "requires loopback fixture access"]
fn enforces_redirect_and_total_timeout_ceilings() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    let redirect_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let redirect_address = redirect_listener.local_addr().unwrap();
    let redirect_server = thread::spawn(move || {
        for _ in 0..=MAX_WEB_FETCH_REDIRECTS {
            let (mut stream, _) = redirect_listener.accept().unwrap();
            let mut request = [0_u8; 2_048];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: /again\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        }
    });
    let request =
        WebFetchRequest::for_loopback_fixture(&format!("http://{redirect_address}/start"));
    let redirect_error =
        tauri::async_runtime::block_on(NativeWebFetch::for_loopback_fixture().fetch(request))
            .unwrap_err();
    redirect_server.join().unwrap();
    assert_eq!(redirect_error.code, WebFetchErrorCode::RedirectRejected);

    let timeout_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let timeout_address = timeout_listener.local_addr().unwrap();
    let timeout_server = thread::spawn(move || {
        let (mut stream, _) = timeout_listener.accept().unwrap();
        let mut request = [0_u8; 2_048];
        let _ = stream.read(&mut request).unwrap();
        thread::sleep(Duration::from_millis(100));
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
        );
    });
    let request = WebFetchRequest::for_loopback_fixture(&format!("http://{timeout_address}/slow"));
    let timeout_error = tauri::async_runtime::block_on(
        NativeWebFetch::for_timeout_fixture(Duration::from_millis(20)).fetch(request),
    )
    .unwrap_err();
    timeout_server.join().unwrap();
    assert_eq!(timeout_error.code, WebFetchErrorCode::Timeout);
}

#[test]
#[ignore = "requires public internet access"]
fn fetches_one_public_https_page_under_production_network_policy() {
    let request = WebFetchRequest::new("https://www.iana.org/help/example-domains").unwrap();
    let response = tauri::async_runtime::block_on(NativeWebFetch::new().fetch(request)).unwrap();

    assert_eq!(
        response.final_url,
        "https://www.iana.org/help/example-domains"
    );
    assert_eq!(response.content_type, "text/html");
    assert!(!response.content.is_empty());
    assert!(response.untrusted);
}
