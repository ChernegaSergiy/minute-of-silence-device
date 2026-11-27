use alloc::boxed::Box;
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::rng::Rng;
use esp_println::println;
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config, ControllerConfig, Interface, WifiController,
    sta::StationConfig,
};

const DEFAULT_SSID: &str = match option_env!("WIFI_SSID") {
    Some(val) => val,
    None => "MinuteOfSilence",
};

const DEFAULT_PASSWORD: &str = match option_env!("WIFI_PASS") {
    Some(val) => val,
    None => "",
};

#[embassy_executor::task]
pub async fn connection_task(mut controller: WifiController<'static>) {
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
pub async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await;
}

pub fn init_wifi(
    wifi_peripheral: esp_hal::peripherals::WIFI<'static>,
    spawner: &Spawner,
) -> Stack<'static> {
    let mut station_config = StationConfig::default();

    if let Ok(ssid) = DEFAULT_SSID.try_into() {
        station_config = station_config.with_ssid(ssid);
    }

    if !DEFAULT_PASSWORD.is_empty() {
        if let Ok(pass) = DEFAULT_PASSWORD.try_into() {
            station_config =
                station_config.with_authentication(AuthenticationMethodConfig::Wpa2Personal(pass));
        }
    }

    let wifi_interface = esp_radio::wifi::Interface::station();
    let controller = esp_radio::wifi::WifiController::new(
        wifi_peripheral,
        ControllerConfig::default().with_initial_config(Config::Station(station_config)),
    )
    .unwrap();

    let net_config = embassy_net::Config::dhcpv4(Default::default());
    let rng = Rng::new();
    let seed = ((rng.random() as u64) << 32) | (rng.random() as u64);

    let stack_resources = Box::leak(Box::new(StackResources::<6>::new()));
    let (stack, runner) = embassy_net::new(wifi_interface, net_config, stack_resources, seed);

    spawner.spawn(connection_task(controller).unwrap());
    spawner.spawn(net_task(runner).unwrap());

    stack
}
