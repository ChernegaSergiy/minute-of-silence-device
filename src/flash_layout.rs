//! Bounds checks for the dedicated audio cache partition.

pub const AUDIO_CACHE_OFFSET: u32 = 0x190000;
const AUDIO_CACHE_SIZE: u32 = 0x260000;
/// Start of the `settings` partition in `partitions.csv`.
pub const SETTINGS_OFFSET: u32 = 0x3f0000;
const SETTINGS_SIZE: u32 = 0x10000;
const HEADER_SIZE: u32 = 16;
const ENTRY_SIZE: u32 = 12;
const MAX_ENTRIES: u16 = 8;
const WAV_HEADER_SIZE: u32 = 44;

pub fn valid_entry(count: u16, offset: u32, length: u32) -> bool {
    if count == 0 || count > MAX_ENTRIES || length < WAV_HEADER_SIZE {
        return false;
    }
    let start = AUDIO_CACHE_OFFSET + HEADER_SIZE + u32::from(count) * ENTRY_SIZE;
    let end = AUDIO_CACHE_OFFSET + AUDIO_CACHE_SIZE;
    offset >= start && offset.checked_add(length).is_some_and(|last| last <= end)
}

pub fn valid_wav_length(entry_length: u32, data_length: u32) -> bool {
    entry_length
        .checked_sub(WAV_HEADER_SIZE)
        .is_some_and(|capacity| data_length <= capacity)
}

const _: () = assert!(
    crate::settings_record::SLOT_SIZE * crate::settings_record::SLOT_COUNT as u32 <= SETTINGS_SIZE
);
const _: () = assert!(AUDIO_CACHE_OFFSET + AUDIO_CACHE_SIZE <= SETTINGS_OFFSET);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_outside_overflowing_and_metadata_ranges() {
        let data_start = AUDIO_CACHE_OFFSET + HEADER_SIZE + ENTRY_SIZE;
        assert!(valid_entry(1, data_start, 44));
        assert!(!valid_entry(0, data_start, 44));
        assert!(!valid_entry(9, data_start, 44));
        assert!(!valid_entry(1, data_start - 1, 44));
        assert!(!valid_entry(1, u32::MAX - 5, 44));
        assert!(!valid_entry(
            1,
            AUDIO_CACHE_OFFSET + AUDIO_CACHE_SIZE - 43,
            44
        ));
    }

    #[test]
    fn rejects_audio_larger_than_its_entry() {
        assert!(valid_wav_length(48, 4));
        assert!(!valid_wav_length(48, 5));
        assert!(!valid_wav_length(43, 0));
    }
}
