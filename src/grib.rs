use bytes::{Buf, BytesMut};
use std::io::Cursor;

/// Streaming parser for GRIB2 messages.
/// Accumulates incoming bytes and extracts complete GRIB2 messages.
pub struct Grib2StreamParser {
    buffer: BytesMut,
}

impl Grib2StreamParser {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(64 * 1024),
        }
    }

    /// Feed incoming bytes and return any complete GRIB2 messages.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(data);
        let mut messages = Vec::new();

        while let Some(msg) = self.try_extract_message() {
            messages.push(msg);
        }
        messages
    }

    /// Try to extract a complete GRIB2 message from the buffer.
    fn try_extract_message(&mut self) -> Option<Vec<u8>> {
        // Find "GRIB" magic bytes
        let pos = self.buffer.windows(4).position(|w| w == b"GRIB")?;

        // Skip any garbage before "GRIB"
        if pos > 0 {
            self.buffer.advance(pos);
        }

        // Need at least 16 bytes for the indicator section
        if self.buffer.len() < 16 {
            return None;
        }

        // Read message length from octets 8-15 (8-byte big-endian)
        let len_bytes: [u8; 8] = self.buffer[8..16].try_into().ok()?;
        let msg_len = u64::from_be_bytes(len_bytes) as usize;

        // Sanity check: messages shouldn't be larger than 1GB
        if msg_len > 1_000_000_000 {
            // Invalid message, skip these 4 bytes and try again
            self.buffer.advance(4);
            return None;
        }

        // Check if we have the complete message
        if self.buffer.len() < msg_len {
            return None;
        }

        // Extract message
        let msg = self.buffer.split_to(msg_len).to_vec();

        // Verify ends with "7777"
        if msg.len() < 4 || &msg[msg.len() - 4..] != b"7777" {
            // Invalid message ending, data might be corrupted
            return None;
        }

        Some(msg)
    }
}

impl Default for Grib2StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a GRIB2 message contains a wind variable (UGRD or VGRD).
///
/// Uses the grib crate to parse the message and check the parameter
/// category and number in the product definition section.
pub fn is_wind_message(msg: &[u8]) -> bool {
    let Ok(grib2) = grib::from_reader(Cursor::new(msg)) else {
        return false;
    };

    for (_, submsg) in grib2.iter() {
        // Get the product definition section
        let prod_def = submsg.prod_def();

        // Parameter category 2 = Momentum
        // Parameter number 2 = UGRD (U-component of wind)
        // Parameter number 3 = VGRD (V-component of wind)
        let Some(cat) = prod_def.parameter_category() else {
            continue;
        };
        let Some(num) = prod_def.parameter_number() else {
            continue;
        };

        if cat == 2 && (num == 2 || num == 3) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_incomplete_message() {
        let mut parser = Grib2StreamParser::new();

        // Feed partial GRIB header
        let messages = parser.feed(b"GRIB");
        assert!(messages.is_empty());

        // Feed more partial data
        let messages = parser.feed(&[0u8; 12]);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_parser_finds_grib_magic() {
        let mut parser = Grib2StreamParser::new();

        // Feed garbage then GRIB magic
        let mut data = vec![0u8; 100];
        data[50..54].copy_from_slice(b"GRIB");
        let messages = parser.feed(&data);

        // Should not return anything yet (incomplete message)
        assert!(messages.is_empty());

        // Buffer should have advanced past garbage
        assert!(parser.buffer.starts_with(b"GRIB"));
    }
}
