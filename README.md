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
| **Audio DAC / Amp** | MAX98357A | 3.2 W Class D mono I2S amplifier |
| **Speaker** | 4–8 Ω, 3 W | 40 mm or 50 mm full-range driver |
| **Status LED** | On-board / External LED | Active-high status indicator |
| **Test Button** | Tactile switch / BOOT button | Active-low button with internal pull-up |
| **Power** | 5 V USB | Standard Micro-USB or USB-C power supply |

### Wiring

#### I2S Audio (MAX98357A)

| MAX98357A Pin | ESP32 GPIO | Function |
|---|---|---|
| `LRC` / `WS` | `GPIO 25` | Word Select (Left/Right clock) |
| `BCLK` | `GPIO 26` | Bit Clock |
| `DIN` | `GPIO 22` | Digital Audio Data In |
| `GND` | `GND` | Ground |
| `VIN` | `5V` / `VBUS` | Power supply (5 V recommended) |

#### Status LED and Controls

| Peripheral | ESP32 GPIO | Configuration | Description |
|---|---|---|---|
| **Status LED** | `GPIO 2` | Output (Active-high) | Illuminates upon boot and blinks during ceremony |
| **Test Button** | `GPIO 0` | Input (Pull-up, active-low) | Triggers on-demand memorial ceremony for testing (BOOT button) |

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

## License

This project is licensed under the CSSM Unlimited License v2.0 (CSSM-ULv2). See the [LICENSE](LICENSE) file for details.
