//! Validation of the PCM WAV format supported by the firmware.

/// Metadata parsed from a standard 44-byte PCM WAV header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub data_length: u32,
}

impl WavHeader {
    /// Parses a canonical mono or stereo 16-bit PCM WAV header.
    pub fn parse(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < 44
            || &buffer[0..4] != b"RIFF"
            || &buffer[8..12] != b"WAVE"
            || &buffer[12..16] != b"fmt "
            || u32::from_le_bytes(buffer[16..20].try_into().ok()?) != 16
            || u16::from_le_bytes(buffer[20..22].try_into().ok()?) != 1
            || &buffer[36..40] != b"data"
        {
            return None;
        }

        let channels = u16::from_le_bytes(buffer[22..24].try_into().ok()?);
        let sample_rate = u32::from_le_bytes(buffer[24..28].try_into().ok()?);
        let byte_rate = u32::from_le_bytes(buffer[28..32].try_into().ok()?);
        let block_align = u16::from_le_bytes(buffer[32..34].try_into().ok()?);
        let bits_per_sample = u16::from_le_bytes(buffer[34..36].try_into().ok()?);
        let data_length = u32::from_le_bytes(buffer[40..44].try_into().ok()?);
        if !(channels == 1 || channels == 2)
            || sample_rate == 0
            || bits_per_sample != 16
            || block_align != channels * 2
            || sample_rate.checked_mul(u32::from(block_align)) != Some(byte_rate)
            || data_length % u32::from(block_align) != 0
        {
            return None;
        }

        Some(Self {
            channels,
            sample_rate,
            bits_per_sample,
            data_length,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::WavHeader;

    fn header(channels: u16) -> [u8; 44] {
        let mut bytes = [0u8; 44];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes[12..16].copy_from_slice(b"fmt ");
        bytes[16..20].copy_from_slice(&16u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&1u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&channels.to_le_bytes());
        bytes[24..28].copy_from_slice(&44100u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&(44100u32 * u32::from(channels) * 2).to_le_bytes());
        bytes[32..34].copy_from_slice(&(channels * 2).to_le_bytes());
        bytes[34..36].copy_from_slice(&16u16.to_le_bytes());
        bytes[36..40].copy_from_slice(b"data");
        bytes[40..44].copy_from_slice(&8u32.to_le_bytes());
        bytes
    }

    #[test]
    fn accepts_mono_and_stereo_pcm() {
        assert_eq!(WavHeader::parse(&header(1)).expect("mono").channels, 1);
        assert_eq!(WavHeader::parse(&header(2)).expect("stereo").channels, 2);
    }

    #[test]
    fn rejects_unsupported_or_inconsistent_formats() {
        assert!(WavHeader::parse(&header(0)).is_none());
        assert!(WavHeader::parse(&header(3)).is_none());
        let mut bytes = header(2);
        bytes[34..36].copy_from_slice(&24u16.to_le_bytes());
        assert!(WavHeader::parse(&bytes).is_none());
        let mut bytes = header(2);
        bytes[40..44].copy_from_slice(&3u32.to_le_bytes());
        assert!(WavHeader::parse(&bytes).is_none());
        assert!(WavHeader::parse(&header(1)[..43]).is_none());
    }
}
