#![no_std]
#![no_main]

extern crate alloc;

mod audio;
mod button;
mod led;
mod scheduler;
mod sdcard;
mod sntp;
mod wifi;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::main;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use khvylyna_core::Settings;
use khvylyna_core::voices::VoiceAssets;

#[main]
async fn main(spawner: Spawner) -> ! {
    heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let rtc = Rtc::new(peripherals.RTC_TIMER);
    let mut status_led = led::StatusLed::new(peripherals.GPIO2);
    let mut test_button = button::Button::new(peripherals.GPIO0);
    let mut amp_control = audio::AmpControl::new(peripherals.GPIO4);
    status_led.on();

    println!("Initializing Minute of Silence device...");

    // MicroSD Card initialization over SPI (SCK: GPIO 18, MISO: GPIO 19, MOSI: GPIO 23, CS: GPIO 5)
    let spi_bus = match Spi::new(peripherals.SPI2, SpiConfig::default()) {
        Ok(spi) => Some(
            spi.with_sck(peripherals.GPIO18)
                .with_mosi(peripherals.GPIO23)
                .with_miso(peripherals.GPIO19),
        ),
        Err(_) => {
            println!("[SPI] Failed to configure SPI2 master bus");
            None
        }
    };

    let sd_cs = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let delay = Delay::new();

    let mut sd_storage = if let Some(spi) = spi_bus {
        match ExclusiveDevice::new(spi, sd_cs, delay) {
            Ok(spi_device) => match sdcard::SdCardStorage::new(spi_device, delay) {
                Ok(storage) => Some(storage),
                Err(err) => {
                    println!("[SD] MicroSD initialization skipped: {}", err);
                    None
                }
            },
            Err(_) => {
                println!("[SD] Failed to create exclusive SPI device for MicroSD");
                None
            }
        }
    } else {
        None
    };

    let mut settings = Settings::default();
    settings.migrate();
    if settings.validate().is_err() {
        println!("Configuration validation failed, fallback to default");
        settings = Settings::default();
    }

    println!("Device ready. NTP server: {}", settings.ntp_server.as_str());

    let stack = wifi::init_wifi(peripherals.WIFI, &spawner);

    println!("Waiting for network IP assignment...");
    stack.wait_config_up().await;
    if let Some(cfg) = stack.config_v4() {
        println!("Network ready! IP: {}", cfg.address);
    }

    sntp::synchronize_time(&stack, &rtc, settings.ntp_server.as_str()).await;

    let mut daily_scheduler = scheduler::DailyScheduler::new();
    let audio_player = audio::AudioPlayer::new(26, 25, 22);

    println!("Scheduler active. Monitoring 09:00:00 trigger or GPIO 0 button press...");

    let voice_assets = VoiceAssets::for_voice(settings.announcement_voice);

    loop {
        let current_epoch_secs = rtc.current_time_us() / 1_000_000;
        let scheduled =
            daily_scheduler.should_trigger(current_epoch_secs, &settings, &voice_assets.durations);
        let manual_test = test_button.is_clicked();

        if scheduled || manual_test {
            if manual_test {
                println!("Manual test triggered via hardware button!");
            } else {
                println!("Starting daily Minute of Silence ceremony...");
            }
            status_led.toggle();
            amp_control.enable();
            Timer::after(Duration::from_millis(200)).await;

            audio_player.play_ceremony(&settings, &mut sd_storage).await;

            Timer::after(Duration::from_millis(500)).await;
            amp_control.disable();
            status_led.on();
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}
