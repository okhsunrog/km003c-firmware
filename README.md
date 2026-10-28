# KM003C custom firmware

Rust 2024 firmware for ChargerLAB POWER-Z KM003C HW2.1 (HC32F460JETA).
The application starts at `0x4000` and preserves the native bootloader and ICG.

**Display, all four buttons and Embassy USB CDC echo are verified on hardware.**
The 240×240 ST7789-compatible panel shows an `embedded-graphics` diagnostic UI.
Button rows highlight and count presses; concurrent byte-for-byte USB echo
passed for 1, 23, 64, 7, 64, 17, 128, 256 and 512 bytes.
The image returns to native DFU about 90 seconds after startup. Holding Exit
for two seconds also requests DFU; this second path has not been separately
tested.

The initial device tests restored the original 1.9.9 application. Subsequent
ADC tests used stock 2.0.7 and restored that same version with full application
readback and unchanged bootloader bytes. [Display/button evidence](hardware/display-buttons/README.md) records
the current run; [USB evidence](hardware/usb-cdc/README.md) preserves earlier
tests.

## Display and buttons

The panel shares QSPI with external flash. The Rust transport uses PB14 SCK,
PB13 MOSI, PB10 D/C and PB15 panel CS. It temporarily deselects flash through
PB1 GPIO/pull-up, verifies that CS is high and restores its AF7 configuration
after each transaction. No other task or DMA may access mapped flash meanwhile.

AW9523B at I2C `0x5b` provides panel reset through P1_1 and backlight dimming
through registers `0x22` / `0x23`. Its hardware ID `0x23` was read successfully.
I2C3 uses PC13 SDA (AF48), PH2 SCL (AF49), 100 kHz and interrupt-driven HAL I/O.
Read-modify-write operations preserve unrelated expander outputs.

| Button | MCU pin | Active level |
| --- | --- | --- |
| Exit | PB7 | Low |
| OK | PB11 | Low |
| Left | PB8 | Low |
| Right / lightning | PB9 | Low |

Inputs have pull-ups and are sampled every 20 ms with repeated-sample debounce.
The UI composes changed rows in a 14.7 KB RAM tile and uploads each row as a
continuous RGB565 block. It does not clear the screen on button changes. ADC
text is updated only when its displayed string changes; no heap is used.
This is not TE/VSYNC synchronization. Retention RAM contains
the debounced mask at `0x200f0d20` and four press counters at `0x200f0d24`
onward.
The lightning-marked button's electrical CC behavior is not established by this
UI test.

## USB implementation

The optional `measurement` feature now reads the hardware-confirmed INA229:
voltage, shunt, temperature and signed current. Two 90-second runs collected
720 samples without SPI errors, with concurrent USB echo. Current `I0` uses the
native scale before per-device factory correction. [ADC evidence](hardware/adc-207/README.md)
and [ADC/PD bring-up](docs/adc-pd.md) describe limits: cold-start configuration
and the wide range remain untested. Passive PD capture is still a candidate.
Both features remain disabled by default.

The [HC32 HAL](https://github.com/okhsunrog/hc32f460-embassy-rs) provides PAC,
interrupt routing and clock setup. The USB task runs on `embassy-executor` and
uses `embassy-usb` CDC ACM with **unmodified** `embassy-usb-synopsys-otg` from
Embassy revision `c2413d5994bbd8b7e6d5975fd49e6427cb903a02`.
The Cargo source override keeps the USB driver trait crate on the same revision;
it does not apply source changes to the Synopsys driver.

The port explicitly configures:

- HRC-backed MPLL, nominal HCLK 192 MHz and USB PLLR 48 MHz; conservative SRAM
  wait states are installed before raising HCLK. The loader jumps into an
  application before its DFU clock setup, so that setup cannot be inherited.
- PA9 VBUS, PA11 DM and PA12 DP, USB clock gate, internal PHY/core reset and
  device mode. IRQ0 routes INTC event `0x113` to the generic driver handler.
- Six endpoint states/TX FIFOs and a total FIFO budget of 308 words, matching
  the live Synopsys 3.30a hardware configuration.
- **32 additional RX FIFO words for status/setup overhead.** With EP0 and CDC
  OUT payload allocations, this yields the native loader's 64-word RX FIFO.
  Leaving this reserve at zero reproducibly stalled enumeration. That was a
  port configuration error; speculative EP0 driver patches were removed.
- An eight-packet queue between independent receive and transmit futures, so
  a multi-packet OUT transfer can finish while its echo waits for host IN reads.

USB enumeration and echo work with the interrupt-driven async path. There is
no busy polling, SDK C driver, local driver fork or alternate USB stack in the
final image. With default features, the measurement circuit and PD logic are
not initialized; `measurement` adds read-only SPI2 access to the INA229.
VBUS detection is disabled for the HID-port-powered test setup;
self-powered operation and general disconnect/reconnect acceptance remain open.

## Build and package

```sh
cargo build --release --locked
cargo fmt -- --check
cargo clippy --release --locked -- -D warnings
```

The configured target is `thumbv7em-none-eabihf`. Use `rust-objcopy -O binary`
on `target/thumbv7em-none-eabihf/release/km003c-rust-smoke` for the raw image.
Keep task artifacts under
`/home/okhsunrog/tmp_zfs/codex_tmp/km003c-rust-bringup/`.

The offline packaging helper reuses the Python modules from the sibling
research checkout:

```sh
uv run --project ../km003c-protocol-research python \
  tools/package_custom_firmware.py INPUT.bin OUTPUT.mencrypt
km003c firmware inspect OUTPUT.mencrypt
```

Build/package commands never flash a device. Installing this experimental image
requires a verified original backup and a maintenance transfer followed by DFU
readback. The normal complete-install command expects the stock application
protocol after restart; this image instead presents CDC and returns to DFU.
The archived hardware harness shows the lower-level library flow actually used.

## Verify CDC echo

```sh
uv run tools/usb_echo.py --output RESULT.json
```

On systems where PyUSB cannot discover libusb, provide `--libusb
PATH_TO_LIBRARY`.
The tool waits for the experimental product descriptor, claims its CDC
interfaces, sets DTR and verifies the listed payload sizes through bulk
endpoints
`0x01` / `0x82`. It temporarily detaches an attached kernel driver if necessary
and attempts to restore it when finished. No serial kernel driver is required.
The payloads and eight-packet queue are intended for these bounded bring-up
checks, not unrestricted buffering or sustained-throughput acceptance.

SysTick advances the HAL Embassy time driver and the independent reset deadline.
The deadline cannot recover a hang that permanently masks interrupts.

## Retention diagnostics

Trace logging uses a 3072-byte text ring at `0x200f0000`, with its monotonic
write cursor at `0x200f0c00`. Read the last `min(cursor, 3072)` bytes in ring
order.
It survives a software reset; complete power loss need not preserve it.

The panic handler formats a bounded, zero-terminated 256-byte message at
`0x200f0e00`, records the source line at `0x200f0fd0` and a zero-padded 24-byte
file prefix at `0x200f0fd4`, then requests native DFU and resets. Interrupts are
masked while recording panic/reset state. Both the source location and formatted
message were verified on hardware when
the flash-CS guard triggered during display bring-up.

The status at `0x200f0fcc` distinguishes panic (`0xbad00002`), HardFault
(`0xbad00001`) and normal USB-test timeout (`0x600d0002`). On normal timeout,
`0x200f0e00` instead holds USB/clock register snapshots. The stage at
`0x200f0fd0` then contains 3 and `0x200f0fd8` is the USB IRQ count; do not
interpret those fields as a panic location unless the status indicates panic.
The loader request is a separate word at `0x200f0ff0`.

Read diagnostics in native DFU before restoring the stock app. Bootloader reply
IDs are independent of request IDs, so an APP-only memory reader is
insufficient.

## Recovery

Hold Exit while connecting power through the HID USB port to enter native DFU.
Exit is PB7, active-low. Keep an exact original package and acquire two matching
backups before the first erase of a newly encountered stock image. Reuse that
verified backup for later tests after checking device identity and live metadata;
do not acquire the same full backup twice on every iteration.
Verify image readback before starting it, and
compare loader bytes throughout. The first four loader bytes were not readable
through the acquired dump and are not part of that comparison.

The initial 1672-byte Rust execution test and the complete MCU-pad inventory,
loader analysis, source links and earlier evidence live in the
[research
repository](https://github.com/okhsunrog/km003c-protocol-research/tree/main/docs/firmware).
