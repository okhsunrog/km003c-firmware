# PD transmission and usbpd integration

The installed HW2.1 **stock 2.0.7** image is the primary source for this analysis,
updated 2026-10-29. Its software encodes PD frames, uses TimerA2/DMA1 to produce
BMC transitions, and uses CMP1/DAC1/timer capture for RX. No dedicated HC32 PD
peripheral or automatic hardware GoodCRC engine has been identified. The USB FS
PHY on PA11/PA12 is a separate peripheral.

The opt-in `pd-tx-test` feature now has a **hardware-tested CC1 transmitter**:
an unloaded charger acknowledges the fixed Get_Source_Capabilities packet with
GoodCRC. PB6 enables the CC1 sink attachment path. Pixel remains disconnected.
The `pd-sink` feature now runs the crates.io `usbpd` 2.0.0 sink stack with
software GoodCRC, message IDs, retries and Request/Accept/PS_RDY negotiation.
Fixed 5/9/12/15/20 V contracts were accepted with an unloaded charger. CC2 TX,
PPS and EPR policy are not implemented. Passive monitoring stays separate.

## Hardware-tested probe

Build with `--features pd-tx-test,orientation,manual-ui-test`. Nothing drives CC
until an explicit CDC command. `pd attach 1` arms RX on CC1, then drives PB6 high
for five seconds. The command requires a fresh line sample, unloaded VBUS below
5.5 V, CC1 above 2.5 V and CC2 below 0.4 V. These preconditions match the tested
charger's detached state; they are not a general Type-C attach algorithm.

`pd getcaps` permits exactly one SOP Get_Source_Capabilities packet (`87 00`,
PD3.0 sink/UFP, MessageID 0) during that lease. It requires CC1 in 0.4..2.15 V,
no active XTAL32 on PC14/15 and an idle receiver. There are no arbitrary-frame,
Request, EPR or reset commands. This probe cannot select a supply voltage.

TX uses a dedicated static buffer, DMA1 CH1 and TimerA2 PA0 AF4. The DMA IRQ
holds the final interval; the following overflow IRQ disables OUTEN/PC14,
restores PA0 analog mode and rearms RX immediately. ADC1's watchdog stays off.
IRQ transitions and SysTick cleanup share a critical section. A five-millisecond
TX deadline and the five-second CC lease are enforced independently of the
capture executor and CDC. `pd stop`, actual USB disconnect and fault/reset also
release the controls. Dropping a host interface alone need not cause a USB
disconnect, so the independent lease is essential. DMA storage stays valid
even on failure; another transfer cannot reuse it without proving CH1 stopped.

The hardware test observed CC1 changing from about 3.18 V to 1.63 V and VBUS
rising to about 4.99 V. The charger returned CRC-valid SOP GoodCRC `a1 01`,
MessageID 0, in each TX run. PB6 low restored the detached state and VBUS near
zero. Source Capabilities retry because this probe does not acknowledge them or
request a contract. ACK reception establishes a working transmitter with this
charger; it does not measure analog impedance, BMC timing compliance or
microsecond turnaround. Captured `end_us` still has 1 ms resolution.

[Captures, image identity, counters and recovery evidence](../hardware/pd-sink/README.md)
include a host-read stall across lease expiry. Forced CDC backpressure can drop
records and leave an interrupted line; it is separate from DMA capture loss.

```sh
uv run tools/bench.py attach --pd 1 --seconds 8 --cc1-probe attach
uv run tools/bench.py tx --pd 1 --seconds 8 --cc1-probe getcaps
```

## Verified 2.0.7 code

[Address mapping](../analysis/pd207/address-map.json) preserves the catalog 1.9.9
and installed 2.0.7 locations. These are runtime addresses. The newer control
flow and register operations were checked; matching a short byte prefix alone
was insufficient to establish a function identity.

| 2.0.7 address | Function |
| --- | --- |
| `0x9268` | Create `PD_PHY` task and initialize hardware |
| `0x934e` | Alternate RX buffers and rearm capture |
| `0x9430` | Encode a frame, switch to TX, wait and return to RX |
| `0xf86e` / `0xf976` | Select CMP input / PA0 or PA1 AF4 output |
| `0xfa8c` | Load first period and start DMA1 CH1 / TimerA2 |
| `0xfb10` | TX DMA completion: disable CH1, hold output, arm overflow IRQ |
| `0xfb46` | Select RX or TX hardware mode and overflow callback |
| `0xfbde` | Initialize timer, comparator, DAC, DMA and IRQ routing |
| `0xfff2` | Final TX timer overflow: stop timer and signal completion |
| `0x1ed58` | PHY task: decode RX, construct GoodCRC and dispatch TX/reset |
| `0x1fff8184` | RAM-resident packet-to-period encoder |

The native RX buffers are at `0x200111e8` and `0x20012866`, with **2860** halfword
intervals per capture (`0xb2c`) and metadata at byte offset `0x1658`. The older
catalog used 860. Our accepted receiver still uses 860; that covers the tested
ordinary/chunked frames and does not claim support for arbitrary long frames.

[Native C/assembly exports and provenance](../analysis/pd207/README.md) distinguish
instruction evidence from inferred decompiler types and signatures.

## TX, completion and errors

The 2.0.7 sequence is:

1. Check the object's TX-enable flag; otherwise return status 10.
2. Check CMP1 output for carrier availability (or bypass in native mode 5).
   The busy path retries three times with a delay-helper argument of 10; it
   returns status 9 if the line never becomes available. Delay units were not
   established by this analysis.
3. Encode into the current native DMA-buffer storage using the RAM encoder.
4. Disable ADC1 analog watchdog through `0x52c0(0)`.
5. Switch to TX: mask CMP edge events, stop RX timer start conditions, set
   selected PCONR.PERC=3 (invert at period), enable OUTEN and clear TX CHEN.
6. Drive selected PC14/PC15 low with its output enabled, through `0x40958`.
7. Preload PERAR=`buffer[0]`, SAR1=`buffer+1`, DMA count=`count-1`; start CH1 and
   TimerA2. Timer overflow event `0x103` supplies each following PERAR value.
8. DMA completion runs `0xfb10`: disable CH1, set PERC=2 (**hold**, not low), and
   enable the timer overflow interrupt. The last loaded interval still has to
   elapse. The vendor DDL calls these output modes `TMRA_PWM_INVT` and
   `TMRA_PWM_HOLD`.
9. The final overflow runs `0xfff2`, stops TimerA2 and signals the PHY event.
10. The waiting TX function disables timer OUTEN (`0xf9c4`), disables the selected
    PC14/PC15 output (`0x4093e`), rearms RX and enables the ADC watchdog again.

The event wait has timeout argument 5. On a nonzero wait result, status 9 is
returned, but the same output-disable/RX-rearm path still runs. The unit of this
RTOS argument is not assumed here. This caller does not explicitly quiesce TX
DMA on its timeout branch; normal DMA completion does disable it. That is not a
proof that DMA is settled after every abnormal exit.

A Rust driver must therefore wait for **wire completion**, not just DMA-done,
and must stop event producers and settle both DMA channels before recycling
memory on cancellation/failure. A dedicated TX buffer avoids the native shared
RX/TX storage arrangement. The current application owns ADC1 separately and
leaves its watchdog off: it must not blindly copy the stock unconditional
watchdog re-enable.

## Electrical controls and what remains unknown

Hardware tests establish PA0/CMP1_INP1 as CC1 and PA1/CMP1_INP2 as CC2 on this
unit. GPIO functions below are verified software controls. The diagram's analog
block is intentionally unresolved; it is not a traced schematic.

```mermaid
flowchart LR
  CC[CC1 / CC2 at tester] <--> A[External analog network: untraced]
  A <--> PA[PA0 / PA1: ADC and TimerA AF4]
  PA --> RX[CMP1 + DAC1: RX]
  TX[TimerA2 + DMA1 CH1: TX] --> PA
  PC[PC14 / PC15 latch, output-enable, pull-up] -.-> A
  SEL[PB6 and AW9523 P0_3 / P1_6] -.-> A
```

| 2.0.7 helper | Verified action |
| --- | --- |
| `0x4093e(selector)` | Disable PC14/PC15 output for selector 1/2 |
| `0x40958(selector)` | Selected PC14/PC15 latch low, output enabled |
| `0x40980(selector)` | Selected PC14/PC15 latch high, output enabled |
| `0x409a6()` | Release P0_3, set P1_6 latch high, disable both PC outputs |
| `0x40a78()` | Clear PC14/15 DDIS and output-enable; enable PB6 output |
| `0x40ab0(selector)` | Select PA0/1 AF4 and matching comparator input |
| `0x40ae0(selector)` | 1: PB6 low; 2: P0_3 high-Z; other: both |
| `0x40b12(mode)` | 0: PB6 low + P0_3 high-Z; 1: PB6 high; 2: P0_3 low; 3: PB6 high + P0_3 low |
| `0x40b58(mode)` | PC14/PC15 pull-ups: 0=off/off, 1=on/off, 2=off/on, 3=on/on |

AW9523 helper `0x1a5ae(mask,state)` changes OUTPUT0 and CONFIG0. State 0 sets the
configuration bit (input/high-Z), state 1 latches high then enables output,
other states latch low then enable output. Mask `0x08` is P0_3. `0x1a63c` changes
OUTPUT1 only: state 1 high, others low; mask `0x40` is P1_6. Its direction is not
set by that helper.

Composite helper `0x409c8(1)` enables PC14 pull-up, drives PC15 high/enabled,
latches P0_3 high/enabled **and sets PB6 high**. The older corresponding helper
lacked that PB6 write. For selector 2, it enables PC15 pull-up, drives PC14
high/enabled, latches P1_6 low and P0_3 low/enabled. These sequences are why
PC14/PC15 cannot be treated as simple independent TX-enable pins.

The firmware and CC1 probe establish that these signals participate in CC
attachment/role and transmit behavior. PB6 high has now enabled sink attachment
on CC1 with an unloaded charger. This does not prove transistor topology,
resistor values, active polarity at a hidden switch, direct CC continuity,
VCONN routing, or the
physical implementation of the lightning button's startup pulldown. Labeling
P0_3 as a hardware-confirmed CC2 Rd control or PC14/15 pull-ups as confirmed Rp
resistors would still go beyond this evidence. Continuity/component tracing
and analog level/impedance measurements remain necessary to finish the schematic.

## Encoding verified without the device

`pd::tx::encode` is allocation-free and writes only a caller-provided RAM slice.
It produces the native preamble, all five SOP ordered sets, low/high-nibble
4b5b data, reflected CRC-32 in wire order, EOP, and native even-halfword padding.
Hard/Cable Reset use their native shorter path without data CRC/EOP.
Length/timing/capacity failures leave output untouched. Wire chunks are limited
to a two-byte header plus up to seven four-byte data objects.

The stock RAM encoder was recovered by executing its actual scatter-load and
decompression stage in Unicorn, stopping before main. Its native CRC32 init
sets `0x40008c00=0x1e`; CRC MMIO was modeled with independent zlib CRC32. Timer,
DMA, analog circuitry, interrupts and electrical response times were not modeled.
The Rust encoder matches **162 full native reload vectors**: all SOP variants,
0..7 data objects, four payload patterns, and both reset ordered sets. The
independent RX decoder also recovers each packet/reset with CRC checks.

Native reloads are 19 and 38 at the current timer setup; these are PERAR values,
not a measurement of edge-to-edge timing. Inclusive up-counting and live reload
behavior must be accounted for and checked electrically before TX acceptance.
Matching the waveform words does not establish amplitude, impedance or timing
on CC.

```sh
uv run tools/native_pd_vectors.py --output native-pd207.tsv
cargo test --manifest-path crates/km003c-drivers/Cargo.toml --locked \
  --all-features --target x86_64-unknown-linux-gnu
```

`KM003C_STOCK_207` overrides the script's cached default input path. It refuses
any binary whose SHA-256 differs from the preserved installed 2.0.7 image.

## Running the usbpd sink

`usbpd` **2.0.0 from crates.io** is an optional `km003c-drivers` dependency.
The offline `pd_decode` example uses it with `--features usbpd` for SOP semantic
parsing; cable-addressed SOP′/SOP″ traffic is kept distinct. This already builds
for the HC32 target as well as the host. No git fork or main dependency is used.

The board implements `usbpd_traits::Driver` with
`HAS_AUTO_GOOD_CRC=false` and `HAS_AUTO_RETRY=false`. Native task `0x1ed58` also
constructs a two-byte GoodCRC in software, then calls the normal TX function;
this is conditional on the native mode/state flags. It is not evidence of
automatic hardware acknowledgement, or a reason to acknowledge packets while
operating as a passive monitor.

The adapter uses these ownership boundaries:

- One owner for the RX/TX timer, CMP/DAC, DMA1 CH1/2, mux and frame buffers.
- `receive`: delivers only CRC-valid SOP header/data bytes, excluding PHY framing
  and CRC; surfaces Hard Reset. Cable traffic stays in diagnostics separately.
- `transmit`: encodes into permanently owned TX storage, checks carrier, switches
  RX→TX, awaits final timer completion, releases outputs and restores RX. DMA
  must settle before memory reuse, including cancellation/error paths.
- Fast receive/protocol work runs outside thread-mode UI/JSON formatting. Software
  GoodCRC must not depend on CDC queues or the 20 ms ADC polling interval.
- `wait_for_vbus` uses the INA229 measurement, while CC attach/termination policy
  is separately owned. Automatic receiver mux changes must not race a TX frame.
- An explicit DPM selects an advertised fixed PDO at 100 mA, falling back to
  5 V when unavailable. It does not use the crate's highest-current default.

```sh
cargo build --release --locked --features pd-sink,orientation,manual-ui-test
uv run tools/bench.py trigger-20v --sink 20 --seconds 10
```

CDC accepts `pd sink 5`, `9`, `12`, `15` or `20`. Each explicit session starts
on detached CC1, runs for up to 20 seconds and releases PB6 on stop/cancellation.
An independent 30-second CC lease still bounds a stalled task. No session starts
on USB enumeration. `pd stop`, Exit and `loader` retain recovery behavior.

## Timing and profiling

The 1 kHz HAL time driver cannot supply the crate's one-millisecond CRCReceive
timeout directly: its rounded-down `now()` can make a one-tick wait end almost
immediately. Only that short deadline uses a rescheduled DWT poll lasting a full
192,000 CPU cycles. Other timers remain async Embassy timers. This bounded poll
can occupy the decoder executor for up to one millisecond; a hardware-alarm time
backend is the intended replacement. A 32,768 Hz time base is sufficient for
these protocol timers; changing the feature alone does not fix the driver, and
does not require 32,768 interrupts per second.

`DEMCR.TRCENA` and `DWT_CTRL.CYCCNTENA` enable profiling. `end_cycles` is recorded
when the RX IRQ ends a frame; `rx_to_tx_cycles` stops immediately before starting
the TX timer. Use wrapping subtraction and divide cycles by 192 for nominal µs.
The RX mark follows the approximately 12.75 µs quiet gap, so this is not itself
an external EOP timestamp. Captures preserve both cycle counts and the unchanged
one-millisecond-resolution `end_us` field.

The completed-burst decoder keeps symbol state in local variables; the separate
streaming decoder remains available. It validates CRC before delivery and is
cross-checked against all 162 native vectors and a captured Pixel Request.
Its hot loop runs in zero-wait SRAMH. Startup follows DDL 3.3.0's 200 MHz example:
SRAMH zero waits, SRAM12/3/R one wait, flash five waits with cache. Linker symbols
and a build assertion protect cortex-m-rt's normal data-copy boundary.

`decode_phase_cycles` contains ordered-set search, payload BMC/4b5b plus shape
validation, CRC and packet assembly/return times. With the selected scalar
Thumb-2 decoder, six-PDO Source Capabilities measured about
10.2 / 28.9 / 2.6 / 1.8 µs respectively, with about 49.8 µs in the instrumented
driver path. GoodCRC TX starts about 102..104 µs after the RX mark (about
115..117 µs including the quiet gap), below the 195 µs tTransmit limit in these
runs. The preceding portable burst implementation measured about 74 µs and
127 µs respectively; its evidence remains in `hardware/pd-sink`.
This is software
instrumentation, not an electrical compliance measurement. See the
[onsemi timing table](https://www.onsemi.com/pdf/datasheet/fusb302b-d.pdf), page 16;
that page was extracted using the user's `mistral_ocr --pages 16` CLI.

The [decoder comparison](pd-decoder.md) measured 18 complete implementations,
including two-lane `UQSUB16` and four-lane `UQSUB8`/`USUB8` kernels. Packing,
classification and dispatch outweighed their SIMD benefit on these fixtures.
The accepted path uses scalar Thumb-2, unrolled BMC/4b5b and confirmed alternating
preamble skipping. Five runs of 128 measurements each reduced Source
Capabilities from 73.36 to 45.17 µs (complete decode, CRC included, profiling
disabled). [New evidence](../hardware/pd-optimization/README.md) also records
renewed 5/9/12/15/20 V contracts and preserved recovery; the timer backend has
not changed.

Sources: [published driver contract](https://docs.rs/usbpd-traits/2.0.0/usbpd_traits/trait.Driver.html),
[published sink policy](https://docs.rs/usbpd/2.0.0/usbpd/sink/device_policy_manager/trait.DevicePolicyManager.html),
[upstream repository](https://github.com/elagil/usbpd). Upstream main was checked
at `b21607ad8b960da56ae801c5e42d9a86485a1f81`; its package declares 3.0.0, but
this integration deliberately uses the published 2.0.0.
