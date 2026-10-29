//! Strict native download and normalization for temporary generated-image results.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode, header::CONTENT_TYPE};

use crate::{
    inference::{ProviderError, ProviderErrorCode},
    storage::{ConversationStore, PreparedGeneratedImage, normalize_generated_png},
    web_fetch::{WebFetchErrorCode, resolve_addresses},
};

use super::GeneratedImageReference;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DOWNLOAD_BYTES: u64 = 25 * 1_024 * 1_024;
const PNG_MEDIA_TYPE: &str = "image/png";
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Redirect-free downloader that retains temporary provider results only behind the native boundary.
///
/// Each download resolves the result host once, refuses private or loopback answers, and pins the connection to
/// those addresses with no proxy, so a provider-supplied URL cannot reach the user's local network.
#[derive(Clone)]
pub(crate) struct GeneratedImageDownloader {
    allow_loopback_http: bool,
}

impl GeneratedImageDownloader {
    /// Builds the production HTTPS-only, public-address-only generated-image downloader.
    pub(crate) fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            allow_loopback_http: false,
        })
    }

    /// Builds an isolated loopback-capable downloader for HTTP response fixture tests.
    #[cfg(test)]
    pub(super) fn for_fixture() -> Result<Self, ProviderError> {
        Ok(Self {
            allow_loopback_http: true,
        })
    }

    /// Creates one client with fixed timeouts, no redirects or proxy, and `host` pinned to `addresses`.
    fn pinned_client(host: &str, addresses: &[SocketAddr]) -> Result<Client, ProviderError> {
        Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(DOWNLOAD_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .resolve_to_addrs(host, addresses)
            .build()
            .map_err(|_| {
                ProviderError::internal(
                    "The generated-image downloader could not be created.",
                    None,
                )
            })
    }

    /// Downloads and normalizes every exact provider result, deleting all partials on failure.
    pub(crate) async fn download_all(
        &self,
        references: &[GeneratedImageReference],
        store: &ConversationStore,
    ) -> Result<Vec<PreparedGeneratedImage>, ProviderError> {
        let temporary_directory = store.generated_asset_temporary_directory();
        fs::create_dir_all(&temporary_directory).map_err(|_| generated_storage_error())?;
        let mut prepared = Vec::with_capacity(references.len());
        for reference in references {
            match self.download(reference, &temporary_directory).await {
                Ok(image) => prepared.push(image),
                Err(error) => {
                    cleanup_prepared(&prepared);
                    return Err(error);
                }
            }
        }
        Ok(prepared)
    }

    /// Streams one temporary result under fixed limits, then decodes and re-encodes it as PNG.
    async fn download(
        &self,
        reference: &GeneratedImageReference,
        temporary_directory: &Path,
    ) -> Result<PreparedGeneratedImage, ProviderError> {
        let url = reference.url();
        let parsed = url::Url::parse(url).map_err(|_| malformed_image())?;
        let allowed_scheme = parsed.scheme() == "https"
            || (self.allow_loopback_http
                && parsed.scheme() == "http"
                && parsed.host_str().is_some_and(is_loopback_host));
        if !allowed_scheme || parsed.username() != "" || parsed.password().is_some() {
            return Err(malformed_image());
        }
        let host = parsed.host_str().ok_or_else(malformed_image)?.to_owned();
        let addresses = resolve_addresses(
            &parsed,
            self.allow_loopback_http,
            Instant::now() + CONNECT_TIMEOUT,
        )
        .await
        .map_err(|error| match error.code {
            WebFetchErrorCode::BlockedAddress => ProviderError::malformed(
                "The image provider returned an image location on a private network.",
                None,
            ),
            _ => ProviderError::unavailable(
                "The temporary generated image could not be reached.",
                Some("temporary image host did not resolve".into()),
            ),
        })?;
        let response = Self::pinned_client(&host, &addresses)?
            .get(parsed)
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .await
            .map_err(download_transport_error)?;
        if response.status().is_redirection() {
            return Err(ProviderError::malformed(
                "The image provider returned an unsafe redirect.",
                None,
            ));
        }
        if response.status() != StatusCode::OK {
            return Err(ProviderError::unavailable(
                "The temporary generated image is no longer available.",
                Some(format!(
                    "temporary image returned HTTP {}",
                    response.status().as_u16()
                )),
            ));
        }
        let media_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if media_type != Some(PNG_MEDIA_TYPE) {
            return Err(ProviderError::malformed(
                "The image provider returned an unsupported image type.",
                None,
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length == 0 || length > MAX_DOWNLOAD_BYTES)
        {
            return Err(oversized_image());
        }
        let download_path = temporary_path(temporary_directory, "download");
        let _download_guard = TemporaryPath::new(download_path.clone());
        let normalized_path = temporary_path(temporary_directory, "png");
        let result = stream_response(response, &download_path)
            .await
            .and_then(|()| {
                sniff_png(&download_path)?;
                normalize_generated_png(
                    &download_path,
                    normalized_path.clone(),
                    reference.dimensions(),
                )
                .map_err(|error| ProviderError::malformed(error.message, None))
            });
        if result.is_err() {
            let _ = fs::remove_file(&normalized_path);
        }
        result
    }
}

/// Cancellation-safe owner for one native temporary path.
struct TemporaryPath {
    path: PathBuf,
}

impl TemporaryPath {
    /// Owns one path until its surrounding download operation ends.
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TemporaryPath {
    /// Removes partial bytes even when the asynchronous download future is aborted.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Streams a successful HTTP response to a create-new file without crossing the byte ceiling.
async fn stream_response(response: reqwest::Response, path: &Path) -> Result<(), ProviderError> {
    let mut file = create_temporary_file(path)?;
    let mut stream = response.bytes_stream();
    let mut written = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(download_transport_error)?;
        written = written
            .checked_add(chunk.len() as u64)
            .ok_or_else(oversized_image)?;
        if written > MAX_DOWNLOAD_BYTES {
            return Err(oversized_image());
        }
        file.write_all(&chunk)
            .map_err(|_| generated_storage_error())?;
    }
    if written == 0 {
        return Err(malformed_image());
    }
    file.flush().map_err(|_| generated_storage_error())?;
    file.sync_all().map_err(|_| generated_storage_error())?;
    Ok(())
}

/// Opens a unique native temporary file without replacing existing data.
fn create_temporary_file(path: &Path) -> Result<File, ProviderError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| generated_storage_error())
}

/// Requires the decoded body to identify as PNG before passing it to the bounded decoder.
fn sniff_png(path: &Path) -> Result<(), ProviderError> {
    let bytes = fs::read(path).map_err(|_| generated_storage_error())?;
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(malformed_image());
    }
    Ok(())
}

/// Builds a unique generated-image temporary filename with no provider-derived component.
fn temporary_path(directory: &Path, stage: &str) -> PathBuf {
    directory.join(format!("{}.{}.part", uuid::Uuid::new_v4(), stage))
}

/// Deletes prepared normalized outputs after a later output fails.
fn cleanup_prepared(images: &[PreparedGeneratedImage]) {
    for image in images {
        let _ = fs::remove_file(&image.temporary_path);
    }
}

/// Allows only literal loopback hostnames inside the test-only HTTP fixture boundary.
fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

/// Maps download timeouts separately from other transport failures without exposing a result URL.
fn download_transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError {
            code: ProviderErrorCode::Timeout,
            message: "The generated image download timed out.".into(),
            retryable: true,
            diagnostic: None,
        }
    } else {
        ProviderError::unavailable("The generated image could not be downloaded.", None)
    }
}

/// Returns the stable invalid-image response used for body and decode failures.
fn malformed_image() -> ProviderError {
    ProviderError::malformed("The image provider returned an invalid PNG image.", None)
}

/// Returns the stable response used when encoded bytes exceed the native ceiling.
fn oversized_image() -> ProviderError {
    ProviderError::malformed(
        "The generated image exceeded Bottie's download limit.",
        None,
    )
}

/// Returns the stable local-storage failure without exposing application-private paths.
fn generated_storage_error() -> ProviderError {
    ProviderError::internal("Bottie could not safely retain the generated image.", None)
}
