# TimerA time-driver acceptance

KM003C HW2.1, HCLK 192 MHz, PCLK1 96 MHz, `pd-bench,orientation` image.
Logical time is 32,768 Hz; TimerA1 counts continuously at 375 kHz and serves
compare alarms. SysTick enable/interrupt bits read back as zero.

## Wait and wrap checks

`clock-final.ndjson` contains 64 samples per case with varied start phase.
Measurements read the hardware counter directly, independent of software clock
conversion. All times use nominal 375 kHz. A 200 ms active read loop crossed a
hardware wrap with its IRQ pending, without a backwards time or implausible
jump. A 1.1-second async wait then spanned six wraps and 173 timer IRQs while
UI, ADC, line ADC, accelerometer and CDC were running.

| Wait | Extra logical tick | Min µs | Median µs | Max µs |
| --- | --- | --- | --- | --- |
| 1 ms | False | 989.333 | 1005.333 | 1024.000 |
| 1 ms | True | 1018.667 | 1034.667 | 1050.667 |
| 5 ms | True | 5016.000 | 5032.000 | 5048.000 |
| 20 ms | True | 20032.000 | 20048.000 | 20064.000 |

The padded 1 ms wait replaces the former DWT polling loop. The unpadded case
shows why rounding the duration alone does not guarantee a full millisecond.
This is sampled acceptance under the measured workload, not a worst-case
latency guarantee.

## IRQ costs

DWT_CYCCNT measures the HAL handler body, excluding exception entry/exit and
profiler bookkeeping. The profiler is board code enabled only with `pd-bench`;
the HAL has no DWT dependency. Statistics below cover the 1.1-second async wait.

| Vector | Calls | Total cycles | Mean body µs | CPU time in window |
| --- | --- | --- | --- | --- |
| Overflow | 6 | 7023 | 6.096 | 0.003325% |
| Compare | 167 | 211677 | 6.602 | 0.100225% |

Total measured IRQ body CPU consumption was about 0.104%. Mandatory overflow
service is 5.722 events/s; using the observed mean body cost gives about 0.0035%
CPU. Ordinary `now()`/alarm-registration calls and hardware exception overhead
are additional. Maximum counters in the capture are cumulative since boot;
call counts and cycle sums are differences over the measurement window.

## PD and lease regression

The first image negotiated all five fixed voltages through the usbpd stack.
The profile image adds phase-sweep/profiling diagnostics and pins the HAL's
CI-enabled revision. The final image also clears inherited pending SysTick and
PendSV before unmasking interrupts; it repeats the clock test and 20 V contract.
The backend code is the same.
Pixel was disconnected; the charger was unloaded.

| Capture | Requested | Final five ADC samples, median |
| --- | --- | --- |
| [5v](5v.ndjson) | 5 V | 4.986908 V |
| [9v](9v.ndjson) | 9 V | 8.990423 V |
| [12v](12v.ndjson) | 12 V | 11.984954 V |
| [15v](15v.ndjson) | 15 V | 14.981829 V |
| [20v](20v.ndjson) | 20 V | 19.978900 V |
| [final-20v](final-20v.ndjson) | 20 V | 19.980462 V |
| [startup-20v](startup-20v.ndjson) | 20 V | 19.980267 V |

Each session reached `contract_ready`, then detached on explicit stop, with one
Request and three software GoodCRC transmissions. The final 20 V capture also
contains SOP′ cable traffic. No alternate CC2 TX claim is made.

[lease.ndjson](lease.ndjson) records the five-second test attachment expiring
through an Embassy task. Sampled active CC lasted 4.757 s (250 ms reporting
interval), then CC1 returned above 3.18 V and VBUS near zero without a host stop
command. The image remained running beyond 117 s, exceeding the former 90-second
automatic-reset deadline. CC lease is a bench policy, not a USB-PD requirement.
TX uses `with_timeout` and a cancellation drop guard; injected TX/DMA failure
acceptance is not claimed by these successful negotiations.

## Image identity and checks

[tested-image.bin](tested-image.bin) is the final full application readback;
[first-tested-image.bin](first-tested-image.bin) preserves the initial image;
[profile-tested-image.bin](profile-tested-image.bin) preserves the profiling
image before core-pending cleanup.
Corresponding `hardware-run.log` files record verified writes, unchanged native
loader and successful manual CDC-requested loader returns. `marker-after.bin`
records the normal return marker. After testing, the existing verified custom
image was started without flash writes. No stock reinstall was performed.

[verification.json](verification.json) records raw-derived statistics, image
hashes and the HAL revision. [clock-first.ndjson](clock-first.ndjson) preserves
the initial clock check; [clock-profile.ndjson](clock-profile.ndjson) and
[clock-final.ndjson](clock-final.ndjson) include phase sweeps and IRQ costs.
[recovery.ndjson](recovery.ndjson) preserves final uptime before explicit
`loader`. [SHA256SUMS](SHA256SUMS) covers this evidence.

The HAL's 46 host tests for the new backend configuration and embedded checks
passed [remote CI](https://github.com/okhsunrog/hc32f460-embassy-rs/actions/runs/40792443540).
Tests cover fractional conversion, independent deadlines, pending overflow/read
races, compare programming races and targets across a wrap. Firmware Clippy,
driver tests and formatting checks also passed. Limits and integration details
are in [the time-driver documentation](../../docs/time-driver.md).
