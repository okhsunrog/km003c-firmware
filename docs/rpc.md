# Native USB postcard-rpc

The firmware uses a vendor bulk interface rather than CDC.
Commands and measurements/PD events are postcard binary messages with shared
schemas in `crates/km003c-interface`. JSON/hex formatting is absent from this
image. CDC, NDJSON and the Python USB clients have been removed.

```sh
just build-firmware
just cli capture
just cli sink 20
just cli loader
```

The host client uses nusb and filters for `KM003C Rust RPC`; no serial driver is
needed. Status replies include uptime, protocol version and HCLK. Protocol **4**
adds live profiles and timestamped measurement snapshots; update the application
and host together. Status and loader entry remain compatible with older custom
images so they can be updated without a button press. `SetStream`
starts or stops measurement topics. `SetStreamInterval` accepts 20..5000 ms
(default 250 ms); it changes publication cadence, not the INA229 conversion
configuration. `GetMeasurement` returns the latest valid sample or None.
The measurement task polls for fresh conversion-ready results every 20 ms;
each published sample includes its acquisition `sampled_at_ms` and
`sample_sequence`. `uptime_ms` is the later publication/snapshot time. Repeating
the same sequence does not represent a new conversion. Values retain integer physical units,
signed/optional current, calibration state, diagnostic flags and optional line
ADC values. A missing ADC snapshot stays None rather than becoming zero.

`ControlPd` accepts Stop, passive Monitor, Sink and SetProfile. Sink includes
voltage, requested current and a finite duration. Only 5/9/12/15/20 V,
100..3000 mA in 10 mA steps, and 1..600 seconds are accepted. The accepted response means
queued, not negotiated: `PdEvents::State(Ready)` follows Accept/PS_RDY. Automatic
CC selection/reorientation, the lease and cancellation cleanup use the accepted
PD implementation. A pending/active sink rejects another sink request as Busy;
Stop immediately disables direct TX and the worker awaits CC2 release. The
default `sink` client records six seconds and requests a five-second longer
lease for cleanup. Test active requests with an unloaded charger. A device such
as Pinecil must release its own PD controller before the meter becomes the
active sink on that CC line; this hardware has no independently managed pair of
PD ports.

`SetProfile { volts, current_ma }` requires a confirmed active sink contract
and no other pending profile. It checks the latest advertised fixed PDOs before
transmitting a Request. It never detaches Rd or renews the session lease.
Unsupported profiles leave the previous contract intact. `Wait` leaves the
request pending while the policy engine retries; `Reject` records a rejection.
Stop, disconnection, reset and lease expiry cancel pending work. In
`GetPdStatus`, `contract` is present only for a confirmed contract, and is cleared
before renegotiation. `pending_profile`, `last_profile` and `source_profiles`
provide command completion and advertised voltage/current limits independently
of lossy topic delivery.

```sh
just cli measure
just cli monitor auto --interval-ms 20 --seconds 60 --output passive.ndjson
# Unloaded source only:
just cli session --sink 15 --current-ma 100 --seconds 120 --output profiles.ndjson
# In the session terminal:
# profile 9 100
# status
# profile 5 100
# stop
# monitor auto
# quit
just maneuver maneuvers/fixed-profiles.json profiles.ndjson
```

`session` keeps subscriptions and the recording open while accepting commands
on stdin. Without `--sink` it starts passively. Each command gets an ID and a
timestamp in the same NDJSON file as ADC and PD events. The host also polls PD
status every 250 ms and records changes, including profile completion. Invalid
interactive commands are recorded as errors without stopping capture. EOF ends
input, not capture; `quit`, Ctrl-C and the duration limit perform Stop cleanup.

PD work stays on INT030 at priority 7. Its producers only copy into a bounded
32-record queue; USB sends happen in thread mode. Events preserve raw PD wire
bytes and length, SOP, RX sequence/timestamp, Hard/Cable Reset ordered sets,
TX DWT timing and capture counters.
The current stats array follows frames, overruns, held_overruns, noise, full,
dma_errors, mismatches, stop_faults, input_switches, switch_discards,
max_handler_cycles, reserved. Queue saturation can lose diagnostics and never
backpressures GoodCRC. `GetPdStatus.event_queue_drops` counts queue overflows,
and `event_publish_failures` counts USB publication errors/timeouts, cumulatively
since boot. These complement the decoder/DMA counters; none prove lossless wire
capture or count drops in a slow host subscriber. Named capture counter fields
remain follow-up work.

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
used `c2413d5` revision to keep their driver/mutex/timer types consistent.

`just cli monitor auto` starts passive capture with ADC-based CC
selection; `monitor 1` or `monitor 2` fixes the receiver input. Passive capture
never enables Rd or sends GoodCRC. Stop ends capture through the same owner as
active sessions. It uses the interrupt-rearmed buffers and validates CRC before
publishing packets.

The [protocol-4 hardware run](../hardware/host-control/README.md) captured actual
pass-through EPR traffic while Pinecil negotiated independently, with simultaneous
ADC samples. Live active profile switching still needs a separate hardware test.
An older active 20 V session in the
[workspace test](../hardware/workspace/README.md) reached Ready but recorded six
receive overruns. The earlier RPC cleanup session recorded zero, so neither
capture establishes that startup losses have been eliminated.

[Hardware evidence](../hardware/rpc/README.md) preserves the initial RPC
acceptance. Offline decoder/time benchmark modes, fixed GetCaps probes and raw
single-burst test commands have been removed. Decoder regression tests and
historical measurements remain available in the driver crate and documentation.

`GetPdStatus` returns current phase, availability, active command ID, the last
completed session and the last completed command. A completion carries its
reason and whether release was confirmed. Stop completion does not erase the
last sink/monitor result. This snapshot is authoritative even if a bounded topic
queue drops the terminal event; the host polls it while waiting for Stop.
`just cli pd-status` prints the snapshot. The original completion API appeared in
protocol 3; protocol 4 extends it with contracts, profile results and loss counts.

`EnterLoader` releases direct TX and awaits confirmation of CC2 Rd release,
then writes the native boot request and resets. If release fails it returns an
error instead of claiming successful recovery. `just cli loader` verifies native
DFU on the selected USB port; a lost RPC reply/reset alone is never success.
The same CLI command uses the pinned `km003c-lib` entry command for stock firmware,
and is idempotent in DFU. Physical recovery remains holding Exit while powering
the HID port. Flash writes start at `0x4000`; native loader and ICG are preserved.
