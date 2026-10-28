# Completed-burst PD decoder

The CC1 sink uses `pd_fast::decode_observed`: a bounded Thumb-2 BMC/4b5b scanner
followed by Rust EOP, NDO and CRC validation. The streaming and portable burst
decoders remain independent references. Passive capture still uses its existing
streaming path; this change accelerates the sink's software GoodCRC path.

The selected kernel is `km_pd_bytes_preamble` (mode 9 in `src/pd_symbols.S`).
It uses halfword loads and ordinary integer instructions. Cortex-M4 DSP variants
were measured and lost to this scalar implementation. Production links only
the selected kernel; the entire `.pd_fast` section occupies 1,908 bytes in the
accepted image, including validation/profiling and constants. It runs in
zero-wait SRAMH; input DMA buffers remain in SRAM12 with one wait state.

## Algorithm and bounds

Input excludes the capture's unknown first duration. A short interval is
15..25 ticks, a long interval 30..50 ticks at the configured 12 MHz capture clock.
A BMC one consumes two shorts; zero consumes one long. Malformed pre-SOP timing
resets the ordered-set search; malformed payload rejects the burst.

- The prefix search retains an exact rolling 20-bit word. Its last five bits
  gate the seven-word SOP/reset search, avoiding useless comparisons through
  the alternating preamble.
- After the first 20 valid bits, a word of `0x55555` or `0xaaaaa` permits a faster
  alternating-preamble loop. It probes three bounded halfwords for SS,L or L,SS.
  Successful probes append two alternating bits, leaving the rolling word
  unchanged. Every intermediate window is alternating and cannot be SOP/reset.
  Failure consumes nothing, so the ordinary search resumes at the same input.
- Five BMC bits are unrolled into a symbol. A 20-byte guard covers its worst
  case of ten short intervals. A 40-byte guard covers a pair of symbols; the
  paired body removes low/high-nibble parity bookkeeping. The bounded scalar
  tail handles insufficient remaining input, including partial bits/symbols.
- Remaining input only decreases. Once the decoder enters a partial-byte or
  partial-symbol tail, it cannot accidentally re-enter a larger fast path.
- EOP is checked after each symbol, including the first symbol of a byte pair.
  Output is at most 34 decoded bytes including CRC, followed by one EOP marker.
  A 69-symbol cap prevents overflow before malformed data can grow the output.
- Rust requires complete bytes, EOP, exact header-declared NDO length and CRC
  before constructing a packet. Assembly does not deliver trusted PD messages.

Truncated input, missing EOP and invalid symbols retain distinct error variants.
Fused decoding rejects invalid 4b5b immediately; with multiple defects, the
first reported error can differ from the reference decoder. No identical error
precedence is claimed. Hard/Cable Reset ordered sets remain separate events.

## Measuring candidates

The opt-in `pd-bench` feature compares 18 complete decoders in one image. The
`pd bench` command briefly attaches as a sink at 5 V to capture real Source
Capabilities. It then releases CC outputs and stops RX DMA before validating
or measuring anything. No voltage Request is transmitted by this command.
Other fixtures are a recorded Pixel Request and a synthetic maximum packet.

Each run first cross-checks 322 valid packet/reset cases, including timing jitter.
Each fixture/candidate then gets eight warm-ups and 128 DWT measurements with
IRQs enabled. Timing includes result construction and CRC, excludes telemetry,
and uses nominal 192 MHz. Raw samples expose interrupt-related outliers.
These measurements describe cached fixtures, not a worst-case timing guarantee.

```sh
cargo build --release --locked --features pd-bench,orientation,manual-ui-test
# Install and verify this image using the normal update/recovery procedure.
uv run tools/bench.py decoder-run --pd-bench --seconds 25
uv run tools/summarize_pd_bench.py CAPTURE.ndjson --json summary.json
```

The capture helper stops on `pd_bench_done`, drains the stop acknowledgement,
and does not flash. Repeating the command reuses the same image.

Compiled instruction correctness is checked separately with Unicorn, which is
an emulator, not a timing source. The differential corpus includes native
encoder vectors, jitter, every truncation of a representative packet, interval
boundary mutations, invalid 4b5b/EOP and noise. Hooks check every input read,
output capacity/canaries, callee-saved registers and stack restoration.

```sh
cargo run --manifest-path crates/km003c-drivers/Cargo.toml \
  --example pd_scanner_vectors --target x86_64-unknown-linux-gnu -- VECTORS.tsv
uv run tools/validate_pd_scanners.py \
  target/thumbv7em-none-eabihf/release/km003c-rust-smoke VECTORS.tsv
# A production image contains only the selected kernel:
uv run tools/validate_pd_scanners.py \
  target/thumbv7em-none-eabihf/release/km003c-rust-smoke VECTORS.tsv \
  --kernel km_pd_bytes_preamble --profile
```

[Hardware results](../hardware/pd-optimization/README.md) preserve all five runs,
raw cycles, fixtures, both image identities, 5/9/12/15/20 V acceptance and recovery.
The Embassy clock backend and protocol policies are unchanged by this work.
