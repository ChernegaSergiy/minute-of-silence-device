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
| **Audio Storage** | MicroSD Card Module (SPI) + Internal Flash | Primary FAT32 SD card with 2.4 MB internal Flash cache fallback |
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
|   +-- flash_cache.rs            # Internal Flash audio cache reader (audio_cache partition)
|   +-- http_server.rs            # Lightweight HTTP/1.1 server (POST /api/bundle) for live settings sync
|   +-- led.rs                    # Status indicator LED controller (GPIO 2)
|   +-- main.rs                   # Firmware entry point, peripherals, and event loop
|   +-- mdns.rs                   # Multicast DNS & DNS-SD responder (_khvylyna._tcp.local, khvylyna.local)
|   +-- scheduler.rs              # Daily ceremony trigger loop with compensation
|   +-- sdcard.rs                 # MicroSD SPI driver & WAV streaming parser (FAT32)
|   +-- sntp.rs                   # SNTP network time synchronization via embassy-net
|   \-- wifi.rs                   # Wi-Fi station controller via esp-radio
+-- Cargo.toml                    # Firmware package manifest (no_std, alloc)
+-- partitions.csv                # ESP32 partition table (1.5 MB app + 2.4 MB audio_cache)
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

### Tiered Audio Storage Hierarchy

The firmware employs a resilient three-tier audio playback fallback:

1. **Tier 1 (MicroSD Card)**: If an SD card is present, WAV files matching the active preset and voice are streamed directly from FAT32 storage via SPI.
2. **Tier 2 (Internal Flash Cache)**: If the SD card is absent, the player reads pre-cached WAV blocks for the active preset from the dedicated `audio_cache` internal Flash partition (2.4 MB).
3. **Tier 3 (Synthetic Cadence)**: If neither storage tier has the audio file, the system synthesizes 440 Hz bell resonance strikes and 1200 Hz metronome pulses on-the-fly.

During normal operation, the microcontroller never writes to internal Flash. Audio caching occurs strictly during intentional configuration import operations.

### MicroSD Audio Assets

The device looks up uncompressed 16-bit PCM WAV audio files at the root of a FAT32 MicroSD card using standard 8.3 FAT filenames corresponding to the active voice configuration:

| Category | File | Description |
|---|---|---|
| **Memorial Announcement** | `ANN_HDAL.WAV` | Voice: Bohdan Hdal (default) |
| | `ANN_SOTN.WAV` | Voice: Sonia Sotnyk |
| | `ANN_KHOM.WAV` | Voice: Dania Khomutovskyi |
| | `ANN_R_BG.WAV` | Voice: Radio BG |
| | `ANN_VINN.WAV` | Voice: Vinnytsia City |
| **Cadence & Bells** | `BELL.WAV` | Memorial bell strikes |
| | `METRONOM.WAV` | 60-second memorial metronome cadence |
| **Closing Recording** | `END_HDAL.WAV` | Closing: Bohdan Hdal |
| | `END_SOTN.WAV` | Closing: Sonia Sotnyk |
| | `END_KHOM.WAV` | Closing: Dania Khomutovskyi |
| | `END_R_BG.WAV` | Closing: Radio BG |
| **National Anthem** | `ANTHEM.WAV` | State Anthem of Ukraine (standard choral) |
| | `ANTH_KHO.WAV` | Anthem performed by Mykhailo Khoma |
| | `ANTH_PON.WAV` | Anthem performed by Oleksandr Ponomarov |

If any file is absent or the card is disconnected, the firmware gracefully falls back to synthetic cadence timing without interruption.

### Local Device Discovery (mDNS & DNS-SD)

Upon connecting to the local network via Wi-Fi, the device joins multicast group `224.0.0.251` on UDP port `5353` and responds to:

- **DNS-SD service queries (`_khvylyna._tcp.local`)**: replies with `PTR`, `SRV` (port `80` -> `khvylyna.local`), `TXT` (`kind=hardware`, `name=Khvylyna ESP32`, `ver=0.1.0`), and `A` records, using unicast replies when queried from ephemeral ports. This allows the Minute of Silence desktop and Android apps to discover and classify the device automatically.
- **Hostname queries (`khvylyna.local`)**: resolves to the device's IPv4 address so it can also be reached directly as `http://khvylyna.local`.

### Remote Settings Sync (`POST /api/bundle`)

The firmware runs a lightweight HTTP/1.1 listener on TCP port `80` exposing `POST /api/bundle`:

- Accepts a JSON `ExportBundle` (or direct `Settings` JSON) up to 16 KiB, matching the format exported by the desktop and Android apps.
- Runs `khvylyna-core` migration and validation before applying the configuration live in the ceremony loop (updating voice assets and schedule without rebooting).
- Responds with `200 OK` on success, or `400` / `404` / `405` / `413` on invalid requests.

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
