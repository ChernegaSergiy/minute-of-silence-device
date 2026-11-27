//! MicroSD card reader driver and WAV stream parser.

use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{
    Error as SdError, File, Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager,
};
use esp_hal::delay::Delay;
use esp_hal::gpio::Output;
use esp_hal::spi::master::Spi;
use esp_println::println;
use khvylyna_core::models::presets::{AnnouncementVoice, AnthemVoice};
use khvylyna_core::{PresetBlock, Settings};

/// Dummy time source for FAT filesystem operations in no_std.
pub struct StaticTimeSource;

impl TimeSource for StaticTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp::from_calendar(2026, 1, 1, 0, 0, 0).unwrap_or(Timestamp {
            year_since_1970: 56,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        })
    }
}

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

/// Resolves standard 8.3 FAT filenames for ceremony audio blocks based on configuration.
pub fn filename_for_block(block: &PresetBlock, settings: &Settings) -> Option<&'static str> {
    match block {
        PresetBlock::Announcement => match settings.announcement_voice {
            AnnouncementVoice::BohdanHdal => Some("ANN_HDAL.WAV"),
            AnnouncementVoice::SoniaSotnyk => Some("ANN_SOTN.WAV"),
            AnnouncementVoice::DaniaKhomutovskyi => Some("ANN_KHOM.WAV"),
            AnnouncementVoice::RadioBg => Some("ANN_R_BG.WAV"),
            AnnouncementVoice::VinnytsiaCity => Some("ANN_VINN.WAV"),
        },
        PresetBlock::Bell => Some("BELL.WAV"),
        PresetBlock::Metronome => Some("METRONOM.WAV"),
        PresetBlock::Silence => None,
        PresetBlock::Ending => match settings.announcement_voice {
            AnnouncementVoice::BohdanHdal => Some("END_HDAL.WAV"),
            AnnouncementVoice::SoniaSotnyk => Some("END_SOTN.WAV"),
            AnnouncementVoice::DaniaKhomutovskyi => Some("END_KHOM.WAV"),
            AnnouncementVoice::RadioBg => Some("END_R_BG.WAV"),
            AnnouncementVoice::VinnytsiaCity => None,
        },
        PresetBlock::Anthem => match settings.anthem_voice {
            AnthemVoice::Default => Some("ANTHEM.WAV"),
            AnthemVoice::MykhailoKhoma => Some("ANTH_KHO.WAV"),
            AnthemVoice::OleksandrPonomarov => Some("ANTH_PON.WAV"),
        },
    }
}

/// Type alias for the SPI device wrapped with exclusive chip-select control.
pub type SdCardDevice<'d> = ExclusiveDevice<Spi<'d, esp_hal::Blocking>, Output<'d>, Delay>;

/// MicroSD card filesystem manager.
pub struct SdCardStorage<'d> {
    volume_mgr: VolumeManager<SdCard<SdCardDevice<'d>, Delay>, StaticTimeSource>,
}

impl<'d> SdCardStorage<'d> {
    /// Initializes SD card and mounts the first FAT volume.
    pub fn new(spi_device: SdCardDevice<'d>, delay: Delay) -> Result<Self, &'static str> {
        let sd_card = SdCard::new(spi_device, delay);
        println!("[SD] Probing MicroSD card...");

        let mut volume_mgr = VolumeManager::new(sd_card, StaticTimeSource);
        let volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|_| "Failed to open FAT volume on MicroSD card")?;

        println!("[SD] Volume mounted successfully. Ready for audio playback.");
        let _ = volume;

        Ok(Self { volume_mgr })
    }
}
