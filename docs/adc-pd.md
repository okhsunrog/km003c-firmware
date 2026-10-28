# Measurement and passive PD

Implementation and hardware tests dated 2026-10-28. INA229 reads are now
confirmed on HW2.1 alongside USB/display operation. Two 90-second runs produced
720 samples without SPI errors. [Evidence](../hardware/adc-207/README.md) records
stock 2.0.7 comparisons and restoration. Single-burst passive PD now captures
a CRC-valid PPS Request on PA0;
continuous acquisition and the other cable orientation remain unverified.

## Measurement

The native SPI2 code is stronger evidence than an unidentified SPI peripheral:

- `0x7118` sends an 8-bit `register << 2` command and a 16-bit value.
- `0x703c` / `0x7098` send `(register << 2) | 1` then read 16/24 bits.
- `0x6f8e` reads register 5 and shifts by 4; `0x710a` reads signed register 6.
- `0x6eb2` reads signed register 7 and switches CONFIG bit 4 for range changes.
- `0x6f9a` writes CONFIG=`0x0170`, ADC_CONFIG=`0xfdb0`, SHUNT_CAL=`0x1062`,
  SHUNT_TEMPCO=`0x001e`. These are INA229 register numbers and SPI framing.

All four SPI helper addresses map to the preserved device 1.9.9 image. The
INA229 driver returned physical manufacturer ID `0x5449` and device ID
`0x2291`. This does not identify a
photographed component on another unit or prove all board revisions are equal.
See TI's [SPI framing](https://www.ti.com/document-viewer/INA229/datasheet/GUID-9C15D0DA-F5E2-4FE1-9EB9-B62E23643817)
and [register map](https://www.ti.com/document-viewer/INA229/datasheet/GUID-25B37629-8261-48D8-BE56-62622C36C345).

```sh
cargo build --release --locked --features measurement
```

This opt-in image starts the measurement task on SPI2: PA8 SCK AF47, PA10 MOSI
AF44, PB0 MISO AF45, PA7 manual CS; mode 1, 3 MHz at PCLK1=96 MHz. The transport
keeps CS asserted across the command and response, drains received command bytes,
bounds polling, and disables SPI/releases CS on a transfer error. It stops on an
ID mismatch or bus error. After identity checks it explicitly applies
`NATIVE_199`, verifies all four registers and waits up to 100 ms for conversion
ready before publishing the first sample. ADC_CONFIG is written last.

The meter UI dedicates three large rows to voltage, signed current and power,
with temperature and CAL/RAW status below/above them. Button counters remain in
retention/logs; holding Exit for two seconds still requests the native loader.
Changed strings are composed in a 19,008-byte RGB565 tile, including scaled
40-pixel-high glyphs, then uploaded as contiguous rectangles. This reduces
redraw artifacts but does not synchronize to panel scanout.

Bus voltage uses 195.3125 µV/count; shunt range comes from CONFIG. Native 2.0.7
SHUNT_CAL `0x1062` corresponds to 16 µA/current count with ADCRANGE=1, or 64 with
ADCRANGE=0. Narrow range is hardware-tested; wide range is statically derived.
A different SHUNT_CAL or math overflow makes current unavailable. Samples are
separate register reads, not atomic snapshots. DIAG_ALRT reads clear some flags.

### Factory correction and startup

Before panel initialization, its exclusive QSPI owner reads the 96-byte settings-A
block at external flash offset `0x3ff100`. It exits QPI with `FF`, uses standard
SPI commands `90` and `03`, and never sends write-enable, erase or program.
The flash returned legacy ID `0x8515`; its exact model is not established.
CRC-32 covers the first 92 bytes. Legacy/uncalibrated flags or out-of-bounds
coefficients reject calibration; the UI then displays RAW with native `I0` scale.
The implementation does not migrate old settings or silently invent coefficients.

Stock 2.0.7 reader `0x21818` adds `0x3ff000`; startup `0x48520` requests offset
`0x100`. The current settings layout contains five signed voltage offsets at
`+0x1c`, ten current gains at `+0x30` and signed current offset at `+0x58`.
Our unit has offsets -6 µV, gains 1002221 ppm and current offset 94 µA.
The Rust implementation reproduces native `0x17b1c`, including voltage bands,
sign-dependent current gain bands and signed division. A synthetic profile
exercises 324 boundary/sign vectors executed against the actual stock
instructions. All match Rust. This establishes algorithm equivalence for those
vectors, not absolute metrology accuracy.

The stock live RAM block at `0x1fffb5a8` has a mutable word at `+0x10` and a stale
CRC. Its persistent flash copy has a valid CRC. Therefore live Settings protocol
CRC failure must not be treated as evidence that persisted calibration is bad,
and the Rust firmware validates the flash copy without weakening CRC checks.

`--features measurement,measurement-reset-test` first resets only INA229 and
waits 20 ms. Hardware acceptance produced defaults `0000/fb68/1000/0000`, then
native profile `0170/fdb0/1062/001e`, 360 samples, and valid factory correction.
This does not prove a whole-board cold start. The normal measurement image does
not reset the sensor. Physical full-power-off acceptance remains pending.
See [startup/calibration evidence](../hardware/cold-adc/README.md).

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

Alone, this compiles `pd_capture::Receiver` without starting it. Adding
`cdc-telemetry` exposes command-driven passive bursts: `pd 1` / `pd 2` configure
PA0/PA1 as analog and select CMP1_INP1/2 with native DAC code `0x20`.
See [CDC protocol and hardware limits](cdc-telemetry.md). The interface requires
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
96 MHz PCLK1 gives 20 nominal ticks per BMC half-bit. Measured groups are
15/19/23 and 35/39/43 ticks; tolerance 5 decodes the captured Request with CRC.
The first edge starts the timer and is excluded from decoding. Code 0x20 is the
native DAC setting, not a verified voltage threshold for this board.

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

## Retained measurement record

At `0x200f0d40`, 24 little-endian words preserve ADC diagnostics independently
of the wrapping text log. Word 0 is `0x41444331`; word 1 is status (1 probing,
2 configured/read, 3 complete sample, 0 partial publication, `0xe001..0xe008`
probe/identity/config/sample/reset/configure/readback/conversion-timeout error).
Words 2–3 are manufacturer/device IDs;
4–7 are CONFIG/ADC_CONFIG/SHUNT_CAL/SHUNT_TEMPCO; 8 is sample count.
Words 9–13 are last bus/shunt/current/temperature counts and DIAG_ALRT;
14–17 are bus min/max and signed current min/max. Word 18 records SPI status
on an error. Word 19 is signed uncalibrated microamps when word 20 gives its
valid scale in nanoamps/count (0 means unavailable). Word 21 is the flash legacy
ID; 22 is calibration status (1 valid, 2 length, 3 CRC, 4 legacy, 5 uncalibrated,
6 coefficient bounds); 23 is the source offset `0x3ff100`.
Status/count are published last; software reset preserves this bank, power loss
need not. These offsets do not overlap button, panic or timeout records.

An independent 16-word init record at `0x200f0c10` starts with `0x494e4931`.
Word 1 is stage (2 configured/readback, 3 sampling); 2–5 are the four registers
before configuration; 6–9 after. Words 10–11 are last corrected µV/signed µA;
12 is DIAG_ALRT, 13 indicates valid correction, 14 reserved, 15 indicates the
sensor-reset test feature. Raw factory settings occupy 96 bytes at `0x200f0da0`.
These records end before the panic buffer at `0x200f0e00`.

Reproduce the native correction fixtures with the exact preserved stock dump:

```sh
uv run tools/native_calibration_vectors.py ORIGINAL_207.bin --output vectors.csv
```

The script checks its SHA-256 before using version-specific addresses. It runs
only synthetic RAM and bounded Unicorn emulation, with no device access.
