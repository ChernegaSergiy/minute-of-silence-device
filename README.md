# Minute of Silence Device

*A standalone hardware device and firmware for Ukraine's daily national moment of silence.*

[![License: CSSM Unlimited License v2.0](https://img.shields.io/badge/License-CSSM%20Unlimited%20License%20v2.0-blue.svg?logo=opensourceinitiative)](LICENSE)

---

**Minute of Silence Device** is an autonomous IoT device built with bare-metal Rust (`no_std`) for the **ESP32-WROOM-32** microcontroller. It accurately triggers the daily memorial ceremony at 09:00 using NTP time synchronization, playing audio announcements, metronome, and bell through an I2S digital audio interface.

The firmware reuses the core domain logic and data contracts directly from [khvylyna-core](https://github.com/ChernegaSergiy/minute-of-silence/tree/main/crates/khvylyna-core), sharing the exact same ceremony configuration, block sequencing, and validation rules as the desktop application.

## Hardware Specifications

| Component | Recommendation | Description |
|---|---|---|
| **MCU** | ESP32-WROOM-32 | Dual-core Xtensa LX6, 4 MB Flash |
| **Audio DAC / Amp** | MAX98357A | 3.2 W Class D mono I2S amplifier |
| **Speaker** | 4–8 Ω, 3 W | 40 mm or 50 mm full-range driver |
| **Power** | 5 V USB | Standard Micro-USB or USB-C power supply |

### Wiring (I2S Audio)

| MAX98357A Pin | ESP32 GPIO | Function |
|---|---|---|
| `LRC` / `WS` | `GPIO 25` | Word Select (Left/Right clock) |
| `BCLK` | `GPIO 26` | Bit Clock |
| `DIN` | `GPIO 22` | Digital Audio Data In |
| `GND` | `GND` | Ground |
| `VIN` | `5V` / `VBUS` | Power supply (5 V recommended) |

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

### Building and Flashing

Build the firmware binary:

```bash
cargo build --release
```

Flash the device and open the serial monitor:

```bash
cargo espflash flash --monitor
```

## Related Projects

- [minute-of-silence](https://github.com/ChernegaSergiy/minute-of-silence) — The desktop application (Tauri 2 + React + Rust).

## License

Licensed under the CSSM Unlimited License v2.0. See [LICENSE](LICENSE) for details.
