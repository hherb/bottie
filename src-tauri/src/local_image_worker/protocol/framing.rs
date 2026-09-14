//! Bounded unsigned big-endian framing for private worker JSON messages.

use serde::Serialize;

use super::ProtocolError;

/// Maximum JSON payload carried by one private-pipe frame.
pub(crate) const MAX_FRAME_BYTES: usize = 1_048_576;
/// Size of the unsigned big-endian payload-length prefix.
const FRAME_PREFIX_BYTES: usize = 4;

/// Incremental decoder for unsigned big-endian length-prefixed JSON payloads.
#[derive(Debug, Default)]
pub(crate) struct FrameDecoder {
    pending: Vec<u8>,
}

impl FrameDecoder {
    /// Creates an empty decoder for one private pipe direction.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Accepts another pipe chunk and returns every complete bounded JSON payload.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, ProtocolError> {
        self.pending.extend_from_slice(bytes);
        let mut payloads = Vec::new();
        loop {
            if self.pending.len() < FRAME_PREFIX_BYTES {
                return Ok(payloads);
            }
            let declared = u32::from_be_bytes(
                self.pending[..FRAME_PREFIX_BYTES]
                    .try_into()
                    .map_err(|_| ProtocolError::MalformedFrame)?,
            ) as usize;
            if declared == 0 {
                self.pending.clear();
                return Err(ProtocolError::MalformedFrame);
            }
            if declared > MAX_FRAME_BYTES {
                self.pending.clear();
                return Err(ProtocolError::FrameTooLarge);
            }
            let frame_bytes = FRAME_PREFIX_BYTES.saturating_add(declared);
            if self.pending.len() < frame_bytes {
                return Ok(payloads);
            }
            let frame = self.pending.drain(..frame_bytes).collect::<Vec<_>>();
            payloads.push(frame[FRAME_PREFIX_BYTES..].to_vec());
        }
    }

    /// Confirms the pipe ended exactly between frames.
    pub(crate) fn finish(self) -> Result<(), ProtocolError> {
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::TruncatedFrame)
        }
    }
}

/// Serializes one closed message into a bounded length-prefixed frame.
pub(super) fn encode_frame(message: &impl Serialize) -> Result<Vec<u8>, ProtocolError> {
    let payload = serde_json::to_vec(message).map_err(|_| ProtocolError::MalformedMessage)?;
    if payload.is_empty() || payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let length = u32::try_from(payload.len()).map_err(|_| ProtocolError::FrameTooLarge)?;
    let mut frame = Vec::with_capacity(FRAME_PREFIX_BYTES + payload.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}
