# Interrupt-rearmed double-buffer PD acceptance

KM003C HW2.1, Pixel 8 Pro PPS, 2026-10-28. HCLK 192 MHz, PCLK1 96 MHz,
TimerA2 capture clock 12 MHz. Source and sink generate PD traffic; custom TX,
termination control and GoodCRC transmission remain disabled.

## Accepted image

`tested-image.bin` uses two permanent DMA buffers, packet-gap IRQ rearm,
a priority-7 decoder, and a separate thread task for JSON formatting.
It stages completed intervals and releases their DMA slot before decoding.
Retained logging no longer formats entire lines with interrupts masked.

The 40-second normal record delivered exactly 36 independently CRC-verified
packets, in six complete ordered cycles:

```text
Request -> GoodCRC -> Accept -> GoodCRC -> PS_RDY -> GoodCRC
```

Final counters: 36 captures; zero overruns, full buffers, DMA errors, count
mismatches, quiescence faults or post-SOP decode errors. CDC output loss was zero.
All 161 ADC records remained calibrated; 161 acceleration records were received.
There were 18 rejected pre-SOP timing intervals (`sync_errors`), with every burst
still yielding its CRC-valid packet. This is distinct from packet decode failure.

Maximum measured handler/rearm portion: 725 DWT cycles, about 3.78 µs at 192 MHz.
This does not measure latency before handler entry. Packet-end uptime is currently
quantized to 1 ms; it cannot resolve sub-millisecond GoodCRC response timing.

## Host backpressure

A separate 25-second record deliberately read no USB IN data for its first
10 seconds. DMA completed 24 bursts with zero overruns or reported post-SOP decode
errors.
Only 12 PD packets reached the host; the cumulative output-drop counter rose
by 96 records, including measurements and other reports. Thus acquisition was
independent of host draining, while finite output buffering visibly lost data.
There were no malformed JSON records. This is not an unconditional lossless
capture claim. Both sessions acknowledged `pd stop`.

## Earlier iterations

`initial-loss.ndjson` records the first double-buffer implementation: it could
receive complete cycles but lost 6 of 210 captured bursts. It still formatted
JSON while holding the decoder's DMA slot. `intermediate-loss.ndjson` moved
formatting out, but lost 2 of 47 bursts while retaining a slot during decoding;
some frames also contained decode errors. These are failed intermediate checks,
not the accepted image. Bounded trace critical sections and early slot release
are both part of the accepted build.

## Recovery and reproducibility

New image readback and comparison with the preserved loader passed before
execution. CDC `loader` returned normally; loader bytes `[4,0x4000)` still matched
the verified original. The existing custom image was then restarted without a
flash write. Stock was not reinstalled. The development image retains its
10-minute automatic return to DFU, plus physical Exit and the CDC loader command.

`pixel.ndjson`, `backpressure.ndjson`, hardware log, retained marker and trace
preserve the evidence. `verification.json` records counters and artifact hashes.
Host analysis uses `usbpdpy.PdDecoder` and an independent CRC-32 check. The earlier
[stock CLI reference](../pd-cdc/stock-reference.log) contains the same six-message
cycle. No stock reflash was necessary for this stage.

All 22 driver tests pass, including stalled-reader/alternation ownership tests
and the real interval fixture. Clippy passed default, PD-only, telemetry-only and
all-feature builds. Fault counters exist, but electrical fault injection and
arbitrary traffic/interrupt starvation are not hardware-accepted here.
See [buffering design and limits](../../docs/pd-buffered.md).
