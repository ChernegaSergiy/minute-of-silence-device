#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use core::net::{IpAddr, SocketAddr};
use embassy_executor::Spawner;
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Runner, StackResources};
use embassy_time::{Duration, Timer};
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::main;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_radio::wifi::{Config, ControllerConfig, Interface, WifiController, sta::StationConfig};
use khvylyna_core::Settings;
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;

const USEC_IN_SEC: u64 = 1_000_000;

#[derive(Clone, Copy)]
struct Timestamp<'a> {
    rtc: &'a Rtc<'a>,
    current_time_us: u64,
}

impl NtpTimestampGenerator for Timestamp<'_> {
    fn init(&mut self) {
        self.current_time_us = self.rtc.current_time_us();
    }

    fn timestamp_sec(&self) -> u64 {
        self.current_time_us / USEC_IN_SEC
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.current_time_us % USEC_IN_SEC) as u32
    }
}

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
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let rtc = Rtc::new(peripherals.RTC_TIMER);

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

    spawner.spawn(connection_task(controller).unwrap());
    spawner.spawn(net_task(runner).unwrap());

    println!("Waiting for network IP assignment...");
    stack.wait_config_up().await;
    if let Some(cfg) = stack.config_v4() {
        println!("Network ready! IP: {}", cfg.address);
    }

    println!("Synchronizing time via SNTP...");
    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buffer = [0u8; 512];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buffer = [0u8; 512];

    if let Ok(addrs) = stack
        .dns_query(settings.ntp_server.as_str(), DnsQueryType::A)
        .await
    {
        if let Some(&first_addr) = addrs.first() {
            let addr: IpAddr = first_addr.into();
            let mut socket = UdpSocket::new(
                stack,
                &mut rx_meta,
                &mut rx_buffer,
                &mut tx_meta,
                &mut tx_buffer,
            );
            if socket.bind(123).is_ok() {
                let socket_wrapper = UdpSocketWrapper::new(socket);
                let context = NtpContext::new(Timestamp {
                    rtc: &rtc,
                    current_time_us: 0,
                });
                match get_time(SocketAddr::from((addr, 123)), &socket_wrapper, context).await {
                    Ok(time) => {
                        let epoch_us = (time.sec() as u64 * USEC_IN_SEC)
                            + ((time.sec_fraction() as u64 * USEC_IN_SEC) >> 32);
                        rtc.set_current_time_us(epoch_us);
                        println!("NTP synchronized! Epoch: {} s", time.sec());
                    }
                    Err(_) => {
                        println!("Failed to receive NTP response");
                    }
                }
            }
        }
    }

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}
