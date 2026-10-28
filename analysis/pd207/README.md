# Installed 2.0.7 PD evidence

Offline analysis dated 2026-10-29, using the two-read verified installed stock
application with SHA-256
`c63057eaf08b8625e7ac2fc7270c06d75515e78fdbdd46080bdf4b696984ec67`.
No device access, flash writes or CC transmissions were performed for this work.

- `address-map.json`: catalog 1.9.9 to installed 2.0.7 function mapping. Short
  prefix matches were followed by instruction/control-flow checks.
- `native/`: Ghidra 12.1.4 exports, normalized only for whitespace. Types and
  signatures are inferred; register/call operations are the relevant evidence.
- `encoder207.asm`: Capstone disassembly of the encoder after executing the
  original native scatter-load/decompression stage in Unicorn.
- `provenance.json`: image hash, emulator boundaries and CRC-model limits.
- `../../crates/km003c-drivers/tests/data/native-pd207.tsv`: 162 complete native
  encoder results; selectors 0..4 are SOP variants, 5/6 Hard/Cable Reset.

The emulator models CRC MMIO with zlib, after running the native CRC32 init
(`0x5dc8`, mode `0x1e`). CPU encoding instructions execute directly. Timer/DMA,
interrupt scheduling and CC electrical characteristics are not modeled.
The Rust encoder matches all vectors exactly and its independent RX decoder
recovers every packet/reset. This is not a hardware TX acceptance test.

The [TX analysis](../../docs/pd-transmit.md) contains the resulting sequencing,
GPIO-state tables, uncertainty boundaries and usbpd adapter requirements.
