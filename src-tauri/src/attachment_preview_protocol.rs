//! Narrow custom-protocol boundary for native attachment thumbnails.

use tauri::http::{Method, Request, Response, StatusCode, header};

use crate::storage::ConversationStore;

/// Resolves one custom-protocol request without exposing a filesystem path.
pub(crate) fn response(store: &ConversationStore, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != Method::GET {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    let attachment_id = request.uri().path().trim_start_matches('/');
    if request.uri().query().is_some()
        || attachment_id.contains('/')
        || uuid::Uuid::parse_str(attachment_id).is_err()
    {
        return empty_response(StatusCode::NOT_FOUND);
    }
    let Ok(Some(preview)) = store.load_attachment_preview(attachment_id) else {
        return empty_response(StatusCode::NOT_FOUND);
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, preview.mime_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(preview.bytes)
        .expect("static preview response should build")
}

/// Builds one bodyless failure response without leaking native error details.
fn empty_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Vec::new())
        .expect("static preview response should build")
}
