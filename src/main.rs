#![no_std]
#![no_main]

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

mod audio;
mod audio_timing;
mod button;
mod flash_cache;
mod flash_layout;
mod http_server;
mod i2s_sink;
mod led;
mod led_pattern;
mod mdns;
mod pcm;
mod scheduler;
mod sdcard;
mod settings_flash;
mod settings_record;
mod settings_store;
mod sntp;
mod transfer;
mod wav;
mod wifi;

use embassy_executor::Spawner;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpEndpoint, Ipv4Address, Stack};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::dma_tx_stream_buffer;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2s::master::{Channels, DataFormat, I2s, TdmConfig};
use esp_hal::main;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use khvylyna_core::Settings;
use khvylyna_core::voices::VoiceAssets;

#[embassy_executor::task]
pub async fn mdns_task(stack: Stack<'static>, local_ip: Ipv4Address) {
    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buffer = [0u8; 1024];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buffer = [0u8; 1024];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    if let Err(e) = socket.bind(mdns::MDNS_PORT) {
        println!("[mDNS] Failed to bind port 5353: {:?}", e);
        return;
    }

    if let Err(e) = stack.join_multicast_group(mdns::MDNS_MCAST_IPV4) {
        println!("[mDNS] Warning: failed to join multicast group: {:?}", e);
    } else {
        println!(
            "[mDNS] Active on 224.0.0.251:5353. Advertising {} and {}.local -> {}",
            mdns::SERVICE_TYPE,
            mdns::MDNS_HOSTNAME,
            local_ip
        );
    }

    let mut buf = [0u8; 512];
    let mut resp = [0u8; 512];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((n, remote)) => {
                let packet = &buf[..n];
                let dest = if remote.endpoint.port != mdns::MDNS_PORT {
                    remote.endpoint
                } else {
                    IpEndpoint::new(mdns::MDNS_MCAST_IPV4.into(), mdns::MDNS_PORT)
                };

                if mdns::is_query_for_service(packet) {
                    if let Some(resp_len) = mdns::build_dnssd_response(local_ip, &mut resp) {
                        let _ = socket.send_to(&resp[..resp_len], dest).await;
                        println!(
                            "[mDNS] Responded to DNS-SD query from {:?}",
                            remote.endpoint
                        );
                    }
                } else if mdns::is_query_for_hostname(packet) {
                    if let Some(resp_len) = mdns::build_a_response(local_ip, &mut resp) {
                        let _ = socket.send_to(&resp[..resp_len], dest).await;
                        println!("[mDNS] Responded to A query from {:?}", remote.endpoint);
                    }
                }
            }
            Err(_) => {
                Timer::after(Duration::from_millis(100)).await;
            }
        }
    }
}

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

    let (stack, pairing_seed) = wifi::init_wifi(peripherals.WIFI, &spawner);

    println!("Waiting for network IP assignment...");
    stack.wait_config_up().await;
    if let Some(cfg) = stack.config_v4() {
        println!("Network ready! IP: {}", cfg.address);
        spawner.spawn(mdns_task(stack, cfg.address.address()).unwrap());
        spawner.spawn(http_server::pairing_task(stack, pairing_seed).unwrap());
    }

    sntp::synchronize_time(&stack, &rtc, settings.ntp_server.as_str()).await;

    let mut daily_scheduler = scheduler::DailyScheduler::new();
    let i2s = match I2s::new(
        peripherals.I2S0,
        peripherals.DMA_I2S0,
        TdmConfig::new_tdm_philips()
            .with_sample_rate(Rate::from_hz(44100))
            .with_data_format(DataFormat::Data16Channel16)
            .with_channels(Channels::STEREO),
    ) {
        Ok(i2s) => Some(
            i2s.into_async()
                .i2s_tx
                .with_bclk(peripherals.GPIO26)
                .with_ws(peripherals.GPIO25)
                .with_dout(peripherals.GPIO22)
                .build(),
        ),
        Err(error) => {
            println!("[Audio] Failed to initialize I2S: {:?}", error);
            None
        }
    };
    let mut audio_player = audio::AudioPlayer::new(i2s, dma_tx_stream_buffer!(8192, 512));
    let mut flash_cache = flash_cache::FlashAudioCache::new();

    println!("Scheduler active. Monitoring 09:00:00 trigger or GPIO 0 button press...");

    let mut voice_assets = VoiceAssets::for_voice(settings.announcement_voice);
    let mut code_display = led_pattern::CodeDisplay::default();

    loop {
        if let Some(updated) = http_server::take_updated_settings() {
            settings = updated;
            voice_assets = VoiceAssets::for_voice(settings.announcement_voice);
            println!("Active settings updated via secure pairing.");
        }

        let now_ms = Instant::now().as_millis() as u64;
        let button_event = test_button.update(now_ms);
        if matches!(button_event, button::ButtonEvent::Pairing) {
            println!("Pairing window requested via long button press.");
        }

        led::show_code(
            &mut status_led,
            &mut code_display,
            http_server::pairing_code(),
            now_ms,
        );

        let current_epoch_secs = rtc.current_time_us() / 1_000_000;
        let scheduled =
            daily_scheduler.should_trigger(current_epoch_secs, &settings, &voice_assets.durations);
        let manual_test = matches!(button_event, button::ButtonEvent::Click);

        if scheduled || manual_test {
            if manual_test {
                println!("Manual test triggered via hardware button!");
            } else {
                println!("Starting daily Minute of Silence ceremony...");
            }
            status_led.toggle();
            amp_control.enable();
            Timer::after(Duration::from_millis(200)).await;

            audio_player
                .play_ceremony(&settings, &mut sd_storage, &mut flash_cache)
                .await;

            Timer::after(Duration::from_millis(500)).await;
            amp_control.disable();
            status_led.on();
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}
