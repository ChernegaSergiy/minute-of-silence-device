[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/banner-direct-single.svg)](https://stand-with-ukraine.pp.ua)

# Minute of Silence Device

> [!IMPORTANT]
> **Project Archived**
>
> This repository is archived and no longer maintained. The firmware is a proof of concept: its audio output has never been verified with a DAC, an amplifier and a speaker, so it should not be relied on for real ceremonies.
>
> **Why this project exists**
>
> -   It grew out of the [Minute of Silence](https://github.com/ChernegaSergiy/minute-of-silence) desktop app. After adding settings export/import and local-network settings transfer, I wanted to build firmware for an ESP32-class microcontroller that could be configured from the app and behave the same way as it does.
> -   The goal was an affordable solution for village councils and schools, sold at the cost of its components rather than at a commercial markup. Commercial air-raid alert units on the market are listed at roughly UAH 4,000-9,500 (for example, an audio module at UAH 8,499). With a feature such as air-raid alerts, the most expensive parts of this device would have been the amplifier with a loudspeaker and a roaming provider's subscription.
>
> **Why it is archived**
>
> -   Finishing the project would require an audio chain (a DAC, an amplifier and a speaker) purely for debugging. Buying it for a project with uncertain prospects was not justified, and without it I can't test or debug audio output. I'm not willing to keep building on firmware I can't verify.
> -   When I drew up a bill of materials, it became clear that the savings over existing commercial solutions would be small. Given that, I decided not to leave the project half-finished.
>
> The code stays available as a reference. If you have the hardware and the time to take this further, you are welcome to fork it under the terms of the license.

[![CI](https://github.com/ChernegaSergiy/minute-of-silence-device/actions/workflows/ci.yml/badge.svg)](https://github.com/ChernegaSergiy/minute-of-silence-device/actions)
[![License: CSSM Unlimited License v2.0](https://img.shields.io/badge/License-CSSM%20Unlimited%20License%20v2.0-blue.svg?logo=opensourceinitiative)](LICENSE)

**Minute of Silence Device** is an autonomous IoT device built with bare-metal Rust (`no_std`) for the **ESP32-WROOM-32** microcontroller. It schedules the daily memorial ceremony for 09:00 using NTP time synchronization and can output supported audio files through an external I2S DAC. Physical audio playback has not yet been verified with a DAC and speaker.

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

The firmware outputs I2S digital audio for an external DAC. The following hardware profiles are possible configurations, not hardware-verified setups:

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
|   +-- audio.rs                  # Ceremony block playback and audio sink
|   +-- audio_timing.rs           # Pause before the metronome
|   +-- button.rs                 # Active-low hardware button driver (GPIO 0 / BOOT)
|   +-- flash_cache.rs            # Internal Flash audio cache reader (audio_cache partition)
|   +-- flash_layout.rs           # Audio cache and settings partition bounds
|   +-- http_server.rs            # Pairing server on TCP port 80 for protected settings transfer
|   +-- i2s_sink.rs               # I2S DMA audio output
|   +-- led.rs                    # Status indicator LED controller (GPIO 2)
|   +-- led_pattern.rs            # Blink pattern that shows the pairing code
|   +-- main.rs                   # Firmware entry point, peripherals, and event loop
|   +-- mdns.rs                   # Multicast DNS & DNS-SD responder (_khvylyna._tcp.local, khvylyna.local)
|   +-- pcm.rs                    # PCM frame conversion and volume
|   +-- scheduler.rs              # Daily ceremony trigger loop with compensation
|   +-- sdcard.rs                 # MicroSD SPI driver & WAV streaming parser (FAT32)
|   +-- settings_flash.rs         # Settings flash access via esp-storage
|   +-- settings_record.rs        # Checksummed two-slot settings record format
|   +-- settings_store.rs         # Loading and durably saving settings
|   +-- sntp.rs                   # SNTP network time synchronization via embassy-net
|   +-- transfer.rs               # Parsing of settings received through pairing
|   +-- wav.rs                    # Supported WAV header validation
|   \-- wifi.rs                   # Wi-Fi station controller via esp-radio
+-- Cargo.toml                    # Firmware package manifest (no_std, alloc)
+-- partitions.csv                # ESP32 partition table (1.5 MB app, 2.4 MB audio_cache, 64 KB settings)
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

The firmware attempts audio playback in this order:

1. **Tier 1 (MicroSD Card)**: If an SD card is present, WAV files matching the active preset and voice are streamed directly from FAT32 storage via SPI.
2. **Internal Flash cache**: If the SD file is unavailable, the player attempts to read a cached WAV block from the dedicated `audio_cache` partition (approximately 2.4 MiB).
3. **Missing audio**: If neither source provides the block, the firmware waits for the announcement, bell, metronome, or ending duration without producing sound. A missing anthem is skipped. If playback fails after I2S transmission has started, the block ends without retrying the other source.

The firmware currently reads the audio cache but does not populate it: audio-file transfer and cache writing are not implemented. Audio files must currently be placed on a FAT32 microSD card separately.

The I2S output supports canonical 44-byte-header WAV files containing 16-bit PCM at **44,100 Hz**, in mono or stereo. Mono samples are duplicated to both output channels. Other sample rates and formats are not played.

The selected `Silence` block lasts 60 seconds. When an opening announcement or bell is immediately followed by the metronome, a one-second pause is inserted before the metronome, using the shared `khvylyna-core` timing rule.

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

If a file is absent or the card is disconnected, the firmware tries the Flash cache and then handles missing audio as described above; it does not synthesize a replacement sound.

### Local Device Discovery (mDNS & DNS-SD)

Upon connecting to the local network via Wi-Fi, the device joins multicast group `224.0.0.251` on UDP port `5353` and responds to:

- **DNS-SD service queries (`_khvylyna._tcp.local`)**: replies with `PTR`, `SRV` (port `80` -> `khvylyna.local`), `TXT` (`kind=hardware`, `name=Khvylyna ESP32`, `ver=0.1.0`), and `A` records, using unicast replies when queried from ephemeral ports. This allows the Minute of Silence desktop and Android apps to discover and classify the device automatically.
- **Hostname queries (`khvylyna.local`)**: resolves to the device's IPv4 address.

### Remote Settings Sync (Pairing on Port 80)

Settings are sent from the desktop or Android app over a protected exchange, not over plain HTTP:

1. Hold the button on `GPIO 0` for at least 1.5 seconds and release it. The device creates a time-limited, single-use six-digit code. The status LED blinks it digit by digit (a digit is its number of blinks, `0` is one long blink), and the code is also printed to the serial log.
2. The app finds the device through mDNS, connects to TCP port `80`, and authenticates with the code using the `khvylyna-pairing` exchange. The settings are then transferred encrypted.
3. The firmware runs `khvylyna-core` migration and validation, saves the settings to flash, and only then confirms to the app. They are applied in the ceremony loop without rebooting (updating voice assets and schedule).

If saving fails, or a ceremony is playing, the settings are rejected and the previous ones stay in force: erasing flash would stall audio streamed from it.

### Persistent Settings

Settings are stored in the `settings` partition as two alternating 16 KiB slots. Each record carries a sequence number and a CRC-32, and a new record is written to the slot that does not hold the current settings. If power is lost during a write, the previous settings are still found and used at the next start. Corrupted or invalid records are skipped.

At startup the device loads the newest valid saved settings, or the defaults if there are none, and reports which one it used in the serial log.

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
