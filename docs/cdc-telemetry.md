# CDC measurement and passive PD diagnostics

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
| `stream on` | ADC records every 250 ms |
| `stream off` | Stop ADC records |
| `pd 1` | One passive burst from CMP1_INP1 / PA0 |
| `pd 2` | One passive burst from CMP1_INP2 / PA1 |
| `loader` | Request native DFU and reset |

PD commands require `pd-capture`. Input numbers do not assign physical CC1/CC2
connector names. Inputs become analog with outputs and pull-ups disabled. TX
outputs, VCOUT, CC termination and GoodCRC generation remain disabled. DAC code
`0x20` follows the native configuration. Only a command starts acquisition.

ADC records contain `seq`, device uptime `ms`, `uv`, signed `ua` (or null),
`mc`, `cal`, `diag` and cumulative `dropped`. Units are µV, µA and milli-Celsius;
`cal` indicates valid factory correction. These are the same snapshots as the
meter screen. Host USB stalls do not suspend measurement/UI tasks.

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

`pd_packet.ms` is processing uptime, **not a hardware wire timestamp**. The raw
first capture has no preceding edge and is excluded from decoding. Measured
interval groups were 15/19/23 and 35/39/43 ticks; half-bit 20 and tolerance 5
keep short/long windows disjoint and decode the measured Request with valid CRC.

The register snapshot contains CMP CTRL/VLTSEL/OUTMON, packed TMRA BCSTRL/H,
CNTER, CMPAR3, DCU DATA0, DMA MONDTCTL2, both AOS TMRA trigger selectors, and DMA
INTMASK1/INTCLR0. Reading these registers does not acknowledge capture flags.

This remains a **single-burst receiver with blind intervals**. Processing and
raw USB output occur before rearming; GoodCRC/Accept/PS_RDY can therefore be
missed even when the first PPS Request passes CRC. Repeated commands are not a
continuous lossless capture. A buffered acquisition/processing split is the next
step. The current observation confirms PA0/CMP1_INP1 carries PD on the connected
cable orientation; it does not establish the other orientation's physical map.

## Host recording

```sh
uv run tools/usb_telemetry.py --seconds 40 --pd 1 --repeat-pd \
  --output capture.ndjson
```

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
