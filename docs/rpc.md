# Native USB postcard-rpc

The `rpc,pd-sink,orientation` image uses a vendor bulk interface rather than CDC.
Commands and measurements/PD events are postcard binary messages with shared
schemas in `crates/km003c-interface`. JSON/hex formatting is absent from this
image; the previous CDC image remains selectable for older bring-up tools.
The default feature set is unchanged: select the RPC image explicitly.

```sh
cargo build --release --locked --features rpc,pd-sink,orientation
bash tools/rpc.sh
bash tools/rpc.sh sink 20
bash tools/rpc.sh loader
```

The host client uses nusb and filters for `KM003C Rust RPC`; no serial driver is
needed. Status replies include uptime, protocol version and HCLK. `SetStream`
starts or stops 250 ms measurement topics. Values retain integer physical units,
signed/optional current, calibration state, diagnostic flags and optional line
ADC values. A missing ADC snapshot stays None rather than becoming zero.

`ControlPd` accepts Stop or an explicitly requested fixed voltage and duration.
Only 5/9/12/15/20 V and 1..600 seconds are accepted. The accepted response means
queued, not negotiated: `PdEvents::State(Ready)` follows Accept/PS_RDY. Automatic
CC selection/reorientation, the lease and cancellation cleanup use the accepted
PD implementation. A pending/active sink rejects another sink request as Busy;
Stop immediately disables direct TX and the worker awaits CC2 release. The
client requests a ten-second session and normally stops it after six seconds.
Test active requests with an unloaded charger.

PD work stays on INT030 at priority 7. Its producers only copy into a bounded
32-record queue; USB sends happen in thread mode. Events preserve raw PD wire
bytes and length, SOP, RX sequence/timestamp, TX DWT timing and capture counters.
The current stats array follows frames, overruns, held_overruns, noise, full,
dma_errors, mismatches, stop_faults, input_switches, switch_discards,
max_handler_cycles, reserved. Queue saturation can lose diagnostics and never
backpressures GoodCRC. Loss accounting and named counter fields remain follow-up
work; these counters do not count every possible telemetry loss.

The existing log implementation still writes retention RAM. Info/warn/error
records additionally enter an eight-record queue, with up to 192 bytes per
record, and are published through standard `LoggingTopic`. Each send is bounded
to 20 ms. Trace/debug remain retained locally; these string logs do not provide
defmt compression. Logging is not a persistence mechanism: reset diagnostics
still live in retention RAM and can be read through the native loader. The
client subscribes to measurements, PD events and logging before starting work.

The upstream postcard-rpc revision is pinned to `2469ebc6`; the published 0.12.1
lacks the Embassy 0.6 backend. This upstream revision supports driver 0.2 rather
than pinning 0.2.0. Embassy USB, sync and time packages are patched to the already
used `c2413d5` revision to keep their driver/mutex/timer types consistent. The
CDC compatibility code uses that revision's `cdc_acm::device` module.

[Hardware evidence](../hardware/rpc/README.md) verifies binary measurement,
logging, 20 V negotiation, Stop and native-loader entry. Passive capture commands,
raw-edge upload and decoder/time benchmark controls still use the legacy CDC
image; they have not been migrated into RPC. Do not combine `rpc` with
`pd-bench`. Accelerometer rotation continues locally; its separate diagnostic
topic is not yet part of this interface.
