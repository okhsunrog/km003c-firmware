# Native USB RPC acceptance

KM003C HW2.1, HCLK 192 MHz, `rpc,pd-sink,orientation`. Pixel disconnected;
unloaded charger connected. No stock reinstall.

The first RPC image passed status, twelve binary measurement notifications and
RPC-requested loader entry. The final image adds active PD control and reporting.
[20v.log](20v.log) records typed RX/TX events, Ready at 20 V, INA229 samples near
19.98 V, standard LoggingTopic records, Stop, confirmed Detached and measured
VBUS returning near zero. These are host-formatted Rust Debug records; device
messages are binary postcard. No NDJSON decoder or encoder is used in this run.

The source-capability GoodCRC mark-to-TX time was 20480 cycles (106.67 us), with
9510 cycles (49.53 us) for instrumented decode at nominal 192 MHz. Final capture
counters have six overruns; DMA, mismatch and stop errors are zero. This does not
claim lossless capture or electrical timing compliance. CC2 wiring and automatic
reorientation use the previous accepted implementation; this RPC test ran CC1.

[tested-image.bin](tested-image.bin) is full verified application readback.
[hardware-run.log](hardware-run.log) records matching readback, unchanged native
loader and a successful manual RPC-requested loader return. The normal marker is
in [marker-after.bin](marker-after.bin), the final ring in [retention.log](retention.log)
and the request's status output in [loader.log](loader.log). The same verified
custom image was restarted without flash writes and remains installed.

[verification.json](verification.json) records acceptance flags and image hash;
[SHA256SUMS](SHA256SUMS) covers these files. Source-image scanning found no JSON
`"type"` literal. This scan complements the RPC implementation and actual binary
client run; it is not by itself proof of protocol behavior. Legacy CDC builds
remain available for the older passive/benchmark commands.
