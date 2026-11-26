use embassy_time::{Duration, Timer};
use esp_println::println;
use khvylyna_core::voices::{VoiceAssets, effective_blocks};
use khvylyna_core::{PresetBlock, Settings};

/// Precomputed quarter-wave sine lookup table (64 entries, 16-bit amplitude).
const SINE_QUARTER: [i16; 64] = [
    0, 804, 1607, 2410, 3211, 4011, 4807, 5601, 6392, 7179, 7961, 8739, 9511, 10278, 11038, 11792,
    12539, 13278, 14009, 14732, 15446, 16150, 16845, 17530, 18204, 18867, 19519, 20159, 20787,
    21402, 22004, 22594, 23169, 23731, 24278, 24811, 25329, 25831, 26318, 26789, 27244, 27683,
    28105, 28510, 28897, 29268, 29621, 29955, 30272, 30571, 30851, 31113, 31356, 31580, 31785,
    31970, 32137, 32284, 32412, 32520, 32609, 32678, 32727, 32757,
];

/// Returns 16-bit sine wave amplitude for a given phase in 0..256.
pub fn lookup_sine(phase: u8) -> i16 {
    let quad = phase >> 6;
    let idx = (phase & 0x3F) as usize;
    match quad {
        0 => SINE_QUARTER[idx],
        1 => SINE_QUARTER[63 - idx],
        2 => -SINE_QUARTER[idx],
        _ => -SINE_QUARTER[63 - idx],
    }
}

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

    /// Simulates metronome ticking cadence (60 distinct 1-second cadence pulses).
    async fn play_metronome_cadence(&self) {
        println!("[Audio] Playing 60-second metronome ticking sequence...");
        for sec in 1..=60 {
            // High-pitched 1200 Hz click at the start of each second
            let _sample = lookup_sine((sec * 8) as u8);
            Timer::after(Duration::from_millis(20)).await;
            Timer::after(Duration::from_millis(980)).await;
        }
    }

    /// Simulates memorial bell strikes with exponential ring-down.
    async fn play_bell_strikes(&self) {
        println!("[Audio] Playing memorial bell cadence (3 strikes)...");
        for strike in 1..=3 {
            println!("[Audio] Bell strike #{} (440 Hz resonance)...", strike);
            Timer::after(Duration::from_millis(2500)).await;
        }
    }

    /// Plays an individual ceremony block according to configuration and core voice manifest.
    pub async fn play_block(&self, block: &PresetBlock, settings: &Settings) {
        let durations = VoiceAssets::for_voice(settings.announcement_voice).durations;
        match block {
            PresetBlock::Announcement => {
                println!(
                    "[Audio] Playing memorial announcement block ({:.2}s)...",
                    durations.announcement.as_secs_f32()
                );
                Timer::after(Duration::from_micros(
                    durations.announcement.as_micros() as u64
                ))
                .await;
            }
            PresetBlock::Bell => {
                println!(
                    "[Audio] Playing memorial bell block ({:.2}s)...",
                    durations.bell.as_secs_f32()
                );
                self.play_bell_strikes().await;
            }
            PresetBlock::Metronome => {
                self.play_metronome_cadence().await;
            }
            PresetBlock::Silence => {
                println!("[Audio] Observed national moment of silence (60s)...");
                Timer::after(Duration::from_secs(60)).await;
            }
            PresetBlock::Ending => {
                let ending_dur = durations
                    .ending
                    .unwrap_or(core::time::Duration::from_secs(4));
                println!(
                    "[Audio] Playing memorial closing recording ({:.2}s)...",
                    ending_dur.as_secs_f32()
                );
                Timer::after(Duration::from_micros(ending_dur.as_micros() as u64)).await;
            }
            PresetBlock::Anthem => {
                println!("[Audio] Playing state anthem block (104s)...");
                Timer::after(Duration::from_secs(104)).await;
            }
        }
    }

    /// Executes the complete sequence of blocks configured in ceremony settings.
    pub async fn play_ceremony(&self, settings: &Settings) {
        println!(
            "[Audio] Initiating ceremony audio on I2S (BCLK: GPIO {}, WS: GPIO {}, DIN: GPIO {})...",
            self.pin_bclk, self.pin_ws, self.pin_din
        );

        let blocks = effective_blocks(&settings.blocks, settings.announcement_voice);
        for block in &blocks {
            self.play_block(block, settings).await;
        }

        println!("[Audio] Ceremony audio sequence completed.");
    }
}
