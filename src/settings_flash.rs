//! Settings flash access backed by the ESP32 SPI flash driver.

use esp_storage::FlashStorage;

use crate::settings_store::SettingsFlash;

impl SettingsFlash for FlashStorage<'_> {
    fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), ()> {
        self.read_nor(offset, buf).map_err(|_| ())
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), ()> {
        FlashStorage::erase(self, from, to).map_err(|_| ())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ()> {
        self.write_nor(offset, bytes).map_err(|_| ())
    }
}
