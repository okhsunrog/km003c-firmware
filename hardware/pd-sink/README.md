# CC1 usbpd sink acceptance

KM003C HW2.1, 192 MHz, unloaded PD charger, Pixel disconnected. The same
`pd-sink,orientation,manual-ui-test` image negotiated all five fixed PDOs through
crates.io `usbpd` 2.0.0. Each capture contains one Request, Accept and PS_RDY,
with software GoodCRC, followed by an explicit stop/detach. No hard/soft reset
or repeated Request occurred in these five sessions.

| Requested | INA229 median of final five samples | Capture |
| --- | --- | --- |
| 5 V | 4.989447 V | [5v.ndjson](5v.ndjson) |
| 9 V | 8.989837 V | [9v.ndjson](9v.ndjson) |
| 12 V | 11.984369 V | [12v.ndjson](12v.ndjson) |
| 15 V | 14.982611 V | [15v.ndjson](15v.ndjson) |
| 20 V | 19.981244 V | [20v.ndjson](20v.ndjson) |

The policy requested 100 mA; no external load/current-capacity test was performed.
The source advertises five fixed 3 A PDOs and a 5..21 V PPS APDO. This acceptance
uses fixed PDOs only. CC2 TX and PPS/EPR policy are separate work.

## Timings and scope

DWT_CYCCNT at nominal 192 MHz measured about 127 µs from the RX completion mark
to GoodCRC TX start for Source Capabilities, and 83..85 µs for Accept/PS_RDY.
The mark follows a 12.75 µs quiet gap: the estimated EOP-to-TX interval is about
140 µs for Source Capabilities, versus the 195 µs limit. These are internal
cycle measurements, not oscilloscope measurements or a compliance test.

| Source Capabilities decode phase | Typical µs |
| --- | --- |
| Preamble / ordered-set search | 19.27 |
| Payload BMC / 4b5b | 45.90 |
| CRC verification | 2.75 |
| Packet assembly / return | 1.40 |

Driver bookkeeping brings the measured decode path to about 74 µs. Early builds
took about 450 µs. The final completed-burst decoder keeps local state, runs
in zero-wait SRAMH and passes the 162 native vectors plus the real Pixel Request
fixture. The one-millisecond CRCReceive timeout uses DWT rather than the current
1 kHz HAL clock, whose rounding caused premature retries. A hardware-alarm
Embassy time backend remains open; 32,768 Hz would be sufficient for protocol
timers. No DSP assembly is enabled in this image.

The captures record nonzero pool overruns during startup/attachment and VBUS
wait. They establish successful negotiation, not lossless startup capture.
There were no DMA errors, count mismatches or DMA-stop faults. The existing
`decode_errors`/`sync_errors` counters belong to the passive streaming decoder;
they do not count fast-sink burst rejections.

## Image and recovery

[verification.json](verification.json) contains measured ranges, cycle timings,
counter snapshots and hashes. [tested-image.bin](tested-image.bin) is the actual
installed/read-back application; the loader commits size/CRC at offsets
`0x428..0x42f`, so it differs there from the raw linked binary.
[hardware-run.log](hardware-run.log) records package verification, full custom
image readback and unchanged loader bytes before launch, then a successful
return to the original loader and another loader comparison.
[marker-after.bin](marker-after.bin) records the normal recovery status.
[SHA256SUMS](SHA256SUMS) covers this evidence.

The verified stock 2.0.7 rollback was reused from the existing backup. Stock was
not reinstalled after the tests; the custom application remains installed.
The initial SRAMH linker experiment hit HardFault before main and returned to
the native loader without a cable/power intervention. The final linker script
asserts the CRT data-copy boundary to catch that placement error during build.

```sh
uv run tools/bench.py trigger-20v --sink 20 --seconds 5
```

Only run active trigger tests with the intended load arrangement. The preserved
tests used the charger alone, keeping HID connected to the computer.
