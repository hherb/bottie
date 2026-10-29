//! Size limits shared by every provider adapter, so a hostile or buggy server cannot grow memory without bound.

use futures_util::StreamExt;

use super::ProviderError;

/// Largest streamed tool-call argument text accepted for one call.
pub(crate) const MAX_STREAMED_ARGUMENT_BYTES: usize = 64 * 1_024;

/// Largest SSE frame or NDJSON line buffered while still waiting for its terminating boundary.
pub(crate) const MAX_STREAM_FRAME_BYTES: usize = 1_024 * 1_024;

/// Largest combined visible and reasoning text one provider round may stream.
pub(crate) const MAX_ROUND_TEXT_BYTES: usize = 4 * 1_024 * 1_024;

/// Largest non-success response body read when looking for a provider error message.
pub(crate) const MAX_ERROR_BODY_BYTES: usize = 64 * 1_024;

/// Largest model-discovery or model-detail response body accepted.
pub(crate) const MAX_DISCOVERY_BODY_BYTES: usize = 1_024 * 1_024;

/// Characters of a provider-supplied error message kept before the `…` marker; some providers echo part of the key.
pub(crate) const MAX_PROVIDER_MESSAGE_CHARACTERS: usize = 300;

/// Reads a response body, returning `Ok(None)` as soon as it is known to exceed `maximum_bytes`.
pub(crate) async fn read_bounded_body(
    response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Option<Vec<u8>>, reqwest::Error> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Ok(None);
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Ok(None);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(bytes))
}

/// Reads a non-success body as text; an unreadable or oversized body becomes empty so the status message is used.
pub(crate) async fn read_error_body(response: reqwest::Response) -> String {
    read_bounded_body(response, MAX_ERROR_BODY_BYTES)
        .await
        .ok()
        .flatten()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

/// Reads a discovery body up to [`MAX_DISCOVERY_BODY_BYTES`], mapping transport errors with the adapter's mapper.
pub(crate) async fn read_discovery_body(
    response: reqwest::Response,
    map_request_error: fn(reqwest::Error) -> ProviderError,
) -> Result<Vec<u8>, ProviderError> {
    read_bounded_body(response, MAX_DISCOVERY_BODY_BYTES)
        .await
        .map_err(map_request_error)?
        .ok_or_else(|| {
            ProviderError::malformed(
                "The provider returned a model list that was too large.",
                Some(format!(
                    "discovery response exceeded {MAX_DISCOVERY_BODY_BYTES} bytes"
                )),
            )
        })
}

/// Shortens a provider-supplied message to [`MAX_PROVIDER_MESSAGE_CHARACTERS`], marking the cut with `…`.
pub(crate) fn truncate_provider_message(message: String) -> String {
    match message.char_indices().nth(MAX_PROVIDER_MESSAGE_CHARACTERS) {
        Some((cut, _)) => format!("{}…", &message[..cut]),
        None => message,
    }
}

/// Rejects a round once its accumulated text (`total_bytes`) passes [`MAX_ROUND_TEXT_BYTES`].
pub(crate) fn ensure_round_text_within_limit(total_bytes: usize) -> Result<(), ProviderError> {
    if total_bytes > MAX_ROUND_TEXT_BYTES {
        return Err(ProviderError::malformed(
            "The provider sent more text than bottie accepts for one response.",
            Some(format!(
                "streamed text exceeded {MAX_ROUND_TEXT_BYTES} bytes"
            )),
        ));
    }
    Ok(())
}

/// Rejects a decoder buffer that grew past [`MAX_STREAM_FRAME_BYTES`] without a frame boundary.
pub(crate) fn ensure_frame_within_limit(buffered_bytes: usize) -> Result<(), ProviderError> {
    if buffered_bytes > MAX_STREAM_FRAME_BYTES {
        return Err(ProviderError::malformed(
            "The provider sent a stream event that was too large.",
            Some(format!(
                "unterminated stream frame exceeded {MAX_STREAM_FRAME_BYTES} bytes"
            )),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_accept_the_boundary_and_reject_one_byte_more() {
        assert!(ensure_round_text_within_limit(MAX_ROUND_TEXT_BYTES).is_ok());
        assert!(ensure_round_text_within_limit(MAX_ROUND_TEXT_BYTES + 1).is_err());
        assert!(ensure_frame_within_limit(MAX_STREAM_FRAME_BYTES).is_ok());
        assert!(ensure_frame_within_limit(MAX_STREAM_FRAME_BYTES + 1).is_err());
    }

    fn response(body: Vec<u8>) -> reqwest::Response {
        reqwest::Response::from(tauri::http::Response::new(body))
    }

    #[tokio::test]
    async fn bounded_body_reads_up_to_the_limit_and_drops_oversized_error_bodies() {
        assert_eq!(
            read_bounded_body(response(b"ok".to_vec()), 2)
                .await
                .unwrap(),
            Some(b"ok".to_vec())
        );
        assert_eq!(
            read_bounded_body(response(b"ok!".to_vec()), 2)
                .await
                .unwrap(),
            None
        );
        let oversized = vec![b'x'; MAX_ERROR_BODY_BYTES + 1];
        assert_eq!(read_error_body(response(oversized)).await, "");
    }

    #[tokio::test]
    async fn bounded_body_stops_a_chunked_body_with_no_declared_length() {
        let chunks = futures_util::stream::iter(
            [b"ok".to_vec(), b"!".to_vec()].map(Ok::<_, std::io::Error>),
        );
        let chunked = reqwest::Response::from(tauri::http::Response::new(
            reqwest::Body::wrap_stream(chunks),
        ));
        assert_eq!(chunked.content_length(), None);
        assert_eq!(read_bounded_body(chunked, 2).await.unwrap(), None);
    }

    #[test]
    fn provider_messages_are_cut_at_the_character_limit() {
        let short = "é".repeat(MAX_PROVIDER_MESSAGE_CHARACTERS);
        assert_eq!(truncate_provider_message(short.clone()), short);
        let long = truncate_provider_message("é".repeat(MAX_PROVIDER_MESSAGE_CHARACTERS + 5));
        assert_eq!(long.chars().count(), MAX_PROVIDER_MESSAGE_CHARACTERS + 1);
        assert!(long.ends_with('…'));
    }
}
