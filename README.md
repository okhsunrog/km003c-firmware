# KM003C custom firmware

Rust 2024 firmware for ChargerLAB POWER-Z KM003C HW2.1 (HC32F460JETA).
The application starts at `0x4000` and preserves the native bootloader and ICG.

The initial 1672-byte smoke image ran successfully on HW2.1. The current image
starts an Embassy executor and CDC ACM echo task, explicitly configures a 48 MHz
HCLK and 48 MHz USB PLLR clock, and returns to native DFU after about 20 seconds.

**Hardware status:** the USB image has been flashed and read back, and the
20-second return-to-DFU path works. Host reset/enumeration events reach the core,
but descriptor exchange and CDC enumeration are not working yet. Continuous
polling was also tried and did not fix enumeration; the normal async path is
retained. Do not describe this as working USB support.

## Retention diagnostics

The panic handler formats `PanicInfo` into a bounded, zero-terminated 256-byte
buffer at `0x200f0e00`, without allocation. It records the source line at
`0x200f0fd0` and a zero-padded 24-byte file prefix at `0x200f0fd4`. The status
word at `0x200f0fcc` distinguishes panic (`0xbad00002`), HardFault
(`0xbad00001`) and normal USB-test timeout (`0x600d0002`). It then writes the
native loader request to `0x200f0ff0` and performs a system reset.

The location record was verified on hardware: it identified an early core-ID
read before clock ungating, then the invalid assumption that application entry
inherits the DFU clock configuration. Formatting the full panic message is a
later extension; its complete text has not yet been verified on hardware.
Retention survives the software reset, not guaranteed loss of power. Read through
the native loader before restoring the stock app; its replies use independent
transaction IDs, so an APP-only memory reader is insufficient.

On a normal USB-test timeout, the first 48 bytes at `0x200f0e00` instead contain
USB/clock register snapshots; interpret the buffer according to the status word.
The original 1.9.9 package is restored and checked after experiments.

## Build

```sh
cargo build --release --locked
cargo fmt -- --check
cargo clippy --release --locked -- -D warnings
```

The target is configured as `thumbv7em-none-eabihf`. The pinned
[HC32 HAL](https://github.com/okhsunrog/hc32f460-embassy-rs) supplies the PAC,
interrupt vectors and routing. USB uses `embassy-usb-synopsys-otg`.

## Recovery and verification

Hold Exit while connecting power through the HID USB port to enter native DFU.
Exit is PB7, active-low. Keep a verified exact original firmware package and
acquire two matching backups before the first erase. Compare the installed
image and bootloader bytes before starting experimental code. Restore the
original application after a one-shot test.

The packaging helper in `tools/` currently depends on the shared Python modules
and dependencies from the sibling `km003c-protocol-research` checkout; it is
not yet a standalone packaging tool. No automatic flashing is performed by
build commands.

Hardware mapping, loader analysis and archived execution evidence live in the
[research repository](https://github.com/okhsunrog/km003c-protocol-research/tree/main/docs/firmware).
