//! Conversion of little-endian 16-bit PCM into stereo output frames.

/// Converts complete PCM frames while preserving partial input across reads.
pub struct StereoPcm {
    channels: usize,
    volume: i32,
    input: [u8; 4],
    input_len: usize,
}

impl StereoPcm {
    /// Creates a converter for mono or stereo PCM and a volume from 0 to 100.
    pub fn new(channels: u16, volume: u8) -> Option<Self> {
        if !(channels == 1 || channels == 2) || volume > 100 {
            return None;
        }
        Some(Self {
            channels: usize::from(channels),
            volume: i32::from(volume),
            input: [0; 4],
            input_len: 0,
        })
    }

    /// Accepts one input byte and returns an output frame when complete.
    pub fn push(&mut self, byte: u8) -> Option<[u8; 4]> {
        self.input[self.input_len] = byte;
        self.input_len += 1;
        if self.input_len != self.channels * 2 {
            return None;
        }
        self.input_len = 0;
        let left = self.scale([self.input[0], self.input[1]]);
        let right = if self.channels == 1 {
            left
        } else {
            self.scale([self.input[2], self.input[3]])
        };
        Some([left[0], left[1], right[0], right[1]])
    }

    /// Returns whether the input ends on a complete PCM frame.
    pub fn is_complete(&self) -> bool {
        self.input_len == 0
    }

    fn scale(&self, bytes: [u8; 2]) -> [u8; 2] {
        let sample = i32::from(i16::from_le_bytes(bytes));
        ((sample * self.volume / 100) as i16).to_le_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::StereoPcm;

    #[test]
    fn duplicates_mono_and_preserves_split_frames() {
        let mut converter = StereoPcm::new(1, 100).expect("converter");
        assert_eq!(converter.push(0x34), None);
        assert!(!converter.is_complete());
        assert_eq!(converter.push(0x12), Some([0x34, 0x12, 0x34, 0x12]));
        assert!(converter.is_complete());
    }

    #[test]
    fn scales_signed_stereo_samples_and_handles_muting() {
        let mut converter = StereoPcm::new(2, 50).expect("converter");
        for byte in [0, 0x80, 0xff] {
            assert_eq!(converter.push(byte), None);
        }
        assert_eq!(converter.push(0x7f), Some([0, 0xc0, 0xff, 0x3f]));
        let mut muted = StereoPcm::new(1, 0).expect("converter");
        assert_eq!(muted.push(0xff), None);
        assert_eq!(muted.push(0x7f), Some([0; 4]));
    }

    #[test]
    fn rejects_unsupported_channels_and_volume() {
        assert!(StereoPcm::new(0, 50).is_none());
        assert!(StereoPcm::new(3, 50).is_none());
        assert!(StereoPcm::new(1, 101).is_none());
    }
}
