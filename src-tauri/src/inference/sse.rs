//! Provider-neutral server-sent-event frame decoding.

use super::ProviderError;

/// Incrementally decodes UTF-8 SSE frames into their joined `data` payloads.
#[derive(Default)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    /// Appends bytes and returns every newly completed SSE data payload.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(bytes);
        self.drain(false)
    }

    /// Flushes a final unterminated SSE frame when the stream closes.
    pub(crate) fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        self.drain(true)
    }

    fn drain(&mut self, finish: bool) -> Result<Vec<String>, ProviderError> {
        let mut payloads = Vec::new();
        while let Some((index, separator_len)) = find_event_boundary(&self.buffer) {
            let frame = self.buffer.drain(..index).collect::<Vec<_>>();
            self.buffer.drain(..separator_len);
            if let Some(payload) = decode_sse_frame(&frame)? {
                payloads.push(payload);
            }
        }
        if finish && !self.buffer.is_empty() {
            let frame = std::mem::take(&mut self.buffer);
            if let Some(payload) = decode_sse_frame(&frame)? {
                payloads.push(payload);
            }
        }
        Ok(payloads)
    }
}

fn find_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(boundary), None) | (None, Some(boundary)) => Some(boundary),
        (None, None) => None,
    }
}

fn decode_sse_frame(frame: &[u8]) -> Result<Option<String>, ProviderError> {
    let text = std::str::from_utf8(frame).map_err(|error| {
        ProviderError::malformed(
            "A provider sent invalid text in its stream.",
            Some(error.to_string()),
        )
    })?;
    let data = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
        .collect::<Vec<_>>();
    Ok((!data.is_empty()).then(|| data.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_fragmented_lf_and_crlf_frames() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push(b"event: delta\ndata: hel").unwrap().is_empty());
        assert_eq!(
            decoder.push(b"lo\r\n\r\ndata: world\n\n").unwrap(),
            ["hello", "world"]
        );
    }
}
