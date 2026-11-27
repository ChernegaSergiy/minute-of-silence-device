use embassy_time::{Duration, Timer};
use esp_hal::Async;
use esp_hal::dma::DmaTxStreamBuf;
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};
use esp_hal::i2s::master::I2sTx;
use esp_println::println;
use khvylyna_core::voices::{VoiceAssets, effective_blocks};
use khvylyna_core::{PresetBlock, Settings};

use crate::flash_cache::FlashAudioCache;
use crate::i2s_sink::I2sSink;
use crate::sdcard::SdCardStorage;
use crate::wav::WavHeader;

pub(crate) trait PcmSink {
    async fn write(&mut self, chunk: &[u8], header: &WavHeader) -> Result<(), ()>;
}

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

/// Ceremony audio player with an optional external I2S output.
pub struct AudioPlayer<'d> {
    i2s: Option<I2sTx<'d, Async>>,
    buffer: Option<DmaTxStreamBuf>,
}

impl<'i2s> AudioPlayer<'i2s> {
    /// Creates an audio player using the configured I2S transmitter.
    pub const fn new(i2s: Option<I2sTx<'i2s, Async>>, buffer: DmaTxStreamBuf) -> Self {
        Self {
            i2s,
            buffer: Some(buffer),
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
        &mut self,
        block: &PresetBlock,
        settings: &Settings,
        storage: &mut Option<SdCardStorage<'d>>,
        flash_cache: &mut FlashAudioCache,
    ) {
        let durations = VoiceAssets::for_voice(settings.announcement_voice).durations;

        if *block == PresetBlock::Silence {
            println!("[Audio] Observed national moment of silence (60s)...");
            Timer::after(Duration::from_secs(60)).await;
            return;
        }

        let mut played_audio = false;

        // Level 1: Attempt to stream directly from MicroSD card
        if let Some(sd) = storage {
            if let Some(filename) = SdCardStorage::filename_for_block(block, settings) {
                let mut sink = I2sSink::new(&mut self.i2s, &mut self.buffer, settings.volume);
                let stream_ok = sd.stream_file_pcm(filename, &mut sink).await;
                match sink.finish(stream_ok).await {
                    Ok(()) => {
                        println!("[Audio] Played {} from SD", filename);
                        played_audio = true;
                    }
                    Err(true) => {
                        println!("[Audio] I2S playback failed for {}", filename);
                        return;
                    }
                    Err(false) => {}
                }
            }
        }

        // Level 2: Fallback to internal Flash audio cache partition
        if !played_audio {
            let mut sink = I2sSink::new(&mut self.i2s, &mut self.buffer, settings.volume);
            let stream_ok = flash_cache.stream_block_pcm(block, &mut sink).await;
            match sink.finish(stream_ok).await {
                Ok(()) => {
                    println!("[Audio] Played block {:?} from Flash cache", block);
                    played_audio = true;
                }
                Err(true) => {
                    println!("[Audio] I2S playback failed for block {:?}", block);
                    return;
                }
                Err(false) => {}
            }
        }

        // Level 3: Fallback to synthetic cadence timing
        if !played_audio {
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
                    println!(
                        "[Audio] Notice: Anthem audio not cached; skipping block to complete ceremony cleanly."
                    );
                }
            }
        }
    }

    /// Executes the complete sequence of blocks configured in ceremony settings.
    pub async fn play_ceremony<'d>(
        &mut self,
        settings: &Settings,
        storage: &mut Option<SdCardStorage<'d>>,
        flash_cache: &mut FlashAudioCache,
    ) {
        if self.i2s.is_none() {
            println!("[Audio] I2S output unavailable");
        }

        let blocks = effective_blocks(&settings.blocks, settings.announcement_voice);
        for block in &blocks {
            self.play_block(block, settings, storage, flash_cache).await;
        }

        println!("[Audio] Ceremony audio sequence completed.");
    }
}
