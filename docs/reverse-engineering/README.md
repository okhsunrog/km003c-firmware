# Hardware and stock-firmware analysis

Board wiring, native peripheral configuration and recovery research are kept
here alongside the custom Rust implementation and its hardware acceptance logs.
The material was moved from `km003c-protocol-research` after reviewing the
2026-10-28 commits, starting with `03f3ad4` and including updates through
`2a1a145`. The protocol research repository retains its USB/package documentation
and a short hardware description; hardware bring-up is maintained here.

| Document | Contents |
| --- | --- |
| [Board bring-up](board_bringup.md) | Buttons and recovery, 48-pad inventory, peripheral setup, hardware acceptance |
| [HC32F460 analysis](hc32f460_analysis.md) | Register map, clocks, SRAM/OTP and instruction evidence |
| [Stock overview](overview.md) | Native RTOS, memory layout, peripherals and protocol-task organization |
| [Bootloader](bootloader.md) | Startup gates, DFU dispatcher, flash/CRC/metadata flow, failure behavior |
| [PD PHY research](pd_phy_hc32f460.md) | Comparator/timer/DMA signal path and native PD implementation |
| [Current line ADC](../line-adc.md) | Stock 2.0.7 scale, D+/D−/CC voltages and automatic CC selection |
| [Current PD reception](../pd-buffered.md) | Interrupt-rearmed buffers, ownership, loss counters and scheduling |
| [PD transmit research](../pd-transmit.md) | Timer/DMA waveform generation, board controls, GoodCRC responsibilities |

Unless labeled otherwise, older catalog analysis uses the decrypted 1.9.9 image
in `fw/KM003C_V1.9.9_key0_ecb.bin`, SHA-256
`9ce6125da454585fde1b5744018f94edc697a23941292cbf55d2a411d4df7517`.
Original `FUN_...` catalog labels are image offsets unless explicitly marked as
runtime addresses. Add `0x4000` to those offsets; raw vector, peripheral and RAM
addresses are already runtime values. Version-specific 2.0.7 findings use their
stated runtime addresses and image hash. Prefer the installed newer version
when evidence differs, and keep those versions identifiable.

The `evidence/` directories preserve the acquired loader, GPIO/button and early
Rust test records. Acquisition/import JSON and archived test sources retain
their original provenance and historical host paths. SHA256 manifests use the
new locations; the underlying binary, C/assembly and JSON evidence is unchanged.
See `fw/README.md` for the two retained binary inputs.
