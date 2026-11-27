[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/banner-direct-single.svg)](https://stand-with-ukraine.pp.ua)

# Minute of Silence Device

[![CI](https://github.com/ChernegaSergiy/minute-of-silence-device/actions/workflows/ci.yml/badge.svg)](https://github.com/ChernegaSergiy/minute-of-silence-device/actions)
[![License: CSSM Unlimited License v2.0](https://img.shields.io/badge/License-CSSM%20Unlimited%20License%20v2.0-blue.svg?logo=opensourceinitiative)](LICENSE)

**Minute of Silence Device** is an autonomous IoT device built with bare-metal Rust (`no_std`) for the **ESP32-WROOM-32** microcontroller. It accurately triggers the daily memorial ceremony at 09:00 using NTP time synchronization, playing audio announcements, metronome, and bell through an I2S digital audio interface.

The firmware reuses the core domain logic and data contracts directly from [khvylyna-core](https://github.com/ChernegaSergiy/minute-of-silence/tree/main/crates/khvylyna-core), sharing the exact same ceremony configuration, block sequencing, and validation rules as the desktop application.

## Hardware Specifications

| Component | Recommendation | Description |
|---|---|---|
| **MCU** | ESP32-WROOM-32 | Dual-core Xtensa LX6, 4 MB Flash |
| **Audio Storage** | MicroSD Card Module (SPI) | FAT32 formatted card with uncompressed WAV audio blocks |
| **Audio DAC / Amp** | MAX98357A / PCM5102A | Modular I2S audio interface (compact amp or line-out DAC) |
| **Speaker** | 4–8 Ω, 3 W / PA Horn | 40–50 mm full-range driver or outdoor horn loudspeaker |
| **Status LED** | On-board / External LED | Active-high status indicator |
| **Test Button** | Tactile switch / BOOT button | Active-low button with internal pull-up |
| **Amp Control** | External relay / MUTE pin | Active-high power/mute control for external amplifiers |
| **Power** | 5 V USB / 12–24 V DC | 5 V Micro-USB / USB-C, or 12–24 V DC with buck converter |

### Wiring

#### I2S Audio Bus

The I2S interface uses the standard Philips format across all audio profiles:

| Audio Pin | ESP32 GPIO | Function |
|---|---|---|
| `LRC` / `WS` | `GPIO 25` | Word Select (Left/Right clock) |
| `BCLK` | `GPIO 26` | Bit Clock |
| `DIN` | `GPIO 22` | Digital Audio Data In |
| `GND` | `GND` | Ground |
| `VIN` | `5V` / `VBUS` | Power supply (5 V recommended) |

#### MicroSD Card Bus (SPI)

The MicroSD module connects over standard hardware SPI (`SPI2` / HSPI):

| MicroSD Pin | ESP32 GPIO | Function |
|---|---|---|
| `CS` | `GPIO 5` | Chip Select (Active-low) |
| `SCK` / `CLK` | `GPIO 18` | SPI Bus Clock |
| `MISO` / `DO` | `GPIO 19` | SPI Master In / Slave Out |
| `MOSI` / `DI` | `GPIO 23` | SPI Master Out / Slave In |
| `VCC` | `3.3V` / `5V` | Module power supply |
| `GND` | `GND` | Ground |

#### Status LED and Controls

| Peripheral | ESP32 GPIO | Configuration | Description |
|---|---|---|---|
| **Status LED** | `GPIO 2` | Output (Active-high) | Illuminates upon boot and blinks during ceremony |
| **Test Button** | `GPIO 0` | Input (Pull-up, active-low) | Triggers on-demand memorial ceremony for testing (BOOT button) |
| **Amp Control** | `GPIO 4` | Output (Active-high) | Powers on amplifier / unmutes 200 ms before ceremony to eliminate idle hiss |

## Audio Output Profiles

Because the firmware outputs standard I2S digital audio, the hardware easily adapts from individual desktop memorials to public village squares without modifying the code:

| Profile | Output Hardware | Power Source | Use Case |
|---|---|---|---|
| **1. Desktop / Compact** | `MAX98357A` + 3–5 W speaker (4–8 Ω) | 5 V USB | Classrooms, offices, government desks, private rooms |
| **2. PA Integration (Line-Out)** | `PCM5102A` (I2S DAC with 3.5 mm / RCA) | 5 V USB | Connects via AUX to existing public address racks in community centers or schools |
| **3. Outdoor High-Power** | `PCM5102A` + `TPA3116D2` (50–100 W) + IP65 Horn | 12–24 V DC (with 5 V buck) | Village squares, schoolyards, memorials, open streets |

> [!TIP]
> Route the six audio and control lines (`5V`, `GND`, `GPIO 25`, `GPIO 26`, `GPIO 22`, and `GPIO 4`) to a dedicated pin header, JST-XH receptacle, or screw terminal block to swap between output stages without modifying the controller board.

## Project Structure

```
minute-of-silence-device/
+-- .cargo/                       # Cargo build target and runner configuration
+-- .github/
|   +-- workflows/ci.yml          # GitHub Actions bare-metal firmware build workflow
|   \-- FUNDING.yml               # Project sponsorship information
+-- src/
|   +-- audio.rs                  # Audio cadence synthesis (metronome, bells) & I2S interface
|   +-- button.rs                 # Active-low hardware button driver (GPIO 0 / BOOT)
|   +-- led.rs                    # Status indicator LED controller (GPIO 2)
|   +-- main.rs                   # Firmware entry point, peripherals, and event loop
|   +-- scheduler.rs              # Daily ceremony trigger loop with compensation
|   +-- sntp.rs                   # SNTP network time synchronization via embassy-net
|   \-- wifi.rs                   # Wi-Fi station controller via esp-radio
+-- Cargo.toml                    # Firmware package manifest (no_std, alloc)
+-- rust-toolchain.toml           # Xtensa ESP toolchain specification
\-- README.md
```

## Development Setup

### Prerequisites

Install the Rust toolchain for Xtensa and flashing utilities:

```bash
cargo install espup espflash
espup install
```

Ensure your shell environment is initialized with the ESP toolchain:

```bash
source ~/export-esp.sh
```

### Wi-Fi Configuration

Configure network credentials at compile time via environment variables:

```bash
WIFI_SSID="YourNetworkName" WIFI_PASS="YourPassword" cargo build --release
```

If omitted, the firmware defaults to searching for an open access point named `MinuteOfSilence`.

### Building and Flashing

Build the firmware binary:

```bash
cargo build --release
```

Flash the device and open the serial monitor:

```bash
cargo espflash flash --monitor
```

### Manual Testing

Press the button connected to `GPIO 0` (or the on-board **BOOT** button on standard ESP32 development boards) at any time to immediately trigger a test ceremony.

## Contributing

Contributions are welcome and appreciated! Here's how you can contribute:

1. Fork the project
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

Please make sure to update tests as appropriate and adhere to the existing coding style.

## License

This project is licensed under the CSSM Unlimited License v2.0 (CSSM-ULv2). See the [LICENSE](LICENSE) file for details.
