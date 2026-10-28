# Measurement and passive PD candidates

Offline implementation dated 2026-10-28. No device reads, writes, resets or
firmware installation were performed during this work. Existing display/button
and USB acceptance does **not** establish hardware acceptance of these drivers.

## Measurement

The native SPI2 code is stronger evidence than an unidentified SPI peripheral:

- `0x7118` sends an 8-bit `register << 2` command and a 16-bit value.
- `0x703c` / `0x7098` send `(register << 2) | 1` then read 16/24 bits.
- `0x6f8e` reads register 5 and shifts by 4; `0x710a` reads signed register 6.
- `0x6eb2` reads signed register 7 and switches CONFIG bit 4 for range changes.
- `0x6f9a` writes CONFIG=`0x0170`, ADC_CONFIG=`0xfdb0`, SHUNT_CAL=`0x1062`,
  SHUNT_TEMPCO=`0x001e`. These are INA229 register numbers and SPI framing.

All four SPI helper addresses map to the preserved device 1.9.9 image. The
INA229-compatible driver remains a candidate until physical manufacturer ID
`0x5449` and device ID upper 12 bits `0x229` are read. This does not identify a
photographed component on another unit or prove all board revisions are equal.
See TI's [SPI framing](https://www.ti.com/document-viewer/INA229/datasheet/GUID-9C15D0DA-F5E2-4FE1-9EB9-B62E23643817)
and [register map](https://www.ti.com/document-viewer/INA229/datasheet/GUID-25B37629-8261-48D8-BE56-62622C36C345).

```sh
cargo build --release --locked --features measurement
```

This opt-in image starts the read-only task on SPI2: PA8 SCK AF47, PA10 MOSI
AF44, PB0 MISO AF45, PA7 manual CS; mode 1, 3 MHz at PCLK1=96 MHz. The transport
keeps CS asserted across the command and response, drains received command bytes,
bounds polling, and disables SPI/releases CS on a transfer error. It stops on an
ID mismatch or bus error. It never applies `NATIVE_199` automatically.

The UI subtitle reports millivolts and raw signed current counts. Logging adds
shunt voltage, current counts and diagnostic flags. Bus voltage uses 195.3125 µV
per count; shunt range comes from the read CONFIG bit. Current/power scaling is
withheld until CURRENT_LSB/shunt calibration is established. Samples are separate
register reads, not atomic snapshots. Reading DIAG_ALRT clears some latched flags;
conversion mode/freshness must be checked during acceptance. An externally shut
down monitor can return stale results until explicitly configured.

The library's `configure` method checks identity and legal bits before writing;
it starts the ADC mode last. An interrupted configuration is not rolled back.
The internal HC32 ADC/PGA pin map and ripple sampling remain separate work.

One ADC interaction in the native PD transmitter is now more precise:
`0x52e4` changes `0x400400a0` bit 0, which the SDK identifies as
`ADC1.AWDCR.AWDEN` (analog watchdog enable). TX disables it and later enables it.
That identifies the register operation; the reason and monitored signal are
still unresolved. The passive receive implementation does not change ADC1.

## Passive PD

`crates/km003c-drivers/src/pd.rs` accepts edge intervals with caller-provided
half-bit ticks and tolerance. It implements BMC, 4b5b, five exact SOP ordered sets,
Hard/Cable Reset, header data-object count, EOP and reflected CRC-32 verification.
Packets preserve the wire header and payload. Extended messages are raw chunks;
there is no reassembly, GoodCRC generation, PD negotiation or transmitter.

```sh
cargo build --release --locked --features pd-capture
```

This compiles `pd_capture::Receiver`; **main does not construct or start it**.
The hardware operator must verify/configure the analog pad, choose CMP1_INP1 or
INP2 and a DAC code, then explicitly create the receiver. The interface requires
exclusive TMRA2, CMP1/DAC1, DCU1, DMA1 and corresponding AOS/IRQ resources.
It does not configure PA0/PA1 TX, PC14/PC15, CC termination or GPIO switches.
External comparator VCOUT and timer waveform outputs are disabled.

The receive candidate follows the native timer capture/DMA channel 2 path:
CMP1 edges trigger TimerA2 capture channel 3 and clear/start its counter. DMA1
**channel 2** transfers halfwords; DCU1 counts events. There is no DMA2 controller
in this path. Timer overflow ends one burst; DMA completion reports buffer-full.
The existing HAL owns borrowed-buffer DMA cancellation/quiescence. A second
check compares settled DMA remaining count against DCU events before exposing
the captured words. `Receiver::receive` passes intervals and decode errors to a
callback and resets decoding at each packet gap.

Use up to 860 halfwords. `capture` exposes raw intervals for inspecting timing
before assuming a decoder configuration. The native divider is PCLK1/8; our
96 MHz PCLK1 suggests 20 ticks per BMC half-bit, but real timing/threshold and
capture/clear ordering still require measurement. Code 0x20 is the native DAC
setting, not a verified voltage threshold for this board.

This is a single-burst implementation: packets during processing/rearm can be
lost. It does not claim a continuous, lossless analyzer. Timeout, DMA errors,
full buffers, count mismatch, timing, invalid symbols, length/EOP and CRC failures
are explicit. A permanently wedged DMA can stall cancellation while protecting
borrowed memory. Observe raw intervals and error counts during acceptance.

## Offline evidence and checks

[Address mapping](../analysis/adc-pd/address-map.json) maps catalog functions to
the exact preserved device image, with both input hashes. Most PD addresses move
by `0x9c`; the SPI helpers do not. Byte signatures are matching evidence, not a
proof of whole-function equivalence across the two builds.

[Native init trace](../analysis/adc-pd/pd-init-writes.json) records 133 CPU writes
from emulating device entry `0x102c0`. MMIO starts as zero-filled synthetic memory;
the trace does not reproduce hardware reset values, DMA behavior or IRQ timing.
Reproduce with the verified plaintext application dump:

```sh
uv run tools/trace_pd_init.py ORIGINAL.bin --entry 0x102c0 --output OUTPUT_DIR
cargo fmt -- --check
cargo fmt --manifest-path crates/km003c-drivers/Cargo.toml -- --check
cargo clippy --release --locked --all-features -- -D warnings
cargo test --manifest-path crates/km003c-drivers/Cargo.toml --locked \
  --target x86_64-unknown-linux-gnu
```

Host tests cover SPI framing/widths, signed boundaries, physical units, ID/write
gating, CRC check vectors, all SOP variants, maximum/extended payloads, jitter,
truncation, noise, corruption and recovery. They do not verify physical sampling.

A host-only interval decoder can inspect future text exports without connecting
to the device. Input is decimal/hex u16 edge intervals, comma/whitespace separated;
`#` starts a comment. It reports valid packets and decode errors separately:

```sh
cargo run --manifest-path crates/km003c-drivers/Cargo.toml \
  --target x86_64-unknown-linux-gnu --example pd_decode -- intervals.txt 20 3
```
