//! Exact response validation for direct and Hugging Face-resolved model files.

use reqwest::{
    Response, StatusCode,
    header::{CONTENT_LENGTH, CONTENT_RANGE, ETAG, HeaderName, LOCATION},
};

use super::{DownloadError, source::ModelFileSource};
use crate::local_image_worker::model_download::source::ModelSourcePlan;

/// Resolver header binding a response to the requested immutable repository commit.
const X_REPO_COMMIT: &str = "x-repo-commit";
/// Resolver header binding a response to the reviewed file entity tag.
const X_LINKED_ETAG: &str = "x-linked-etag";
/// Optional resolver header declaring the linked large-file byte length.
const X_LINKED_SIZE: &str = "x-linked-size";

/// Validates a direct repository response before any response bytes are retained.
pub(super) fn validate_direct_response(
    response: &Response,
    offset: u64,
    total_size: u64,
    expected_etag: &str,
) -> Result<(), DownloadError> {
    if response.status().is_redirection()
        || exact_header(response, ETAG.as_str())? != Some(expected_etag)
    {
        return Err(DownloadError::InvalidResponse);
    }
    validate_file_response(response, offset, total_size)
}

/// Validates immutable Hugging Face resolver metadata and returns its safe second-hop URL.
pub(super) fn validate_hugging_face_resolution(
    response: &Response,
    plan: &ModelSourcePlan,
    source: &ModelFileSource,
    total_size: u64,
) -> Result<url::Url, DownloadError> {
    if !matches!(
        response.status(),
        StatusCode::FOUND | StatusCode::TEMPORARY_REDIRECT
    ) || exact_header(response, X_REPO_COMMIT)? != Some(plan.manifest().source_revision.as_str())
        || exact_header(response, X_LINKED_ETAG)? != Some(source.strong_etag.as_str())
    {
        return Err(DownloadError::InvalidResponse);
    }
    if let Some(linked_size) = exact_header(response, X_LINKED_SIZE)? {
        if linked_size.parse::<u64>().ok() != Some(total_size) {
            return Err(DownloadError::InvalidResponse);
        }
    }
    let location =
        exact_header(response, LOCATION.as_str())?.ok_or(DownloadError::InvalidResponse)?;
    plan.resolved_url(source, location)
}

/// Validates the final response reached through one approved Hugging Face resolution.
pub(super) fn validate_resolved_response(
    response: &Response,
    offset: u64,
    total_size: u64,
) -> Result<(), DownloadError> {
    if response.status().is_redirection() {
        return Err(DownloadError::InvalidResponse);
    }
    validate_file_response(response, offset, total_size)
}

fn validate_file_response(
    response: &Response,
    offset: u64,
    total_size: u64,
) -> Result<(), DownloadError> {
    let expected_remaining = total_size
        .checked_sub(offset)
        .ok_or(DownloadError::InvalidResponse)?;
    if exact_content_length(response)? != expected_remaining {
        return Err(DownloadError::InvalidResponse);
    }
    if offset == 0 {
        if response.status() != StatusCode::OK
            || exact_header(response, CONTENT_RANGE.as_str())?.is_some()
        {
            return Err(DownloadError::InvalidResponse);
        }
    } else {
        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(DownloadError::InvalidResponse);
        }
        let expected = format!("bytes {offset}-{}/{}", total_size - 1, total_size);
        if exact_header(response, CONTENT_RANGE.as_str())? != Some(expected.as_str()) {
            return Err(DownloadError::InvalidResponse);
        }
    }
    Ok(())
}

fn exact_content_length(response: &Response) -> Result<u64, DownloadError> {
    exact_header(response, CONTENT_LENGTH.as_str())?
        .and_then(|value| value.parse().ok())
        .ok_or(DownloadError::InvalidResponse)
}

fn exact_header<'a>(response: &'a Response, name: &str) -> Result<Option<&'a str>, DownloadError> {
    let name =
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| DownloadError::InvalidResponse)?;
    let mut values = response.headers().get_all(name).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(DownloadError::InvalidResponse);
    }
    first
        .map(|value| value.to_str().map_err(|_| DownloadError::InvalidResponse))
        .transpose()
}
