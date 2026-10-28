# Line ADC and automatic CC acceptance

2026-10-28, KM003C HW2.1, Pixel 8 Pro charging through the main connectors, side
HID connected to the computer. Only the Pixel cable plug **at the tester** was
turned; its phone end and charger side stayed unchanged. The original orientation
uses CC1/PA0; the flipped one uses CC2/PA1 on this setup.

## Preserved reference

`stock-original.csv` and `stock-flipped.csv` each contain 40 stock 2.0.7 samples.
They were collected with `km003c-lib::request_adc_data` and authenticated memory
reads through that same library. Columns preserve **D+, D−, CC1, CC2, VDD** in mV,
then ADC1 DR0..16 counts. Each protocol sample precedes its register read: these
are sequential measurements, not simultaneous snapshots. `stock-adc1.bin` is the
0x128-byte block at `0x40040000` in the original orientation.

Stock plaintext SHA-256:
`c63057eaf08b8625e7ac2fc7270c06d75515e78fdbdd46080bdf4b696984ec67`.
The nominal 1080 mV scale and the D+/D− control override are traced to this
version's instructions, not copied from the older 1.9.9 image.
See [registers and formula](../../docs/line-adc.md).

## Comparison

`custom-original.ndjson` preserves 48 line samples (12 seconds).
`custom-auto-switch.ndjson` preserves 361 line samples over 90 seconds, alongside
PD, INA229 and accelerometer records. The comparison tool excludes the first
two seconds of each stable visit to the selected input. All values below are mV;
the reference means use 40 samples, custom means use 40 original/101 flipped.

| Signal/setup | Stock mean | Rust mean | Rust − stock |
| --- | ---: | ---: | ---: |
| D+, original | 596.193 | 594.756 | −1.436 |
| D+, flipped | 596.393 | 594.484 | −1.909 |
| D−, both | 0 | 0 | 0 |
| CC1, original | 1661.808 | 1659.989 | −1.818 |
| CC2, flipped | 1653.595 | 1651.787 | −1.808 |
| Inactive CC, both | 0 | 0 | 0 |
| VDD estimate, original | 3293.073 | 3355.658 | +62.586 |
| VDD estimate, flipped | 3293.373 | 3354.645 | +61.272 |

These are observed differences across firmware runs, not absolute accuracy.
They do not establish equal resistor paths, ADC gain accuracy over range, or
nonzero D− accuracy. No fitted per-channel offsets were introduced. Stock uses
longer software averaging; Rust currently publishes individual hardware-averaged
scans. Custom CC standard deviation is about 2.16 mV and D+ about 0.69/0.67 mV.
The 61..63 mV VDD-estimate difference remains unexplained; different reference
sampling and board load are possible candidates, not established causes. An
external supply/reference measurement and a nonzero D− point remain necessary
before claiming calibration accuracy. The raw counts and VDD are kept so these
questions can be investigated without losing the original evidence.

Reproduce the summaries:

```sh
uv run tools/compare_lines.py hardware/line-adc/custom-original.ndjson
uv run tools/compare_lines.py hardware/line-adc/custom-auto-switch.ndjson \
  --input 2
```

## PD and recovery

During the 90-second capture the owner flipped the tester plug and flipped it
back. Selection records are `1 → 2 → 1`; both visits after insertion produced
24 CRC-valid packets, 48 total. Final counters: 376 frames,
328 short/noise bursts,
zero overrun/held-overrun, full buffer, DMA error, mismatch or stop fault, two
input switches, zero switch discards and zero post-SOP decode errors.
There are 34 synchronization rejects before packet lock around insertion;
these are preserved rather than counted as decoded packet errors. Maximum
IRQ-handler/rearm work is 730 CPU cycles (3.80 µs at 192 MHz), not IRQ latency.
The owner confirmed all four voltage fields were visible and charging continued.
No new button test was requested in this run.
The CDC `dropped` counter was already 3 at the first synchronized sample and
stayed 3 throughout the 90-second recording; no new output drops were counted.

The custom application was written once after the requested stock-reference
restore. Application readback matched and bootloader bytes `4..0x3fff` were
unchanged before/after programming and after reset. `marker-after.bin` records
`RUST, 1, 0x4000, 0x600d0002`: an explicit CDC loader command returned to native
DFU normally. The same image was restarted and CDC/measurements worked again,
without another flash write. Custom firmware remains installed; stock was not
automatically restored.

Application binary SHA-256:
`6680b8db77a77eec9986bff088b037babca9e06f9f6172023ae29a2386c6228a`.
Bootloader SHA-256:
`17d51eeff9fb6abe1a7024a108006248e0aaebdaae52de69a3e26c000565d4bf`.
Build features: `cdc-telemetry,pd-capture,orientation,manual-ui-test`.
Local checks: formatting, clippy with warnings denied, 24 driver tests.
After hardware testing, a feature guard was added to the telemetry-only CC
accessor so measurement-only builds also pass clippy. Rebuilding the tested
feature set changes only three source-line diagnostic bytes by +1; acquisition
and UI code are otherwise byte-identical. That diagnostics-only change was not
flashed again.
