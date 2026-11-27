//! Internal Flash audio cache manager for active ceremony presets.
//!
//! Stores the active preset audio blocks in the dedicated `audio_cache`
//! partition (offset 0x190000, size 2.4 MB) when no MicroSD card is present.

use esp_println::println;
use esp_rom_sys::rom::spiflash::esp_rom_spiflash_read;
use khvylyna_core::PresetBlock;

use crate::wav::WavHeader;

/// Base offset of the audio_cache partition in Flash (from partitions.csv).
pub const AUDIO_CACHE_OFFSET: u32 = 0x190000;

/// Magic identifier for valid audio cache header.
pub const CACHE_MAGIC: &[u8; 6] = b"KCACHE";

/// Descriptor for a single cached audio block.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CachedBlockEntry {
    pub block_id: u8,
    pub _reserved: [u8; 3],
    pub flash_offset: u32,
    pub length: u32,
}

/// Reader and manager for the internal Flash audio cache partition.
pub struct FlashAudioCache;

impl FlashAudioCache {
    /// Creates a new instance of the internal Flash audio cache.
    pub const fn new() -> Self {
        Self
    }

    /// Reads arbitrary bytes from raw SPI Flash via ROM driver.
    fn read_flash(&self, addr: u32, buf: &mut [u8]) -> bool {
        let len = buf.len() as u32;
        let res = unsafe { esp_rom_spiflash_read(addr, buf.as_mut_ptr() as *const u32, len) };
        res == 0
    }

    /// Converts a PresetBlock into an internal numerical ID.
    pub fn block_to_id(block: &PresetBlock) -> u8 {
        match block {
            PresetBlock::Announcement => 1,
            PresetBlock::Bell => 2,
            PresetBlock::Metronome => 3,
            PresetBlock::Silence => 4,
            PresetBlock::Ending => 5,
            PresetBlock::Anthem => 6,
        }
    }

    /// Streams raw PCM audio chunks for a given block from the internal Flash cache.
    pub fn stream_block_pcm<F>(&mut self, block: &PresetBlock, mut chunk_consumer: F) -> bool
    where
        F: FnMut(&[u8], &WavHeader),
    {
        let target_id = Self::block_to_id(block);
        let mut header_buf = [0u8; 16];

        if !self.read_flash(AUDIO_CACHE_OFFSET, &mut header_buf) {
            return false;
        }

        if &header_buf[0..6] != CACHE_MAGIC {
            // Flash partition does not contain a valid audio cache
            return false;
        }

        let num_entries = u16::from_le_bytes([header_buf[6], header_buf[7]]) as usize;
        let mut matching_entry: Option<CachedBlockEntry> = None;

        // Table of entries follows 16-byte header, 12 bytes each
        let mut entry_buf = [0u8; 12];
        for i in 0..num_entries.min(8) {
            let entry_offset = AUDIO_CACHE_OFFSET + 16 + (i as u32 * 12);
            if self.read_flash(entry_offset, &mut entry_buf) {
                let block_id = entry_buf[0];
                let flash_offset =
                    u32::from_le_bytes([entry_buf[4], entry_buf[5], entry_buf[6], entry_buf[7]]);
                let length =
                    u32::from_le_bytes([entry_buf[8], entry_buf[9], entry_buf[10], entry_buf[11]]);

                if block_id == target_id {
                    matching_entry = Some(CachedBlockEntry {
                        block_id,
                        _reserved: [0; 3],
                        flash_offset,
                        length,
                    });
                    break;
                }
            }
        }

        let Some(entry) = matching_entry else {
            return false;
        };

        if entry.length < 44 {
            return false;
        }

        let mut wav_header_buf = [0u8; 44];
        if !self.read_flash(entry.flash_offset, &mut wav_header_buf) {
            return false;
        }

        let Some(wav_header) = WavHeader::parse(&wav_header_buf) else {
            return false;
        };

        println!(
            "[Flash] Streaming cached block #{}: {} Hz, {}-bit, {} ch, {} bytes",
            entry.block_id,
            wav_header.sample_rate,
            wav_header.bits_per_sample,
            wav_header.channels,
            wav_header.data_length
        );

        let mut data_buf = [0u8; 512];
        let mut bytes_left = wav_header.data_length as usize;
        let mut current_offset = entry.flash_offset + 44;

        while bytes_left > 0 {
            let to_read = data_buf.len().min(bytes_left);
            if !self.read_flash(current_offset, &mut data_buf[..to_read]) {
                println!("[Flash] Read error during audio streaming");
                break;
            }

            chunk_consumer(&data_buf[..to_read], &wav_header);
            current_offset += to_read as u32;
            bytes_left = bytes_left.saturating_sub(to_read);
        }

        true
    }
}
