//! Provider-neutral server-sent-event frame decoding.

use super::{ProviderError, limits::ensure_frame_within_limit};

/// Incrementally decodes UTF-8 SSE frames into their joined `data` payloads.
///
/// Line endings (`\r\n`, `\r`, `\n`) are normalized to `\n` as bytes arrive, so a frame boundary is always
/// `\n\n`. Scanning resumes where the last search stopped, and a frame that grows past
/// [`MAX_STREAM_FRAME_BYTES`](super::limits::MAX_STREAM_FRAME_BYTES) without a boundary is rejected.
#[derive(Default)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
    /// Bytes at the start of `buffer` already known to contain no boundary.
    scanned: usize,
    /// The previous byte was `\r`, so an immediately following `\n` (in this chunk or the next) ends the same line.
    pending_cr: bool,
}

impl SseDecoder {
    /// Appends bytes and returns every newly completed SSE data payload.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        for &byte in bytes {
            if std::mem::take(&mut self.pending_cr) && byte == b'\n' {
                continue;
            }
            if byte == b'\r' {
                self.pending_cr = true;
                self.buffer.push(b'\n');
            } else {
                self.buffer.push(byte);
            }
        }
        self.drain(false)
    }

    /// Flushes a final unterminated SSE frame when the stream closes.
    pub(crate) fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        self.drain(true)
    }

    fn drain(&mut self, finish: bool) -> Result<Vec<String>, ProviderError> {
        let mut payloads = Vec::new();
        // Back up one byte: the first `\n` of a boundary may be the last byte scanned before.
        while let Some(index) = find_event_boundary(&self.buffer, self.scanned.saturating_sub(1)) {
            let frame = self.buffer.drain(..index).collect::<Vec<_>>();
            self.buffer.drain(..2);
            self.scanned = 0;
            if let Some(payload) = decode_sse_frame(&frame)? {
                payloads.push(payload);
            }
        }
        self.scanned = self.buffer.len();
        ensure_frame_within_limit(self.buffer.len())?;
        if finish && !self.buffer.is_empty() {
            let frame = std::mem::take(&mut self.buffer);
            self.scanned = 0;
            if let Some(payload) = decode_sse_frame(&frame)? {
                payloads.push(payload);
            }
        }
        Ok(payloads)
    }
}

/// Finds the first blank-line boundary (`\n\n`) at or after `from`.
fn find_event_boundary(bytes: &[u8], from: usize) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| from + index)
}

fn decode_sse_frame(frame: &[u8]) -> Result<Option<String>, ProviderError> {
    let text = std::str::from_utf8(frame).map_err(|error| {
        ProviderError::malformed(
            "A provider sent invalid text in its stream.",
            Some(error.to_string()),
        )
    })?;
    // The SSE spec removes exactly one space after `data:`; further indentation is part of the value.
    let data = text
        .split('\n')
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|value| value.strip_prefix(' ').unwrap_or(value))
        .collect::<Vec<_>>();
    Ok((!data.is_empty()).then(|| data.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::limits::MAX_STREAM_FRAME_BYTES;

    #[test]
    fn decodes_fragmented_lf_and_crlf_frames() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push(b"event: delta\ndata: hel").unwrap().is_empty());
        assert_eq!(
            decoder.push(b"lo\r\n\r\ndata: world\n\n").unwrap(),
            ["hello", "world"]
        );
    }

    #[test]
    fn decodes_bare_cr_frames_even_when_crlf_is_split_across_chunks() {
        let mut decoder = SseDecoder::default();
        assert_eq!(decoder.push(b"data: a\r\r").unwrap(), ["a"]);
        assert!(decoder.push(b"data: b\r").unwrap().is_empty());
        assert_eq!(decoder.push(b"\n\r\n").unwrap(), ["b"]);
    }

    #[test]
    fn strips_only_one_leading_space_from_data_values() {
        let mut decoder = SseDecoder::default();
        assert_eq!(
            decoder.push(b"data:   indented\n\n").unwrap(),
            ["  indented"]
        );
        assert_eq!(decoder.push(b"data:tight\n\n").unwrap(), ["tight"]);
    }

    #[test]
    fn rejects_an_unterminated_frame_larger_than_the_limit() {
        let mut decoder = SseDecoder::default();
        let chunk = vec![b'x'; MAX_STREAM_FRAME_BYTES / 2];
        assert!(decoder.push(b"data: ").unwrap().is_empty());
        assert!(decoder.push(&chunk).unwrap().is_empty());
        let error = decoder.push(&chunk).unwrap_err();
        assert_eq!(
            error.code,
            crate::inference::ProviderErrorCode::MalformedResponse
        );
    }
}
