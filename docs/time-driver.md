# TimerA Embassy time

The firmware selects the user's HAL fork with `default-features=false`,
`time-driver-tmra` and `embassy-time/tick-hz-32_768`. The driver owns all of
TimerA1 and two routes: overflow event 256 on INT080, compare event 258 on INT081,
both priority 5. TimerA2 remains exclusively owned by PD. No timer pin/output is
enabled. SysTick stays disabled after pre-init and is not passed to the HAL.

The HC32F460 has six 16-bit TimerA units, three 16-bit Timer4 units, three 16-bit
Timer6 units and two 16-bit Timer0 units. Core SysTick is 24-bit;
DWT_CYCCNT is a 32-bit debug cycle counter. Four TimerA units remain unused.

## Clock and alarm model

PCLK1 is 96 MHz. TimerA1 uses /256, counts continuously at 375,000 Hz, and wraps
every 65,536 counts: 174.762667 ms, or 5.722046 overflows/s. A logical tick is
1/32,768 s, about 30.518 µs. The physical and logical rates need not match.

The HAL accumulates raw counter epochs and converts deltas to 64-bit logical
time, retaining the fractional remainder. Reading time also accounts for a
pending overflow: it reads counter before flag, clears/accounts an overflow and
retries. The eventual overflow IRQ cannot count the same wrap twice. No DWT or
elapsed-time calibration is involved in timekeeping.

The 16-entry Embassy queue supplies the nearest deadline. Compare channel 1
is programmed with a rounded-up hardware target while the counter continues.
The driver checks time again after writing compare/enable: a deadline that
passed during programming is processed immediately. Distant alarms are
reconsidered at overflow. Empty queues leave compare disabled; the only required
maintenance is overflow handling. Other interrupts reflect actual deadlines.

Service the driver before an entire physical counter wrap is missed. PCLK1
must remain running and unchanged. Stop/power-down and debugger-halt timekeeping
are outside the tested contract. CPU sleep with the peripheral clock running
is exercised by ordinary async waits. These constraints belong to the backend,
not to the 64-bit `Instant` representation.

The [HAL implementation](https://github.com/okhsunrog/hc32f460-embassy-rs/blob/23e09a053ddf5284d28c06407b9172adf9c48a34/embassy/src/time_driver_tmra.rs)
has host models for fractional accumulation, independent deadlines, pending
wrap/read races, comparisons across wraps and deadlines passing during writes.
Its default legacy SysTick backend remains available for other callers; this
firmware explicitly selects TimerA and the 32,768 Hz logical unit.

## PD timeouts and DFU

The stack's `Clock::after_millis` awaits an ordinary Embassy Timer with one
extra tick. `now()` rounds down; a 1 ms duration rounds up to 33 ticks. Depending
on start phase, 33 ticks can mean less than a full millisecond. Adding one tick
guarantees at least the requested duration before interrupt/scheduling delay.
The former one-millisecond DWT polling loop is removed.

TX uses `with_timeout(5 ms, completion)` and an RAII guard that releases active
DMA/output if the future is cancelled. A separate async CC lease task handles
five-second attachment and 30-second sink leases. Generation checks prevent
expiry of an old lease from releasing a renewed connection. Timers rely on
normal executor progress; no separate interrupt-level guard or SysTick is used.

The early smoke-test automatic DFU reset is removed. Holding Exit for two
seconds or sending `loader\n` through CDC requests DFU. The handler writes
`0x12345678` at retention address `0x200f0ff0` and resets the MCU so its native
loader handles the request. Panic/HardFault preserve their diagnostic/reset
paths. Normal operation has no 90-second or ten-minute deadline;
`manual-ui-test` remains an empty compatibility feature for earlier scripts.

## Measurement

Build with `pd-bench,orientation` and run:

```sh
uv run tools/bench.py clock --time-bench --seconds 10
```

This command does not attach CC. It exercises monotonic reads across a wrap,
varies the start phase of 64 waits each at 1/5/20 ms, reads the physical counter
directly as a reference, and checks a 1.1-second wait across several wraps.
The opt-in board DWT profiler separates overflow/compare IRQ body costs. These
measurements exclude exception entry/exit and the profiler's own bookkeeping;
the HAL itself has no DWT dependency. [Evidence](../hardware/time-driver/README.md)
records raw samples, images, IRQ cost and renewed PD contracts.

## Why use a peripheral timer?

SysTick only interrupts at zero. Arbitrary alarms require changing its running
counter or another alarm source; the resulting races/calibration problems are
discussed in [Embassy issue 786](https://github.com/embassy-rs/embassy/issues/786).
TimerA's compare channel provides the needed alarm without disturbing time.
Software extension of a narrow timer is established practice: the official
[STM32 driver](https://github.com/embassy-rs/embassy/blob/main/embassy-stm32/src/time_driver/tim.rs)
supports both 16- and 32-bit counters; backend choice depends on the MCU.
