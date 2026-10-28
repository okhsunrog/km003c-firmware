# CDC telemetry and passive PPS reception

HW2.1, 2026-10-28, stock 2.0.7 reference; connected load was Pixel 8 Pro using
PPS, confirmed by the owner. The source and cable orientation were left unchanged
between stock and custom runs. `tested-image.bin` is the exact accepted image.

## Stock reference

The existing km003c-rs CLI captured the native firmware's decoded traffic and
raw PD header/payload bytes:

```sh
km003c --reset pd capture --duration 20 --raw
```

`stock-reference.log` ended with `Capture complete.` and exit status 0.
Two PPS cycles occurred at 141.681 and 150.888 device seconds: 9.207 s apart.
Each contained Request, GoodCRC, Accept, GoodCRC, PS_RDY, GoodCRC. The earlier
changing-load capture also showed a cluster of requests about 0.3 s apart before
settling near 9.2 s. Neither log includes initial Source_Capabilities, so the
CLI reports requested PDO/RDO without resolving the full negotiated profile.
`stock-registers.log` is a separate authenticated read while monitoring was enabled.
The first exploratory capture's USB disconnect on termination was excluded from
this accepted reference; repeated isolated capture after USB reset succeeded.

## Rust observations

Over 40 seconds, CDC delivered 160 ADC records with contiguous sequence numbers,
valid calibration and zero reported drops, plus three passive bursts. Each
burst had one CRC-valid SOP Request, complete raw chunks, and zero decoder
errors: 290, 294 and 292 intervals. Wire bytes were `8210269a0363`,
`8214269a0363` and `8216269a0363`. RDO `0x63039a26` selects PDO 6; the Message ID
changes between captures. PPS voltage/current changed over time, so payloads
need not equal the earlier stock RDO `0x6303a430` byte for byte.

`capture.ndjson` preserves the complete acquisition, raw intervals, packet CRCs,
ADC values and register snapshots. Host reassembly checked all raw chunk offsets
and total counts; standard CRC-32 independently matches all three packet CRCs.
`verification.json` records counts and artifact hashes. A separately captured
292-interval Request is a host regression fixture in the driver crate. All 17
host tests pass, including that fixture and prior malformed/CRC recovery tests.

Three corrections were needed for hardware acceptance:

- Overflow enable/flag is BCSTRH at byte offset `+0x81`, not `+0x82`.
- DMA live remaining count is MONDTCTL2 at `0x400530e8`; DTCTL2 at `0x400530c8`
  holds the programmed count. SDK `DMA_GetTransCount` reads MONDTCTL.
- Real edge groups are 15/19/23 and 35/39/43 ticks. Half-bit 20 with tolerance 5
  accepts the measured asymmetry while keeping timing windows disjoint. The
  first edge starts the timer and has no previous interval; decoding skips it.

Output/termination GPIOs and GoodCRC TX remained disabled. This proves reception
on PA0/CMP1_INP1 for the connected orientation, not the physical CC1/CC2 names.
The receiver is command-driven and single-burst. Raw processing/USB output leave
blind intervals; this is not a complete or lossless negotiation capture. The
stock reference demonstrates the fast responses that this stage can miss.

## Installation and recovery

The exact stock 2.0.7 backup had previously been read twice and matched its rollback
package. New image readback and bootloader comparison passed before execution.
The CDC `loader` command returned to native DFU with normal retained status;
Bootloader bytes `[4,0x4000)` remained unchanged. Stock 2.0.7 restoration
completed with full 456896-byte application readback.
`restore.log` records success. Final loader readback matched the original
SHA-256 `17d51eeff9fb6abe1a7024a108006248e0aaebdaae52de69a3e26c000565d4bf`.

See [CDC commands and limits](../../docs/cdc-telemetry.md).
