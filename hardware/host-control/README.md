# Host control and passive EPR capture, 2026-11-06

KM003C HW2.1 was connected inline between the EPR source and Pinecil V2.
The separate HID/control port stayed connected to the laptop. Pinecil remained
the only active PD sink throughout these tests; its heater was OFF.

The unified `loader` command entered native DFU from stock 2.0.7 without a cable
change. A fresh CRC-verified application backup matched the earlier stock backup
byte-for-byte. The rollback package remains outside the repository. The custom
application was flashed once, fully read back, and its loader compared before
and after. Then its own RPC `loader` command returned to DFU, `verify` repeated
the comparison, and `start` restarted it without another flash write. Both
loader commands confirmed DFU on the same physical USB port. The working custom
application was left installed. See [verification.json](verification.json).

[passive-epr.ndjson](passive-epr.ndjson) contains a 12-second simultaneous ADC
and pass-through PD capture: 533 measurement records, 455 distinct acquisitions,
124 received packets. Calibrated voltage remained 27.892767..27.904681 V. No
reset ordered set was decoded. Queue drops and USB publication failures were
zero; raw capture counters are preserved in the file and summary. This validates
actual passive EPR traffic, unlike the older unloaded passive captures. It does
not establish lossless capture or absolute voltage accuracy.

The requested USB publication interval was 20 ms. Acquisition sequences expose
repeated snapshots and scheduling delays; this run does not claim a 50 Hz or
1 kHz conversion rate. Source negotiation continued independently during meter
loader/application resets, and Pinecil remained at 28 V with heating disabled.

[interactive-commands.ndjson](interactive-commands.ndjson) records rejection of
28 V and of a profile request during passive operation, plus Stop → Monitor →
quit with confirmed final detach. [idle-stdin.ndjson](idle-stdin.ndjson) confirms
that an interactive session ends at its three-second deadline even without
terminal input. The detached stdin reader does not keep the CLI process alive.

The source/host checks passed 38 tests, rustfmt, Clippy with warnings denied,
and a Thumb release link. **Live active profile switching remains untested on
hardware** in this checkpoint: it requires Pinecil to release its PD controller
first. The prior fixed-voltage unloaded tests cover the older sink implementation,
not the new SetProfile lifecycle. No second sink was enabled for these captures.

Before the final install, cancellation was tightened so a queued profile cannot
run after a disconnect has already completed it as Cancelled. The final image
was separately flashed and read back with the loader unchanged, then its RPC
loader/start round trip passed again. [passive-final.ndjson](passive-final.ndjson)
records the subsequent ten-second capture; the `final_image` section in
[verification.json](verification.json) identifies that installed image. The
earlier traces above retain their original identity and are not relabelled as
captures from a later build.

The first final-image capture confirmed Stop through RPC before the queued Stats
topic had reached the host. Capture cleanup now drains subscriptions for a
bounded 100 ms after Stop. [cleanup-tail.ndjson](cleanup-tail.ndjson) verifies
that the terminal counters are retained with that host fix, rather than silently
discarded when the authoritative detach snapshot arrives first.
