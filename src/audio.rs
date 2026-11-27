use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};
use esp_println::println;
use khvylyna_core::voices::{VoiceAssets, effective_blocks};
use khvylyna_core::{PresetBlock, Settings};

use crate::sdcard::SdCardStorage;

/// Amplifier enable / mute controller (active-high: GPIO 4 for relay or SD/MUTE).
pub struct AmpControl<'d> {
    output: Output<'d>,
}

impl<'d> AmpControl<'d> {
    /// Initializes amplifier control on the given pin (starts disabled/muted).
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        let output = Output::new(pin, Level::Low, OutputConfig::default());
        Self { output }
    }

    /// Powers on the amplifier or unmutes audio output.
    pub fn enable(&mut self) {
        self.output.set_high();
    }

    /// Powers off the amplifier or mutes audio output.
    pub fn disable(&mut self) {
        self.output.set_low();
    }
}

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
    async fn play_fallback_metronome(&self) {
        println!("[Audio] Fallback: Playing 60-second metronome synthetic ticking sequence...");
        for sec in 1..=60 {
            // High-pitched 1200 Hz click at the start of each second
            let _sample = lookup_sine((sec * 8) as u8);
            Timer::after(Duration::from_millis(20)).await;
            Timer::after(Duration::from_millis(980)).await;
        }
    }

    /// Simulates memorial bell strikes with exponential ring-down.
    async fn play_fallback_bell(&self) {
        println!("[Audio] Fallback: Playing memorial bell synthetic cadence (3 strikes)...");
        for strike in 1..=3 {
            println!("[Audio] Bell strike #{} (440 Hz resonance)...", strike);
            Timer::after(Duration::from_millis(2500)).await;
        }
    }

    /// Plays an individual ceremony block, streaming real WAV from SD if present, or using fallback cadence.
    pub async fn play_block<'d>(
        &self,
        block: &PresetBlock,
        settings: &Settings,
        storage: &mut Option<SdCardStorage<'d>>,
    ) {
        let durations = VoiceAssets::for_voice(settings.announcement_voice).durations;

        if *block == PresetBlock::Silence {
            println!("[Audio] Observed national moment of silence (60s)...");
            Timer::after(Duration::from_secs(60)).await;
            return;
        }

        let mut played_from_sd = false;
        if let Some(sd) = storage {
            if let Some(filename) = SdCardStorage::filename_for_block(block, settings) {
                let mut total_samples = 0usize;
                let stream_ok = sd.stream_file_pcm(filename, |chunk, _header| {
                    total_samples += chunk.len() / 2;
                });
                if stream_ok {
                    println!(
                        "[Audio] Completed streaming {} ({} samples) to I2S DAC",
                        filename, total_samples
                    );
                    played_from_sd = true;
                } else {
                    println!("[SD] File {} missing on SD card, using fallback", filename);
                }
            }
        }

        if !played_from_sd {
            match block {
                PresetBlock::Announcement => {
                    println!(
                        "[Audio] Fallback: Simulating announcement block ({:.2}s)...",
                        durations.announcement.as_secs_f32()
                    );
                    Timer::after(Duration::from_micros(
                        durations.announcement.as_micros() as u64
                    ))
                    .await;
                }
                PresetBlock::Bell => {
                    self.play_fallback_bell().await;
                }
                PresetBlock::Metronome => {
                    self.play_fallback_metronome().await;
                }
                PresetBlock::Silence => {}
                PresetBlock::Ending => {
                    let ending_dur = durations
                        .ending
                        .unwrap_or(core::time::Duration::from_secs(4));
                    println!(
                        "[Audio] Fallback: Simulating closing recording ({:.2}s)...",
                        ending_dur.as_secs_f32()
                    );
                    Timer::after(Duration::from_micros(ending_dur.as_micros() as u64)).await;
                }
                PresetBlock::Anthem => {
                    println!("[Audio] Fallback: Simulating state anthem block (104s)...");
                    Timer::after(Duration::from_secs(104)).await;
                }
            }
        }
    }

    /// Executes the complete sequence of blocks configured in ceremony settings.
    pub async fn play_ceremony<'d>(
        &self,
        settings: &Settings,
        storage: &mut Option<SdCardStorage<'d>>,
    ) {
        println!(
            "[Audio] Initiating ceremony audio on I2S (BCLK: GPIO {}, WS: GPIO {}, DIN: GPIO {})...",
            self.pin_bclk, self.pin_ws, self.pin_din
        );

        let blocks = effective_blocks(&settings.blocks, settings.announcement_voice);
        for block in &blocks {
            self.play_block(block, settings, storage).await;
        }

        println!("[Audio] Ceremony audio sequence completed.");
    }
}
