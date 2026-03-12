//! Length-delimited bincode codec for QUIC streams.

use mysticeti_core::NetworkMessage;
use std::io;

/// Max message size (4 MiB) to avoid allocation bombs.
const MAX_MESSAGE_LEN: usize = 4 * 1024 * 1024;

/// Encode a message to bytes: 4-byte little-endian length + bincode payload.
pub fn encode(msg: &NetworkMessage) -> Result<Vec<u8>, io::Error> {
    let bytes = bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if bytes.len() > MAX_MESSAGE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    Ok(out)
}

/// Decode one message from the front of `buf`. Returns the message and the number of bytes consumed.
pub fn decode(buf: &[u8]) -> Result<Option<(NetworkMessage, usize)>, io::Error> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_MESSAGE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message length exceeds limit",
        ));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let msg: NetworkMessage =
        bincode::deserialize(&buf[4..4 + len]).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some((msg, 4 + len)))
}
