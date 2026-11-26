use alloc::boxed::Box;
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::rng::Rng;
use esp_println::println;
use esp_radio::wifi::{Config, ControllerConfig, Interface, WifiController, sta::StationConfig};

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
    let station_config = Config::Station(StationConfig::default());
    let wifi_interface = esp_radio::wifi::Interface::station();
    let controller = esp_radio::wifi::WifiController::new(
        wifi_peripheral,
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

    stack
}
