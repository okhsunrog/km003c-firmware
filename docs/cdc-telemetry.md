# CDC measurement and PD diagnostics

Build the line protocol instead of the original binary echo test:

```sh
cargo build --release --locked --features cdc-telemetry,pd-capture,manual-ui-test
```

The product string is `KM003C Rust telemetry`; USB endpoints remain OUT `0x01`,
IN `0x82`. Ordinary CDC serial clients can send ASCII commands terminated by
LF (CRLF accepted), and read newline-delimited JSON. A record may span multiple
USB packets. The host must assemble lines rather than parse USB packets as JSON.
No measurement stream starts before `stream on`. The separate default echo
image and `tools/usb_echo.py` remain available for USB transport regression tests.

| Command | Result |
| --- | --- |
| `status` | Reply with `km003c-cdc-v1`, also used for host resynchronization |
| `stream on` | INA229 and line-voltage records every 250 ms |
| `stream off` | Stop ADC records |
| `pd start 1` / `pd start 2` | Continuous interrupt-rearmed receive |
| `pd start auto` | Continuous receive, ADC-driven CC selection |
| `pd stop` | Stop continuous receive and publish counters |
| `pd 1` | One passive burst from CMP1_INP1 / PA0 |
| `pd 2` | One passive burst from CMP1_INP2 / PA1 |
| `pd attach 1` | With `pd-tx-test` only: unloaded CC1 sink attachment for five seconds |
| `pd getcaps` | With `pd-tx-test` only: one fixed Get_Source_Capabilities per lease |
| `pd sink 5/9/12/15/20` | With `pd-sink`: negotiate the selected fixed voltage on CC1 for up to 20 seconds |
| `loader` | Request native DFU and reset |

PD commands require `pd-capture`. On the tested HW2.1, inputs 1/2 correspond to
CC1/CC2 on PA0/PA1. Inputs become analog with outputs and pull-ups disabled. TX
outputs, VCOUT, CC termination and GoodCRC generation remain disabled in passive
builds. The separate [bounded CC1 TX probe](pd-transmit.md) requires `pd-tx-test`
and explicit attach/GetCaps commands. `pd stop` also releases its outputs. DAC code
`0x20` follows the native configuration. Only a command starts acquisition.

ADC records contain `seq`, device uptime `ms`, `uv`, signed `ua` (or null),
`mc`, `cal`, `diag` and cumulative `dropped`. Units are µV, µA and milli-Celsius;
`cal` indicates valid factory correction. These are the same snapshots as the
meter screen. Host USB stalls do not suspend measurement/UI tasks.

`lines` records carry `cc1_uv`, `cc2_uv`, `dp_uv`, `dm_uv`, `vdd_uv`, `raw`
(`[DR0,DR1,DR2,DR3,DR16]`), `cc_input` (0 ambiguous/unavailable, 1 or 2), `seq`
and `ms`. The line scale follows stock 2.0.7, independently of INA229 `cal`.
An unavailable conversion reports `line ADC unavailable`. See
[line acquisition, scale and automatic selection](line-adc.md).

A bounded eight-line queue serializes records. Measurement records are dropped
rather than blocking if it fills. Each line transfer has a timeout. A newline
resynchronizes after an interrupted line; the host ignores pre-status fragments
and blank lines, but reports malformed records after the status acknowledgement.
Explicit `stream off` avoids accumulating data when a libusb client closes
without a USB disconnect. Slow readers can lose records; `dropped` reports queue
and send failures, not a count of physical PD packets lost while unarmed.

## PD records and limits

Acquisition waits up to 15 seconds for the first burst. Pixel 8 Pro PPS requests
in the stock reference were about 9.2 seconds apart, so a one-second empty window
was insufficient evidence of a broken receiver. Timer overflow ends a burst
about 12.75 µs after the last edge at the configured 12 MHz timer clock.

- `pd_raw`: burst ID, input, total `count`, chunk `offset`, and `ticks` array.
- `pd_packet`: CRC-valid SOP, header, header/payload `wire_hex`, CRC and uptime.
- `pd_reset`: decoded Hard/Cable Reset ordered set.
- `pd_summary`: captured count, valid events and decode errors.
- `pd_capture_error`: timeout, DMA error, full buffer or count mismatch.
- `pd_registers`: post-cancellation snapshot in the order below.
- `pd_input`: selected comparator input, including automatic switches.

Legacy `pd_packet.ms` is processing uptime, **not a hardware wire timestamp**.
Buffered builds use `end_us`, IRQ-observed end uptime with 1 ms resolution.
The raw
first capture has no preceding edge and is excluded from decoding. Measured
interval groups were 15/19/23 and 35/39/43 ticks; half-bit 20 and tolerance 5
keep short/long windows disjoint and decode the measured Request with valid CRC.

The register snapshot contains CMP CTRL/VLTSEL/OUTMON, packed TMRA BCSTRL/H,
CNTER, CMPAR3, DCU DATA0, DMA MONDTCTL2, both AOS TMRA trigger selectors, and DMA
INTMASK1/INTCLR0. Reading these registers does not acknowledge capture flags.

The original `pd 1` / `pd 2` interface remains single-burst with blind intervals
while processing and uploading raw data. The additional
`pd start 1` / `pd start 2`
commands now use interrupt-rearmed double buffering, a separate priority decoder
and queued JSON formatting. See [buffered receiver](pd-buffered.md) for ownership,
loss counters, scheduling and accepted hardware results. This still does not
claim losslessness under arbitrary traffic, IRQ masking or host backpressure.

## Host recording

```sh
uv run tools/bench.py my-capture --pd auto
```

The bench wrapper defaults to a 30-second recording, discovers libusb (including
NixOS store paths), and stores NDJSON under
`~/tmp_zfs/codex_tmp/km003c-rust-bringup/bench/`. Override with
`LIBUSB_LIBRARY` and `KM003C_BENCH_ROOT`. `--pd` implies continuous capture;
`--seconds`, `--pause-read`, `--loader` and `--dry-run` are available. No flash
write is hidden in this helper. The lower-level `usb_telemetry.py` still accepts
explicit paths and single-burst options.

`uv run tools/bench.py trigger --sink 20 --seconds 10` runs an explicit active
sink session without passive/probe options. `pd_sink` records distinguish
`starting`, `request`, `contract_ready` (after PS_RDY), and `detached`.
`pd_tx` records preserve transmitted bytes plus DWT cycle timings and the four
decode phases documented in [PD transmission](pd-transmit.md). ADC records
provide the independent measured voltage. These commands change charger output;
the hardware acceptance tests used an unloaded charger, with Pixel disconnected.

`--pd-continuous --pd 1` starts buffered capture and stops it after recording.
`--pd-continuous --pd auto` uses automatic selection.
`--pause-read 10` deliberately delays host reads for a backpressure check.
`--cc1-probe attach` / `getcaps` explicitly exercise the five-second test lease;
they require `--pd 1` and at least seven seconds. With the attach probe,
`--pause-read 6` waits for an observed VBUS above 4 V before pausing reads,
records `bench_pause` on the host and tests expiry while CDC is stalled. Lost
or interrupted CDC records are expected under a long stall; malformed fragments
are counted and still make the recorder return an error.
`--repeat-pd` repeats the selected burst command every 16 seconds. `--loader`
returns to native DFU after recording. Supply `--libusb PATH` if the system
cannot discover its native library. The tool checks the product string, detaches
only attached interface drivers, claims CDC, sets DTR and releases/reattaches
interfaces afterward. It never flashes the device.

The stock reference was acquired with the existing `km003c-rs` CLI:

```sh
km003c --reset pd capture --duration 20 --raw
```

See [hardware evidence](../hardware/pd-cdc/README.md) for the reference and raw
interval regression fixture. No external transmitter or synthetic wave source
was used for that fixture.

With the additional `orientation` feature, `accel` records report SC7A20-family
XYZ values in mg and screen direction. See [orientation](orientation.md).
