//! MicroSD card reader driver and WAV stream parser.

use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use esp_hal::delay::Delay;
use esp_hal::gpio::Output;
use esp_hal::spi::master::Spi;
use esp_println::println;
use khvylyna_core::models::presets::{AnnouncementVoice, AnthemVoice};
use khvylyna_core::{PresetBlock, Settings};

use crate::{audio::PcmSink, wav::WavHeader};

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
        let raw_volume = volume_mgr
            .open_raw_volume(VolumeIdx(0))
            .map_err(|_| "Failed to open FAT volume on MicroSD card")?;

        println!("[SD] Volume mounted successfully. Ready for audio playback.");
        let _ = volume_mgr.close_volume(raw_volume);

        Ok(Self { volume_mgr })
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

    /// Streams a WAV audio file from the SD card into a PCM sink.
    pub(crate) async fn stream_file_pcm<S: PcmSink>(
        &mut self,
        filename: &str,
        sink: &mut S,
    ) -> bool {
        let raw_volume = match self.volume_mgr.open_raw_volume(VolumeIdx(0)) {
            Ok(v) => v,
            Err(_) => {
                println!("[SD] Error: Unable to open FAT volume");
                return false;
            }
        };

        let raw_root_dir = match self.volume_mgr.open_root_dir(raw_volume) {
            Ok(d) => d,
            Err(_) => {
                println!("[SD] Error: Unable to open root directory");
                let _ = self.volume_mgr.close_volume(raw_volume);
                return false;
            }
        };

        let raw_file =
            match self
                .volume_mgr
                .open_file_in_dir(raw_root_dir, filename, Mode::ReadOnly)
            {
                Ok(f) => f,
                Err(_) => {
                    let _ = self.volume_mgr.close_dir(raw_root_dir);
                    let _ = self.volume_mgr.close_volume(raw_volume);
                    return false;
                }
            };

        let mut header_buf = [0u8; 44];
        let mut header_read = 0;
        while header_read < 44 {
            match self
                .volume_mgr
                .read(raw_file, &mut header_buf[header_read..])
            {
                Ok(0) => break,
                Ok(n) => header_read += n,
                Err(_) => break,
            }
        }

        if header_read < 44 {
            println!(
                "[SD] Error: {} header is too short ({} bytes)",
                filename, header_read
            );
            let _ = self.volume_mgr.close_file(raw_file);
            let _ = self.volume_mgr.close_dir(raw_root_dir);
            let _ = self.volume_mgr.close_volume(raw_volume);
            return false;
        }

        let Some(header) = WavHeader::parse(&header_buf) else {
            println!(
                "[SD] Error: {} has invalid or unsupported WAV format",
                filename
            );
            let _ = self.volume_mgr.close_file(raw_file);
            let _ = self.volume_mgr.close_dir(raw_root_dir);
            let _ = self.volume_mgr.close_volume(raw_volume);
            return false;
        };

        println!(
            "[SD] Streaming {}: {} Hz, {}-bit, {} ch, {} bytes",
            filename,
            header.sample_rate,
            header.bits_per_sample,
            header.channels,
            header.data_length
        );

        let mut data_buf = [0u8; 512];
        let mut bytes_left = header.data_length as usize;

        while bytes_left > 0 {
            let to_read = data_buf.len().min(bytes_left);
            match self.volume_mgr.read(raw_file, &mut data_buf[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    if sink.write(&data_buf[..n], &header).await.is_err() {
                        break;
                    }
                    bytes_left = bytes_left.saturating_sub(n);
                }
                Err(_) => {
                    println!("[SD] Error during streaming read of {}", filename);
                    break;
                }
            }
        }

        let _ = self.volume_mgr.close_file(raw_file);
        let _ = self.volume_mgr.close_dir(raw_root_dir);
        let _ = self.volume_mgr.close_volume(raw_volume);

        bytes_left == 0
    }
}
