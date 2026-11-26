#![no_std]
#![no_main]

extern crate alloc;

mod audio;
mod button;
mod led;
mod scheduler;
mod sntp;
mod wifi;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::main;
use esp_hal::rtc_cntl::Rtc;
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
    status_led.on();

    println!("Initializing Minute of Silence device...");

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
            audio_player.play_ceremony(&settings).await;
            status_led.on();
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}
