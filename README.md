# KM003C custom firmware

Rust 2024 firmware for ChargerLAB POWER-Z KM003C HW2.1 (HC32F460JETA).
The application starts at `0x4000` and preserves the native bootloader and ICG.

The initial 1672-byte smoke image was tested on hardware: Rust wrote its
retention marker, reset into native DFU, and the original 1.9.9 firmware was
restored with complete readback verification. The current working tree adds
Embassy executor/USB dependencies and an unfinished Synopsys USB adapter.
**The USB integration has not yet been built successfully or tested on hardware.**

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
