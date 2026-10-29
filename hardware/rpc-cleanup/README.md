# RPC-only application acceptance

The application builds without feature flags. CDC, NDJSON, single-burst/GetCaps
probes, sensor-reset tests and standalone benchmarks have been removed. Passive
monitoring uses RPC and the same buffered capture owner as active sessions.

[monitor.log](monitor.log) records passive monitoring with an unloaded charger:
VBUS stayed near zero, no attachment/Ready was emitted. There was no external PD
partner during that capture, so this run does not claim hardware acceptance of
decoded passive traffic or cable flips. Driver regression tests cover decoding.

[20v.log](20v.log) records the subsequent explicit 20 V request, Ready, valid PD
RX/TX and Stop/Detached. Final capture counters report zero overruns, DMA errors,
mismatches and stop faults. [hardware-run.log](hardware-run.log) records matching
image readback, unchanged loader and normal RPC-requested loader return.
[tested-image.bin](tested-image.bin) is the application readback. It was restarted
without another flash write and remains installed; stock was not restored.

The retained Thumb-2 decoder is 1364 bytes and occurs byte-for-byte in the
previous accepted RPC image. Unused assembler variants and benchmark wrappers
were removed. [verification.json](verification.json) records its hash and this
image's identity. [SHA256SUMS](SHA256SUMS) covers the evidence.
