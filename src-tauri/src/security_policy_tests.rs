//! Contract tests for Bottie's compiled WebView policy and capability allowlist.

use serde_json::{Value, json};

const TAURI_CONFIG: &str = include_str!("../tauri.conf.json");
const DEFAULT_CAPABILITY: &str = include_str!("../capabilities/default.json");

/// Parses one checked-in JSON policy fixture.
fn policy_json(contents: &str) -> Value {
    serde_json::from_str(contents).expect("checked-in security policy should be valid JSON")
}

#[test]
fn csp_allows_only_bundled_ui_ipc_and_opaque_attachment_previews() {
    let config = policy_json(TAURI_CONFIG);
    let security = &config["app"]["security"];

    assert_eq!(security["capabilities"], json!(["default"]));
    assert!(security.get("dangerousRemoteDomainIpcAccess").is_none());
    assert!(security.get("assetProtocol").is_none());
    assert_eq!(
        security["csp"],
        json!({
            "base-uri": "'none'",
            "connect-src": "ipc: http://ipc.localhost",
            "default-src": "'self'",
            "font-src": "'self'",
            "form-action": "'none'",
            "frame-src": "'none'",
            "img-src": "'self' bottie-attachment: http://bottie-attachment.localhost",
            "object-src": "'none'",
            "script-src": "'self'",
            "style-src": "'self' 'unsafe-inline'"
        })
    );
}

#[test]
fn main_window_can_only_listen_for_and_release_native_events() {
    let capability = policy_json(DEFAULT_CAPABILITY);

    assert_eq!(capability["windows"], json!(["main"]));
    assert_eq!(
        capability["permissions"],
        json!(["core:event:allow-listen", "core:event:allow-unlisten"])
    );
}
