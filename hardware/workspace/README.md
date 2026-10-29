# Workspace and host-tool acceptance

The self-contained host CLI packaged and installed the excluded firmware crate
on KM003C HW2.1, using the saved verified application backup. [flash.log](flash.log),
[before.json](before.json) and [verified.json](verified.json) record backup and
image identity, application readback and unchanged loader. The installed image
matches a final rebuild after accounting for loader-patched size/CRC metadata.
No external checkout or temporary harness was needed by the tool.

[monitor.ndjson](monitor.ndjson) records passive capture without attachment.
[20v.ndjson](20v.ndjson) records the fixed-20-V maneuver, PD Ready, measurements
and a matching command-ID Stop/Detached. Pixel and other loads were disconnected.
[rejection.ndjson](rejection.ndjson) records a second sink request rejected as
Busy; the runner still requested Stop and received matching Detached.
[after-rejection.ndjson](after-rejection.ndjson) records the subsequent capture.

RPC loader entry, native-DFU diagnostic extraction and restart were exercised
without another flash write. [diagnostics.json](diagnostics.json) is the decoded
retention status. The final `just start` and status query succeeded after USB
re-enumeration. The verified custom application remains installed.

All host records are NDJSON produced on the computer. Device transport uses
postcard-rpc binary messages. Passive decoded traffic with an attached PD partner
and a GUI were outside this run.
