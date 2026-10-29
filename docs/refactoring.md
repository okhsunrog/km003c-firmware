# HAL and application refactoring

CC2 support extends the existing bring-up code. The following changes are the
next implementation stage, not claims about already migrated drivers.

## Peripheral ownership and drivers

The HC32 fork includes a generated `hc32f460-pac`, exported by `embassy_hc32::pac`.
Use it for chip registers and `cortex-m` for core peripherals. Replace repeated
peripheral steals and register-address constants with ownership passed from
board initialization. Keep justified unsafe boundaries small and documented;
retain host register-model tests for algorithms and register ordering.

Generic clocks, interrupt routing, DMA, ADC, timers and USB port support belong
in the HC32 HAL. KM003C pin assignments, AW9523 CC wiring, calibration storage,
PD attachment policy and UI belong here. Review the PAC against vendor DDL/RM
while converting: the current GPIO latch-read constant is `PODR=0x02`, whereas
the PAC places it at `0x04`. Existing POSR/PORR writes use different offsets and
must not be changed along with that read.

I2C operations, USB endpoints, PD receive/transmit waits and timeouts are async.
Display/external-flash SPI remains synchronous; line ADC acquisition runs in
hardware but completion is currently checked by timer polling. Do not describe
the whole firmware as async. Prefer interrupt/DMA-backed HAL operations where
they improve scheduling, with explicit buffer and cancellation ownership.

The shared I2C3 bus now uses `embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice`
and an `embassy_sync::mutex::Mutex`. The AW9523 device mutex additionally protects
multi-transaction read-modify-write sequences. For synchronous SPI sharing,
consider `embedded-hal-bus`; mapped external flash and the display's common
QSPI pins first need an ownership design that prevents concurrent mapped reads.
Select reusable chip drivers by their actual embedded-hal support and board
constraints rather than replacing working register sequences blindly.

## USB control, telemetry and logging

The [postcard-rpc image](rpc.md) now replaces CDC ASCII/NDJSON for status,
measurements, active PD control/events and logging. Passive and benchmark
commands still require the legacy image. Define remaining request/response and
topic types in the shared `no_std`
interface crate used by firmware and a Rust host client. Use endpoints for
status, configuration, explicit PD control and loader entry; topics carry
measurements, captured PD traffic and diagnostics. Preserve explicit active-PD
control and cancellation/release semantics across the migration.

The image retains the existing Synopsys/HC32 USB driver and uses upstream's
native bulk backend. Upstream revision `2469ebc6` adds Embassy USB 0.6 support
and relaxes the old driver pin. The USB/sync/time dependency graph now uses one
Embassy git revision; [integration details](rpc.md) record that compatibility
work and hardware results.

`Sender::log_str` and `log_fmt` publish text on standard `LoggingTopic`.
An adapter for synchronous `log` calls must enqueue bounded records for a
separate sender task. Queue saturation, a disconnected host or USB backpressure
must never delay the PD software GoodCRC path. Persisted panic/retention
diagnostics remain readable after reset; RPC is a way to retrieve them during
normal operation, not their storage. Compact defmt frames could use a separate
binary topic and matching ELF metadata if needed; the standard logging topic
itself is text.

Primary references: [postcard-rpc sender API](https://docs.rs/postcard-rpc/latest/postcard_rpc/server/struct.Sender.html),
[upstream features and dependency versions](https://github.com/jamesmunns/postcard-rpc/blob/main/source/postcard-rpc/Cargo.toml),
[standard logging topic](https://github.com/jamesmunns/postcard-rpc/blob/main/source/postcard-rpc/src/standard_icd.rs),
[async I2cDevice](https://docs.embassy.dev/embassy-embedded-hal/git/default/shared_bus/asynch/i2c/struct.I2cDevice.html),
[embedded-hal-bus](https://docs.rs/embedded-hal-bus/latest/embedded_hal_bus/).
