# KM003C custom firmware

Rust firmware for POWER-Z KM003C HW2.1, HC32F460JETA at 192 MHz.
The application starts at `0x4000` and preserves the native bootloader and ICG.

The firmware includes an embedded-graphics meter UI on the 240×240 ST7789 panel,
buttons, SC7A20-family automatic rotation, INA229 measurements with validated
factory calibration, D+/D−/CC1/CC2 ADC readings and USB-PD reception/transmission.
USB uses postcard-rpc over native bulk endpoints. There is one application build;
CDC, NDJSON, test images and the Python USB clients have been removed.

```sh
cargo build --release --locked
cargo clippy --release --locked -- -D warnings
bash tools/rpc.sh
bash tools/rpc.sh monitor auto
bash tools/rpc.sh monitor 2
bash tools/rpc.sh sink 20
bash tools/rpc.sh loader
```

`monitor` receives passively without Rd, acknowledgements or voltage requests.
`sink` explicitly negotiates a fixed PDO on the ADC-selected CC line. Fixed
5/9/12/15/20 V contracts and active operation in both orientations have been
verified with an unloaded charger. PPS/EPR policy remains unimplemented.
Use active commands with an unloaded charger during development.

Holding Exit for two seconds or issuing the RPC loader command returns to native
DFU. Holding Exit while connecting power through the HID port enters that loader
independently of application USB. Keep a verified original package for recovery.
Updates require readback and unchanged-loader verification. Working custom images
stay installed between tests; there is no automatic stock restore or test timeout.

## Hardware

| Function | Connection |
| --- | --- |
| Exit / OK / Left / Right-lightning | PB7 / PB11 / PB8 / PB9, active low |
| Display SPI | PB14 SCK, PB13 MOSI, PB10 D/C, PB15 CS |
| External NOR | Shared QSPI pins; PB1 flash CS; factory settings at `0x3ff100` |
| AW9523B | I2C3 `0x5b`, PC13 SDA / PH2 SCL, 100 kHz |
| Panel reset / backlight | AW P1_1 / dimming registers `0x22`, `0x23` |
| Accelerometer | SC7A20-family, I2C `0x19`, identity `0x11` |
| CC1 / CC2 receive and ADC | PA0 / PA1, CMP1 with DAC1 reference |
| CC1 / CC2 transmit | TimerA2 channels 1/2, PA0/PA1 AF4, PC14/PC15 gates |
| CC1 / CC2 Rd controls | PB6 high / AW P0_3 low-output; release low / high-Z |
| Host USB | Separate Synopsys FS OTG with internal PHY, PA11/PA12 |

I2C devices share Embassy `I2cDevice` handles; an expander mutex protects whole
read-modify-write sequences. Display/external NOR access remains synchronous.
Display updates use dirty rectangles; TE/VSYNC synchronization is not implemented.
The external flash reader does not erase or program factory settings.

TimerA1 provides Embassy time at 32,768 logical ticks/s, with a free-running
375 kHz counter and compare alarms. SysTick is unused. PD capture uses timer/DMA
and interrupt rearming; decoding runs in a separate priority executor. Software
GoodCRC does not wait for USB. Bounded diagnostics can drop records under load.

## Documentation

- [RPC protocol and host client](docs/rpc.md)
- [PD transmission and electrical controls](docs/pd-transmit.md)
- [PD receive buffering](docs/pd-buffered.md)
- [Line ADC scale](docs/line-adc.md), [INA229/calibration](docs/adc-pd.md)
- [Orientation](docs/orientation.md), [external flash](docs/external-flash.md)
- [Time driver](docs/time-driver.md), [remaining refactoring](docs/refactoring.md)
- [Stock firmware and hardware research](docs/reverse-engineering/README.md)

Historical acceptance records under `hardware/` and research documents preserve
measurements and provenance. Commands/source paths inside those records describe
the versions tested then; they are not the current build or host interface.

## Packaging

The target is `thumbv7em-none-eabihf`. Convert the linked image using
`rust-objcopy -O binary`, then package it offline:

```sh
uv run --project ../km003c-protocol-research python \
  tools/package_custom_firmware.py INPUT.bin OUTPUT.mencrypt
```

The helper reuses the sibling research implementation for authenticated framing.
It never accesses a device. Temporary task artifacts belong under
`~/tmp_zfs/codex_tmp/km003c-rust-bringup/`.
