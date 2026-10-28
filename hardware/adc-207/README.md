# INA229 hardware acceptance with stock 2.0.7

Tested on KM003C HW2.1 on 2026-10-28. The device was running stock 2.0.7 dated
2026.9.29. The owner confirmed voltage and current counts on the Rust display.
No PD transmitter, CC termination control or PD capture was enabled.

SPI2 returned manufacturer ID `0x5449` and device ID `0x2291`: **INA229**.
Both 90-second runs collected 360 samples, for 720 total, without SPI errors.
CONFIG/ADC_CONFIG/SHUNT_CAL/SHUNT_TEMPCO remained `0170/fdb0/1062/001e`.
Concurrent USB echo passed 1/23/64/7/64/17/128/256/512 bytes in both runs.

| Run | VBUS range, V | Uncalibrated current range, A | Last temperature, °C |
| --- | --- | --- | --- |
| Probe | 4.862695–4.883398 | -1.010736…-0.934768 | 34.273438 |
| Units | 4.894336–4.983398 | -0.736448…-0.092416 | 33.289063 |

The load changed during the tests. Stock samples before the units run were
4.899536–4.900312 V and -0.752806…-0.748368 A; after it, 4.909235–4.909938 V
and -0.673306…-0.670042 A. The complete ranges are preserved, including transient
low current in the Rust run. These are sequential samples, not an accuracy
calibration or simultaneous comparison under a stable reference load.

## Current units and factory correction

In actual stock 2.0.7, helper `0x6d78` reads register 7 through `0x6fac`.
For CONFIG.ADCRANGE=1, it sign-extends the left-justified 24-bit value; for
ADCRANGE=0 it multiplies that value by four. The ADC task calls this at `0x1df60`
and `0x1dff4`, passing the result as microamps into calibration at `0x17b1c`.
A signed 20-bit CURRENT count therefore corresponds to 16/64 microamps before
factory correction for the native SHUNT_CAL=`0x1062` profile. The wide range was
not exercised in hardware. See `native-measurement.asm` and `native-adc-task.asm`.

The units image reports the narrow-range scale as 16000 nanoamps/count in its
retained record. Its last value was -44079 counts, exactly -705264 microamps.
The UI labels this uncorrected current `I0`. A different SHUNT_CAL or arithmetic
overflow falls back to raw counts; physical shunt resistance is not guessed.

Read-only stock calibration inspection found voltage offsets -6 microvolts,
current gain 1002221 ppm and current offset 94 microamps. The three 30-sample CSVs
were obtained using the pinned `km003c-rs` library's `request_adc_data` API,
including original uncalibrated averages. All 90 averaged voltage/current pairs
match the stock averaging correction formula. This validates interpreting stock
protocol values and corrections; it does not establish ADC absolute accuracy.
The original stock RAM address is not used by the Rust image.

## Recovery and artifacts

The initial stock backup was read twice and matched the rollback package.
Both custom images were read back before execution and preserved the loader.
Both returned to native DFU. Stock 2.0.7 was restored after each run and verified
by complete 456896-byte readback. A final loader dump over `[0x4,0x4000)` matched
the original SHA-256 `17d51eeff9fb6abe1a7024a108006248e0aaebdaae52de69a3e26c000565d4bf`.

`verification.json` contains counts, hashes, ranges and restoration facts.
The binary retention records and hardware logs preserve the raw evidence.
`tested-image.bin` is the exact **units run** image, before the later dirty-row
UI optimization. Its ADC transport and current scale are the accepted ones;
subsequent display changes have a [separate smoke test](../dirty-rows/README.md)
and still need visual confirmation of the tearing fix.

The tests inherited sensor configuration across software reboots from stock.
At this stage, cold-start configuration, wide-range current, factory correction
and passive PD were untested; the image performed no configuration writes.
[Later startup/calibration tests](../cold-adc/README.md) add explicit configuration
from sensor reset and persisted factory correction. Whole-board cold-start
acceptance remains separate.
