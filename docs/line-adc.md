# D+, D− and CC voltages

Hardware acceptance on KM003C HW2.1, stock reference 2.0.7, 2026-10-28.
The `measurement` feature starts a separate internal ADC1 task alongside INA229.
These are passive voltage readings. No CC termination, PD transmitter, QC output
or extra GPIO pull-up is enabled by this task.

| Signal | MCU pad | ADC1 data register |
| --- | --- | --- |
| CC1 | PA0 | DR0, `0x40040050` |
| CC2 | PA1 | DR1, `0x40040052` |
| D+ | PA2 | DR2, `0x40040054` |
| D− | PA3 | DR3, `0x40040056` |
| Internal reference | Internal | DR16, `0x40040070` |

Stock snapshots preserve CHMUXR0=`0x3210`, CHMUXR1=`0x7654`, CHMUXR2=`0xba98`,
CHMUXR3=`0xfedc`: the line channels use their default pin mapping. Flipping only
the cable plug at the tester moves the nonzero CC voltage from DR0 to DR1.
Each corresponding comparator input independently receives CRC-valid PD frames.

## Scale in the installed 2.0.7 firmware

The native ADC task at `0x1de5c` reads enabled DR0..3 and the internal reference.
At `0x1e160..0x1e1ac`, its instantaneous calculation is:

```text
VDD_mV = floor(1080 * 4096 / ADCref)
line_mV = floor(ADCline * VDD_mV / 4096)
```

All four inputs share this scale. There is no distinct D+/D− or CC1/CC2 gain or
offset in the passive conversion loop. `0x5552` uses the same `0x438000` constant
when calculating analog-watchdog thresholds. Catalog 1.9.9 used 1062 mV; this
implementation follows 2.0.7. Installed 2.0.7 evidence takes precedence over the
older catalog image when they disagree.

For the longer average, the native task accumulates 250 scans. At
`0x1e2ca..0x1e31c` it computes VDD in 0.1 mV as
`(1080 * 4096 * 250) / floor(sum_reference / 10)` and line voltage in 0.1 mV as
`sum_line * VDD_tenth_mV / (4096 * 250)`. This explains the fractional precision
in USB ADC responses; an individual DR read is not an atomic copy of that average.

When PA2/PA3 output-enable is asserted, stock additionally replaces D+/D− with
`100 * control_value + (DR & 0xf)` in mV; its averaged result uses that value
times ten. See `0x1e1ae..0x1e20a` and `0x1e31e..0x1e35c`, and getters `0x21cb4`
and `0x21c94`. Our passive reader does not reproduce that output-control override.
It also does not apply INA229's unrelated external-flash VBUS/current correction.

## Rust acquisition

The board task owns ADC1, PA0..3 and the internal-reference routing. It uses PAC
register access following the vendor DDL: 12 bits, right alignment, software-start
sequence A once, channels 0..3 and 16, 16 hardware averages per channel, 160 ADC
clock sample cycles on every selected channel. Stock uses 160 on the pads and
7 on the reference; our longer reference sampling is an explicit difference.
Analog watchdog, PGA, sequence B and external/synchronous triggering are disabled.
The ADC clock is PCLK2=48 MHz in the current clock tree.

Conversion happens in hardware. The task yields for 1 ms between end-of-scan
checks, bounds completion at 5 ms, stops the converter before publishing, then
waits 20 ms. It does not use DMA or an ADC IRQ; PD timer/DMA reception continues
in its separate interrupt path. A failed stop retires this ADC owner. A timeout,
out-of-range count, zero reference or inferred supply outside 2.0..3.6 V makes
the sample unavailable rather than substituting an assumed supply voltage.

Rust retains fractional precision with `line_uV = ADCline * 1080000 / ADCref` and
`VDD_uV = 1080000 * 4096 / ADCref`. This is the same nominal transfer ratio,
with fewer intermediate integer truncations, not bit-for-bit native averaging.
Display and CDC update at about 4 Hz. `CAL` on the main screen still refers to
INA229 correction only: the four line inputs have a stock-derived scale, not a
new per-device calibration against an external voltage source.

## Automatic CC receiver selection

`pd start auto` selects the unique CC input in 0.400..2.150 V after two matching
ADC scans. A receiver task checks this decision at most every 20 ms; switches
quiesce producers/DMA before changing CMP1's mux and restarting the buffer pool.
Frames carry the input at capture time. `input_switches` and `switch_discards`
are cumulative across switches. Old queued/partial bursts may be discarded at a
switch and are counted separately from buffer overruns.

When neither/both inputs qualify, hold the last selection (initial fallback 1).
Manual `pd start 1/2` stays available. This is a passive heuristic with a settling
window, not a complete Type-C attach/role policy or a guarantee to catch the first
packet after insertion. TX and GoodCRC generation are still absent; a future
`usbpd` adapter must keep `HAS_AUTO_GOOD_CRC=false`.

[Preserved samples, comparisons and limits](../hardware/line-adc/README.md) include
D+ and D− explicitly. D− has only a zero-level reference so far.
