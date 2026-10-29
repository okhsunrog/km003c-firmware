# Active CC2 and automatic orientation acceptance

KM003C HW2.1, HCLK 192 MHz, `pd-bench,orientation`, crates.io `usbpd` 2.0.0.
Charger connected without Pixel or another load; HID supplies the host link.

The new image negotiated 20 V on CC1. During one later session, the owner
flipped the charger connection twice. Without another host PD command, the
firmware released CC1, selected CC2 from ADC, negotiated 20 V, then repeated
the sequence back to CC1. Each attachment created a new sink/protocol state.
Received Source Capabilities, source GoodCRC, Accept and PS_RDY and transmitted
sink GoodCRC/Request bytes are preserved in [reorient-20v.ndjson](reorient-20v.ndjson).
The earlier [cc1-20v.ndjson](cc1-20v.ndjson) records the initial regression test.

CC2 uses PA1 AF4, TimerA2 PWM channel 2, PC15 gate and AW9523 P0_3 low/output
for Rd. CC1 retains PA0/channel 1/PC14 and PB6 high. CC2 release uses expander
input/high-Z with register readback. The switching events record `released`,
`reorienting`, `waiting` and `attached`; line ADC independently shows the active
CC moving between about 1.63 V and zero. This validates the software controls
on this board, not resistor values or a complete traced schematic.

All three attachments reached `contract_ready` at 20 V; INA229 measured about
19.98 V. Accelerometer records remained error-free, including rotation changes
while AW9523 was used for CC2. There were no malformed host records or reported
telemetry queue drops. Decoder/DMA/stop counters and measured voltage summaries
are in [verification.json](verification.json). Successful negotiations do not
establish lossless capture or USB-PD electrical compliance.

| Attachment | Measured VBUS, final-five-sample median | Active CC median |
| --- | --- | --- |
| CC1, initial | 19.979290 V | 1.630636 V |
| CC2, flipped | 19.981244 V | 1.627778 V |
| CC1, returned | 19.980462 V | 1.629818 V |

Source-Capabilities decode took 49.90 µs in the instrumented driver. GoodCRC
TX started 106.85..107.16 µs after the RX IRQ's end mark, about 119.6..119.9 µs
including the configured quiet gap. These are nominal 192 MHz DWT measurements,
not externally measured CC timing. Final counters retain six `overruns` from
capture slot pressure; DMA, count mismatch, stop, decode and sync errors were
zero. The run does not claim losslessness.

After explicit stop, [after-stop.ndjson](after-stop.ndjson) shows VBUS below
1.5 mV, CC1 above 3.178 V and CC2 at zero, while the custom application remains
running. The orientation capture contains 1162 ADC, line and accelerometer
records each, 15 decoded packets and 12 completed transmissions.

One earlier image failed during startup: synchronous USB settling delays ran
after an async AW9523 operation had begun, exhausting its new 30 ms outer bound.
It panicked before active PD commands and returned to the preserved loader.
The accepted image finishes synchronous USB hardware startup before polling
tasks. Its application readback matched the package and the native loader was
unchanged. [hardware-run.log](hardware-run.log) records that verification and a
normal CDC-requested loader return; [marker-after.bin](marker-after.bin) has
status `0x600d0002`. The same installed custom image was restarted without flash
writes for the orientation session. No stock reinstall was performed.

[tested-image.bin](tested-image.bin) is the accepted full application readback.
A final rebuild after comment-only edits matched the original raw image exactly.
[recovery.ndjson](recovery.ndjson) preserves live ADC/accelerometer data before
the explicit loader request; [retention.log](retention.log) preserves the end of
the trace ring. [SHA256SUMS](SHA256SUMS) covers the evidence.

PPS/EPR negotiation, loaded power operation, forced I2C/DMA failure and physical
CC impedance/timing measurements remain outside this run. In particular, a
panic can stop direct TX but cannot await the external CC2 Rd write. Normal
stop/reset paths await it; persistent I2C failure is reported, not claimed as a
successful detach. See [implementation and limits](../../docs/pd-transmit.md).
