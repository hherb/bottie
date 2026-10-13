//! Loopback-only reuse of an already installed Qwen-Image 2.1 MLX Serve model.

use std::time::Duration;

use super::execution::{LOCAL_GENERATION_DIMENSIONS, LOCAL_GENERATION_OUTPUT_COUNT};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use tokio::sync::watch;
use url::Url;

/// Exact installed quantized pack accepted by the local image endpoint.
pub(crate) const IMAGE_MODEL_ID: &str = "Qwen-Image-2.1-MLX-Serve-8bit";
/// Durable provenance identifies the quantized package rather than relabelling its bytes.
pub(crate) const IMAGE_PACKAGE_ID: &str = "ddalcu/Qwen-Image-2.1-MLX-Serve-8bit";
/// Local-only endpoint; the existing oMLX text endpoint has no image output API.
pub(crate) const IMAGE_SERVER_BASE_URL: &str = "http://127.0.0.1:11234/";
/// Marks server-owned installation reuse without claiming a pinned worker bundle.
pub(crate) const IMAGE_SERVER_RUNTIME_ID: &str = "mlx-serve@external";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const INSPECTION_TIMEOUT: Duration = Duration::from_secs(3);
const GENERATION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_MODELS_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 34 * 1024 * 1024;
const MAX_PNG_BYTES: usize = 25 * 1024 * 1024;
const BASE_INFERENCE_STEPS: u32 = 40;
const MAX_PROMPT_CHARACTERS: usize = 1_000;

/// Freshly inspected local endpoint, never a native model path or a download instruction.
#[derive(Clone, Debug)]
pub(crate) struct LocalImageServer {
    client: Client,
    base_url: Url,
}

/// Cancellation stays distinct from fixed path-free endpoint or codec failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImageServerError {
    /// The client closed the local request and discarded every uncommitted result.
    Cancelled,
    /// The local endpoint or image bytes did not satisfy the closed request contract.
    Unavailable,
    /// A response exceeded bounds or was not exactly one inline PNG.
    InvalidResponse,
}

impl LocalImageServer {
    /// Requires explicit image capability from the exact already-loaded MLX Serve pack.
    pub(crate) async fn inspect() -> Result<Self, LocalImageServerError> {
        Self::inspect_at(IMAGE_SERVER_BASE_URL).await
    }

    async fn inspect_at(endpoint: &str) -> Result<Self, LocalImageServerError> {
        let server = Self::build(endpoint)?;
        let response = server
            .client
            .get(
                server
                    .base_url
                    .join("v1/models")
                    .map_err(|_| unavailable())?,
            )
            .timeout(INSPECTION_TIMEOUT)
            .send()
            .await
            .map_err(|_| unavailable())?;
        let bytes = bounded_body(response, MAX_MODELS_BYTES).await?;
        let models: Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        select_image_model(&models)?;
        Ok(server)
    }

    fn build(endpoint: &str) -> Result<Self, LocalImageServerError> {
        let base_url = Url::parse(endpoint).map_err(|_| unavailable())?;
        if base_url.scheme() != "http"
            || base_url.host_str() != Some("127.0.0.1")
            || base_url.port().is_none()
            || base_url.path() != "/"
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(unavailable());
        }
        let client = Client::builder()
            .no_proxy()
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self { client, base_url })
    }

    #[cfg(test)]
    /// Builds an isolated loopback endpoint for adversarial HTTP fixtures only.
    pub(super) fn for_fixture(endpoint: &str) -> Result<Self, LocalImageServerError> {
        Self::build(endpoint)
    }

    /// Requests one seeded inline PNG for the caller's native normalization and storage boundary.
    pub(crate) async fn generate(
        &self,
        prompt: &str,
        dimensions: (u32, u32),
        seed: u64,
        mut cancellation: watch::Receiver<bool>,
        mut on_progress: impl FnMut() + Send,
    ) -> Result<Vec<u8>, LocalImageServerError> {
        if *cancellation.borrow() {
            return Err(LocalImageServerError::Cancelled);
        }
        if dimensions != LOCAL_GENERATION_DIMENSIONS
            || prompt.trim().is_empty()
            || prompt.chars().count() > MAX_PROMPT_CHARACTERS
        {
            return Err(LocalImageServerError::InvalidResponse);
        }
        on_progress();
        let work = async {
            let (width, height) = dimensions;
            let response = self
                .client
                .post(
                    self.base_url
                        .join("v1/images/generations")
                        .map_err(|_| unavailable())?,
                )
                .timeout(GENERATION_TIMEOUT)
                .json(&json!({
                    "model": IMAGE_MODEL_ID, "prompt": prompt, "size": format!("{width}x{height}"),
                    "n": LOCAL_GENERATION_OUTPUT_COUNT, "steps": BASE_INFERENCE_STEPS, "seed": seed,
                    "response_format": "b64_json", "stream": false,
                }))
                .send()
                .await
                .map_err(|_| unavailable())?;
            let bytes = bounded_body(response, MAX_RESPONSE_BYTES).await?;
            decode_response(&bytes)
        };
        let result = tokio::select! {
            result = work => result,
            _ = wait_for_cancellation(&mut cancellation) => Err(LocalImageServerError::Cancelled),
        };
        if *cancellation.borrow() {
            return Err(LocalImageServerError::Cancelled);
        }
        result
    }
}

/// Rejects mere model-name discovery, text-only engines, other packages, and unloaded entries.
pub(super) fn select_image_model(models: &Value) -> Result<(), LocalImageServerError> {
    let supported = models
        .get("data")
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|model| {
                model.get("id").and_then(Value::as_str) == Some(IMAGE_MODEL_ID)
                    && model.get("owned_by").and_then(Value::as_str) == Some("mlx-serve")
                    && model.get("loaded").and_then(Value::as_bool) == Some(true)
                    && model.get("state").and_then(Value::as_str) == Some("ready")
                    && model
                        .get("capabilities")
                        .and_then(Value::as_array)
                        .is_some_and(|items| {
                            items.iter().any(|item| item.as_str() == Some("image"))
                        })
            })
        });
    if supported {
        Ok(())
    } else {
        Err(unavailable())
    }
}

async fn bounded_body(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, LocalImageServerError> {
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size > limit as u64)
    {
        return Err(malformed());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| unavailable())?;
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(malformed());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn decode_response(bytes: &[u8]) -> Result<Vec<u8>, LocalImageServerError> {
    let response: Value = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    let outputs = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(malformed)?;
    if outputs.len() != 1 || outputs[0].get("url").is_some() {
        return Err(malformed());
    }
    let encoded = outputs[0]
        .get("b64_json")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    let png = STANDARD.decode(encoded).map_err(|_| malformed())?;
    if png.len() > MAX_PNG_BYTES || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(malformed());
    }
    Ok(png)
}

async fn wait_for_cancellation(cancellation: &mut watch::Receiver<bool>) {
    while !*cancellation.borrow_and_update() {
        if cancellation.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

fn unavailable() -> LocalImageServerError {
    LocalImageServerError::Unavailable
}

fn malformed() -> LocalImageServerError {
    LocalImageServerError::InvalidResponse
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
