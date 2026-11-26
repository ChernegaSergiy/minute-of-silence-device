#![no_std]
#![no_main]

extern crate alloc;

use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::entry;
use esp_println::println;
use khvylyna_core::Settings;

#[entry]
fn main() -> ! {
    // Initialize 64 KB heap for alloc (Vec, String)
    heap_allocator!(size: 64 * 1024);

    let _peripherals = esp_hal::init(esp_hal::Config::default());

    println!("Initializing Minute of Silence device...");

    let mut settings = Settings::default();
    settings.migrate();
    if settings.validate().is_err() {
        println!("Configuration validation failed, fallback to default");
        settings = Settings::default();
    }

    println!("Device ready. NTP server: {}", settings.ntp_server.as_str());
    println!("Ceremony enabled: {}", settings.ceremony_enabled);

    loop {}
}
