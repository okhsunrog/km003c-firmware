# PD decoder optimization acceptance

KM003C HW2.1, nominal 192 MHz. The comparison image measured 18 complete
implementations in five runs without reflashing between runs. Each candidate
received eight warm-ups and 128 samples per fixture per run: 640 measured decodes
per fixture/candidate. DWT includes CRC and packet construction; IRQs stay enabled.
CC outputs are released and RX DMA stopped before the validation/measurement
batches. Fixture capture briefly attaches at 5 V without sending a Request.

The scalar Thumb-2 `asm_preamble` candidate won all three fixtures. Six-PDO
Source Capabilities fell from 73.36 to 45.17 µs (38.4% less time); the Pixel
Request fell from 39.26 to 24.53 µs and the maximum packet from 82.52 to 50.38 µs.
DSP packing did not win: the four-lane variant was particularly poor for the
synthetic maximum packet. Production uses no DSP or unaligned word loads.

## Complete comparison

Pooled median µs; `burst` is the preceding portable completed-burst decoder.
`streaming` is the general per-interval reference, not the preceding sink path.
The Source Capabilities fixture is a live six-PDO charger response, Pixel is the
committed one-object Request capture, and maximum is a synthetic seven-object
packet with FF payload. Firmware/fixtures and warm-up conditions match within
the comparison; these are not cold-code or worst-case interrupt guarantees.

| Candidate | Source Capabilities | Pixel Request | Maximum packet |
| --- | --- | --- | --- |
| `streaming` | 160.35 | 78.23 | 186.24 |
| `burst` | 73.36 | 39.26 | 82.52 |
| `burst_flash` | 75.94 | 41.81 | 84.72 |
| `burst_filter` | 71.71 | 37.20 | 80.55 |
| `symbols_rust` | 66.70 | 34.05 | 74.27 |
| `paired_rust` | 66.96 | 34.86 | 70.63 |
| `paired_dsp` | 68.21 | 36.12 | 71.89 |
| `asm_scalar` | 61.25 | 32.52 | 67.86 |
| `asm_dsp_half` | 70.12 | 36.68 | 74.91 |
| `asm_dsp_word` | 66.10 | 34.75 | 70.48 |
| `asm_dsp4_word` | 64.52 | 33.85 | 75.95 |
| `asm_dsp4_half` | 70.73 | 35.83 | 83.40 |
| `asm_fused` | 58.32 | 30.76 | 64.70 |
| `asm_unroll` | 53.92 | 29.57 | 60.36 |
| `asm_hoist` | 50.72 | 28.60 | 56.36 |
| `asm_byte_pair` | 48.62 | 27.98 | 53.83 |
| `asm_preamble` | 45.17 | 24.53 | 50.38 |
| `asm_hoist_preamble` | 47.26 | 25.13 | 52.89 |

[summary.json](summary.json) contains min/p50/p95/max and each run's median.
[run-a.ndjson](run-a.ndjson), [run-b.ndjson](run-b.ndjson),
[run-c.ndjson](run-c.ndjson), [run-d.ndjson](run-d.ndjson),
[run-e.ndjson](run-e.ndjson) preserve all raw samples and fixture intervals.
For Source Capabilities, the selected candidate's pooled p95 was 45.17 µs,
maximum 48.12 µs; `burst` p95 was 74.62 µs, maximum 75.76 µs.

The winning sequence of changes is isolated by the candidates: fuse BMC/4b5b,
unroll five bits, hoist worst-case bounds once per symbol, unroll byte pairs,
then skip confirmed alternating preamble. See the
[algorithm, safety argument and reproduction commands](../../docs/pd-decoder.md).

## Correctness and working-image checks

- Every hardware run passed 322 exact/jittered packet and reset cases on all 18
  candidates before timing, including five SOP selectors and all NDO values.
- All 11 compiled Thumb-2 kernels passed 11,044 differential cases each in Unicorn:
  native-derived packets, jitter, leading noise, truncation at every interval of
  a representative packet, timing boundaries and invalid symbols/EOP.
- Hooks check input/output bounds and canaries. The final production kernel
  additionally passed the same 11,044 cases with non-null DWT profiling output,
  callee-saved register and stack checks.
- The portable driver tests passed 28 tests, including 162 native TX vectors,
  streamed/burst/two-stage/fused validation parity and CRC/NDO/EOP rejection.
- Release Clippy passed with `-D warnings` for comparison and production features.

The production `.pd_fast` section is 1,908 bytes. It includes only the selected
kernel, constants and Rust validation/profiling; experimental kernels are
removed by section garbage collection. This image was flashed once and all
five contracts below reused it. [tested-image.bin](tested-image.bin) is the
actual verified application readback; [benchmark-image.bin](benchmark-image.bin)
is the separately verified 18-candidate image. The loader patches the eight
size/CRC bytes at image offsets 0x428..0x42f.

## Renewed active negotiation

Pixel disconnected, unloaded charger, HID connected to host. The production
image negotiated each fixed voltage, received Accept/PS_RDY and detached on
explicit stop. Each capture has one Request and three software GoodCRC TXs,
without observed resets or repeated Requests in these sessions.

| Requested | INA229 median of final five samples | Capture |
| --- | --- | --- |
| 5 V | 4.987494 V | [5v.ndjson](5v.ndjson) |
| 9 V | 8.990033 V | [9v.ndjson](9v.ndjson) |
| 12 V | 11.983587 V | [12v.ndjson](12v.ndjson) |
| 15 V | 14.981634 V | [15v.ndjson](15v.ndjson) |
| 20 V | 19.980658 V | [20v.ndjson](20v.ndjson) |

Instrumented Source Capabilities phases were approximately
10.18 / 28.92 / 2.64 / 1.79 µs for prefix, payload/shape, CRC and assembly.
The driver decode mark,
including additional bookkeeping/instrumentation, was 49.81 µs. Software GoodCRC
started 102.29..103.56 µs after the RX completion mark, versus about 127 µs before.
That mark follows the 12.75 µs quiet gap, so estimated EOP-to-TX was 115..117 µs.
These are software marks, not electrical compliance measurements.

The captures show six pool overruns during startup/attachment in each session.
No DMA errors, count mismatches, held overruns or stop faults occurred. These
sessions establish negotiation, not lossless startup capture or load capability.
The passive decoder's `decode_errors`/`sync_errors` counters do not count sink
burst rejections. CC2 TX, PPS/EPR policy and the Embassy timer backend are unchanged.

## Identity and recovery

[verification.json](verification.json) records analog samples, raw cycle marks,
TX bytes, counters, image hashes and recovery metadata.
[hardware-run.log](hardware-run.log) and
[benchmark-hardware-run.log](benchmark-hardware-run.log) record package/update
verification, complete custom application readback and unchanged native loader
bytes at 0x4..0x3fff, followed by a successful native-loader return.
[marker-after.bin](marker-after.bin) contains the normal `600d0002` recovery
marker. The accepted custom application was then restarted without a flash
write. Stock 2.0.7 was not reinstalled; the existing verified rollback was reused.
[SHA256SUMS](SHA256SUMS) covers the preserved evidence.
