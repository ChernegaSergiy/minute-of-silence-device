//! MicroSD card reader driver and WAV stream parser.

/// Metadata parsed from standard RIFF/WAVE audio headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub data_length: u32,
}

impl WavHeader {
    /// Attempts to parse a standard 44-byte uncompressed PCM WAV header.
    pub fn parse(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < 44 {
            return None;
        }

        // Check RIFF and WAVE magic signatures
        if &buffer[0..4] != b"RIFF" || &buffer[8..12] != b"WAVE" {
            return None;
        }

        // Subchunk 1 "fmt "
        if &buffer[12..16] != b"fmt " {
            return None;
        }

        let audio_format = u16::from_le_bytes([buffer[20], buffer[21]]);
        if audio_format != 1 {
            // 1 = uncompressed PCM
            return None;
        }

        let channels = u16::from_le_bytes([buffer[22], buffer[23]]);
        let sample_rate = u32::from_le_bytes([buffer[24], buffer[25], buffer[26], buffer[27]]);
        let bits_per_sample = u16::from_le_bytes([buffer[34], buffer[35]]);

        // Subchunk 2 "data"
        if &buffer[36..40] != b"data" {
            return None;
        }

        let data_length = u32::from_le_bytes([buffer[40], buffer[41], buffer[42], buffer[43]]);

        Some(Self {
            channels,
            sample_rate,
            bits_per_sample,
            data_length,
        })
    }
}
