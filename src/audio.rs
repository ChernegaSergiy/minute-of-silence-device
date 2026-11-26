use embassy_time::{Duration, Timer};
use esp_println::println;
use khvylyna_core::{CeremonyBlock, Settings};

/// Hardware audio interface controller for MAX98357A I2S DAC amplifier.
pub struct AudioPlayer {
    pub pin_bclk: u8,
    pub pin_ws: u8,
    pub pin_din: u8,
}

impl AudioPlayer {
    /// Creates a new AudioPlayer instance with specified GPIO pins.
    pub const fn new(pin_bclk: u8, pin_ws: u8, pin_din: u8) -> Self {
        Self {
            pin_bclk,
            pin_ws,
            pin_din,
        }
    }

    /// Plays an individual ceremony block according to configuration.
    pub async fn play_block(&self, block: &CeremonyBlock, _settings: &Settings) {
        match block {
            CeremonyBlock::Announcement => {
                println!("[Audio] Playing memorial announcement block...");
                Timer::after(Duration::from_secs(5)).await;
            }
            CeremonyBlock::Metronome => {
                println!("[Audio] Playing metronome ticking cadence...");
                Timer::after(Duration::from_secs(60)).await;
            }
            CeremonyBlock::Silence => {
                println!("[Audio] Observed national moment of silence...");
                Timer::after(Duration::from_secs(60)).await;
            }
            CeremonyBlock::Bell => {
                println!("[Audio] Playing memorial bell cadence...");
                Timer::after(Duration::from_secs(10)).await;
            }
            CeremonyBlock::Anthem => {
                println!("[Audio] Playing state anthem block...");
                Timer::after(Duration::from_secs(90)).await;
            }
        }
    }

    /// Executes the complete sequence of blocks configured in ceremony settings.
    pub async fn play_ceremony(&self, settings: &Settings) {
        println!(
            "[Audio] Initiating ceremony audio on I2S (BCLK: GPIO {}, WS: GPIO {}, DIN: GPIO {})...",
            self.pin_bclk, self.pin_ws, self.pin_din
        );

        for block in &settings.blocks {
            self.play_block(block, settings).await;
        }

        println!("[Audio] Ceremony audio sequence completed.");
    }
}
