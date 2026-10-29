//! Size limits shared by every provider adapter, so a hostile or buggy server cannot grow memory without bound.

use super::ProviderError;

/// Largest streamed tool-call argument text accepted for one call.
pub(crate) const MAX_STREAMED_ARGUMENT_BYTES: usize = 64 * 1_024;

/// Largest SSE frame or NDJSON line buffered while still waiting for its terminating boundary.
pub(crate) const MAX_STREAM_FRAME_BYTES: usize = 1_024 * 1_024;

/// Largest combined visible and reasoning text one provider round may stream.
pub(crate) const MAX_ROUND_TEXT_BYTES: usize = 4 * 1_024 * 1_024;

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
}
