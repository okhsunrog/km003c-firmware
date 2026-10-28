# Verified Embassy USB CDC on HW2.1

Final hardware acceptance on 2026-10-28:

- Linux enumerated `KM003C Rust USB test` with CDC control/data interfaces.
- The archived libusb probe verified exact echo for 1, 23, 64, 7, 64, 17,
  128, 256 and 512 bytes, including multi-packet transfers.
- Automatic reset returned to the native loader with status `0x600d0002`.
- The image was compared with full installed readback before execution.
- Original firmware 1.9.9 was restored and all 453,828 bytes verified.
- Loader bytes `[4, 0x4000)` remained identical to the known backup.

`verification.json` records hashes and source revisions. `echo-result.json`
and `echo.log` are the successful runtime probe's results. `trace.log` is the final
retention ring; `usb-diagnostic.bin` holds the timeout register snapshot and
`marker.bin` holds the reset result. `application.bin` / `application.mencrypt`
are the tested uncommitted image/package; `installed.bin` includes the native
loader's final metadata patch. `restore-result.txt` records full restoration.

`host_hardware_test.rs` archives the maintenance harness, including application
backup checks, DFU reads with independent reply IDs and exact image verification.
It uses pinned km003c-lib source and machine-local paths. It performs flash
writes and is archived evidence, not a portable install command.

The native core register comparison and official DDL source exposed the missing
RX status/setup reserve. A zero reserve yielded a 32-word RX FIFO and failed
EP0 status handling. Adding 32 words yields the native 64-word allocation and
works with the unmodified upstream git driver. Trial EP0 source patches and an
alternate USB stack were removed. Separate RX/TX futures with an eight-packet
queue avoid serializing receive progress behind an IN transfer waiting on the
host. Sustained throughput, self-powered VBUS behavior and reconnect stress
are outside this bounded acceptance run.

Raw bus captures are kept locally; they are not published because the capture
surface also includes other USB devices. SHA256SUMS covers this directory;
verify it from the repository root with:

```sh
sha256sum --check hardware/usb-cdc/SHA256SUMS
```

A later upload timed out after reporting 24,480 ciphertext bytes completed.
The next block's completion was unknown; blocks were not retried.
The native USB maintenance channel then required a complete power cycle with
Exit held; a fresh Setup restored the exact original package and full readback
passed. The cause of that transfer timeout is not established. The portable
`tools/usb_echo.py` now rejects native iProduct=4 before issuing string requests,
so it does not actively probe the native loader during discovery. Its native-
firmware negative-discovery check passed; the successful echo log above comes
from the archived runtime probe, not a post-recovery run of the portable tool.
This observed recovery is not an exhaustive power-loss-during-flash test.
