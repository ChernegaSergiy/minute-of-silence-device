//! Loading and durably saving settings in the `settings` flash partition.

use alloc::{vec, vec::Vec};
use khvylyna_core::Settings;

use crate::settings_record::{self as record, HEADER_LEN, SLOT_COUNT, SLOT_SIZE, WRITE_ALIGN};

/// Sector size used to bound erases to what a record actually needs.
const SECTOR_SIZE: u32 = 4096;

/// Raw flash access needed by the settings store. Offsets are absolute flash addresses.
pub trait SettingsFlash {
    /// Reads `buf.len()` bytes; `offset` and the length are multiples of [`WRITE_ALIGN`].
    fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), ()>;
    /// Erases the sector-aligned range `from..to`.
    fn erase(&mut self, from: u32, to: u32) -> Result<(), ()>;
    /// Writes `bytes`; `offset` and the length are multiples of [`WRITE_ALIGN`].
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ()>;
}

/// Why settings could not be saved.
#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    /// The settings could not be serialized or do not fit into a slot.
    Encode,
    /// Erasing or writing the flash failed.
    Flash,
    /// The data read back after writing differs from what was written.
    Verify,
}

/// Settings persisted in two alternating flash slots.
pub struct SettingsStore<F: SettingsFlash> {
    flash: F,
    base: u32,
    sequences: [Option<u32>; SLOT_COUNT],
}

impl<F: SettingsFlash> SettingsStore<F> {
    /// Scans both slots and returns the store with the newest valid saved settings, if any.
    pub fn open(flash: F, base: u32) -> (Self, Option<Settings>) {
        let mut store = Self {
            flash,
            base,
            sequences: [None; SLOT_COUNT],
        };
        let mut loaded: [Option<Settings>; SLOT_COUNT] = [None, None];
        for slot in 0..SLOT_COUNT {
            if let Some((sequence, settings)) = store.read_slot(slot) {
                store.sequences[slot] = Some(sequence);
                loaded[slot] = Some(settings);
            }
        }
        let settings = record::newest(store.sequences).and_then(|slot| loaded[slot].take());
        (store, settings)
    }

    /// Writes `settings` into the slot that does not hold the newest valid record, then reads it
    /// back. The previous settings remain valid until this returns successfully.
    pub fn save(&mut self, settings: &Settings) -> Result<(), SaveError> {
        let payload = serde_json::to_vec(settings).map_err(|_| SaveError::Encode)?;
        let target = record::next_target(self.sequences);
        let bytes = record::encode(target.sequence, &payload).ok_or(SaveError::Encode)?;
        let start = self.slot_offset(target.slot);
        let erase_end = start + (bytes.len() as u32).next_multiple_of(SECTOR_SIZE);
        self.flash
            .erase(start, erase_end)
            .map_err(|()| SaveError::Flash)?;
        self.flash
            .write(start, &bytes)
            .map_err(|()| SaveError::Flash)?;
        match self.read_slot(target.slot) {
            Some((sequence, stored)) if sequence == target.sequence && stored == *settings => {
                self.sequences[target.slot] = Some(sequence);
                Ok(())
            }
            _ => Err(SaveError::Verify),
        }
    }

    fn slot_offset(&self, slot: usize) -> u32 {
        self.base + slot as u32 * SLOT_SIZE
    }

    fn read_slot(&mut self, slot: usize) -> Option<(u32, Settings)> {
        let offset = self.slot_offset(slot);
        let mut header = [0u8; HEADER_LEN];
        self.flash.read(offset, &mut header).ok()?;
        let length = record::record_len(&header)?;
        let mut bytes = vec![0u8; length.next_multiple_of(WRITE_ALIGN)];
        self.flash.read(offset, &mut bytes).ok()?;
        let found = record::decode(&bytes)?;
        Some((found.sequence, parse(found.payload)?))
    }
}

fn parse(payload: &[u8]) -> Option<Settings> {
    let mut settings: Settings = serde_json::from_slice(payload).ok()?;
    settings.migrate();
    settings.validate().ok()?;
    Some(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u32 = 0x3f0000;

    struct RamFlash {
        bytes: Vec<u8>,
        fail_writes_after: Option<usize>,
        written: usize,
    }

    impl RamFlash {
        fn new() -> Self {
            Self {
                bytes: vec![0xFF; (2 * SLOT_SIZE) as usize],
                fail_writes_after: None,
                written: 0,
            }
        }
    }

    impl SettingsFlash for &mut RamFlash {
        fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), ()> {
            let start = (offset - BASE) as usize;
            buf.copy_from_slice(self.bytes.get(start..start + buf.len()).ok_or(())?);
            Ok(())
        }

        fn erase(&mut self, from: u32, to: u32) -> Result<(), ()> {
            assert_eq!(from % SECTOR_SIZE, 0);
            assert_eq!(to % SECTOR_SIZE, 0);
            self.bytes[(from - BASE) as usize..(to - BASE) as usize].fill(0xFF);
            Ok(())
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ()> {
            assert_eq!(offset as usize % WRITE_ALIGN, 0);
            assert_eq!(bytes.len() % WRITE_ALIGN, 0);
            let start = (offset - BASE) as usize;
            let allowed = match self.fail_writes_after {
                Some(limit) => limit.saturating_sub(self.written).min(bytes.len()),
                None => bytes.len(),
            };
            for (index, byte) in bytes[..allowed].iter().enumerate() {
                // Flash can only clear bits; programming over written data is a bug.
                assert_eq!(self.bytes[start + index], 0xFF, "write over unerased flash");
                self.bytes[start + index] = *byte;
            }
            self.written += allowed;
            if allowed < bytes.len() {
                Err(())
            } else {
                Ok(())
            }
        }
    }

    fn changed(volume: u8) -> Settings {
        Settings {
            volume,
            ..Settings::default()
        }
    }

    #[test]
    fn empty_flash_has_no_saved_settings() {
        let mut flash = RamFlash::new();
        let (_, settings) = SettingsStore::open(&mut flash, BASE);
        assert_eq!(settings, None);
    }

    #[test]
    fn saved_settings_are_restored_after_reopening() {
        let mut flash = RamFlash::new();
        let (mut store, _) = SettingsStore::open(&mut flash, BASE);
        store.save(&changed(33)).expect("save");
        store.save(&changed(44)).expect("save");
        store.save(&changed(55)).expect("save");
        drop(store);
        let (_, restored) = SettingsStore::open(&mut flash, BASE);
        assert_eq!(restored, Some(changed(55)));
    }

    #[test]
    fn saves_alternate_between_slots() {
        let mut flash = RamFlash::new();
        let (mut store, _) = SettingsStore::open(&mut flash, BASE);
        store.save(&changed(10)).expect("first");
        assert_eq!(store.sequences, [Some(1), None]);
        store.save(&changed(20)).expect("second");
        assert_eq!(store.sequences, [Some(1), Some(2)]);
        store.save(&changed(30)).expect("third");
        assert_eq!(store.sequences, [Some(3), Some(2)]);
    }

    #[test]
    fn a_write_cut_off_at_any_point_keeps_the_previous_settings() {
        let mut flash = RamFlash::new();
        let (mut store, _) = SettingsStore::open(&mut flash, BASE);
        store.save(&changed(10)).expect("baseline");
        let record_len = serde_json::to_vec(&changed(20)).expect("json").len() + HEADER_LEN;
        drop(store);
        for cut in 0..record_len.next_multiple_of(WRITE_ALIGN) {
            let mut torn = RamFlash::new();
            torn.bytes.copy_from_slice(&flash.bytes);
            let (mut store, _) = SettingsStore::open(&mut torn, BASE);
            store.flash.fail_writes_after = Some(cut);
            assert!(store.save(&changed(20)).is_err(), "cut {cut}");
            drop(store);
            let (_, restored) = SettingsStore::open(&mut torn, BASE);
            if cut < record_len {
                assert_eq!(restored, Some(changed(10)), "cut {cut}");
            } else {
                // Only the 0xFF alignment padding is missing, which equals erased flash.
                assert_eq!(restored, Some(changed(20)), "cut {cut}");
            }
        }
    }

    #[test]
    fn a_failed_save_does_not_lose_the_ability_to_save_again() {
        let mut flash = RamFlash::new();
        let (mut store, _) = SettingsStore::open(&mut flash, BASE);
        store.save(&changed(10)).expect("baseline");
        store.flash.fail_writes_after = Some(store.flash.written + 8);
        assert_eq!(store.save(&changed(20)), Err(SaveError::Flash));
        store.flash.fail_writes_after = None;
        store.save(&changed(30)).expect("retry");
        drop(store);
        let (_, restored) = SettingsStore::open(&mut flash, BASE);
        assert_eq!(restored, Some(changed(30)));
    }

    #[test]
    fn corrupted_newest_record_falls_back_to_the_older_one() {
        let mut flash = RamFlash::new();
        let (mut store, _) = SettingsStore::open(&mut flash, BASE);
        store.save(&changed(10)).expect("older");
        store.save(&changed(20)).expect("newer");
        drop(store);
        flash.bytes[SLOT_SIZE as usize + HEADER_LEN + 3] ^= 0x40;
        let (_, restored) = SettingsStore::open(&mut flash, BASE);
        assert_eq!(restored, Some(changed(10)));
    }

    #[test]
    fn records_that_fail_validation_are_ignored() {
        let mut flash = RamFlash::new();
        let bytes = record::encode(1, br#"{"volume": 101}"#).expect("encode");
        flash.bytes[..bytes.len()].copy_from_slice(&bytes);
        let (_, restored) = SettingsStore::open(&mut flash, BASE);
        assert_eq!(restored, None);
    }
}
