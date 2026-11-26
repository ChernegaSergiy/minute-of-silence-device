#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources};
use embassy_time::{Duration, Timer};
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::main;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_radio::wifi::{Config, ControllerConfig, Interface, WifiController, sta::StationConfig};
use khvylyna_core::Settings;

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    println!("Starting Wi-Fi connection task...");
    loop {
        match controller.connect_async().await {
            Ok(_) => {
                println!("Wi-Fi connected successfully");
                let _ = controller.wait_for_disconnect_async().await;
                println!("Wi-Fi disconnected, reconnecting...");
            }
            Err(_) => {
                println!("Wi-Fi connection failed, retrying in 5s...");
            }
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await;
}

#[main]
async fn main(spawner: Spawner) -> ! {
    heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    println!("Initializing Minute of Silence device...");

    let mut settings = Settings::default();
    settings.migrate();
    if settings.validate().is_err() {
        println!("Configuration validation failed, fallback to default");
        settings = Settings::default();
    }

    println!("Device ready. NTP server: {}", settings.ntp_server.as_str());

    let station_config = Config::Station(StationConfig::default());

    let wifi_interface = esp_radio::wifi::Interface::station();
    let controller = esp_radio::wifi::WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(station_config),
    )
    .unwrap();

    let net_config = embassy_net::Config::dhcpv4(Default::default());
    let rng = Rng::new();
    let seed = ((rng.random() as u64) << 32) | (rng.random() as u64);

    let stack_resources = Box::leak(Box::new(StackResources::<4>::new()));
    let (stack, runner) = embassy_net::new(wifi_interface, net_config, stack_resources, seed);

    spawner.spawn(connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    println!("Waiting for network IP assignment...");
    stack.wait_config_up().await;
    if let Some(cfg) = stack.config_v4() {
        println!("Network ready! IP: {}", cfg.address);
    }

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}
