# /// script
# requires-python = ">=3.13"
# dependencies = ["pyusb>=1.3.1"]
# ///
"""Record NDJSON from the experimental CDC interface, optionally capture PD.

Uses libusb when no serial kernel driver is bound. Commands are ASCII lines;
USB packet boundaries are unrelated to record boundaries. Output contains only
complete JSON records. Invalid fragments are counted and reported on stderr.
"""

import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import time

import usb.backend.libusb1
import usb.core
import usb.util


@contextmanager
def connect(libusb: str | None, wait: float):
    backend = (
        usb.backend.libusb1.get_backend(find_library=lambda _: libusb)
        if libusb
        else usb.backend.libusb1.get_backend()
    )
    if backend is None:
        raise RuntimeError("Cannot load libusb; supply --libusb")
    deadline = time.monotonic() + wait
    device = None
    while time.monotonic() < deadline:
        candidate = usb.core.find(idVendor=0x5FC9, idProduct=0x0063, backend=backend)
        if (
            candidate
            and usb.util.get_string(candidate, candidate.iProduct)
            == "KM003C Rust telemetry"
        ):
            device = candidate
            break
        time.sleep(0.1)
    if device is None:
        raise RuntimeError("Telemetry image did not enumerate")
    claimed, detached = [], []
    try:
        for interface in (0, 1):
            if device.is_kernel_driver_active(interface):
                device.detach_kernel_driver(interface)
                detached.append(interface)
            usb.util.claim_interface(device, interface)
            claimed.append(interface)
        device.ctrl_transfer(0x21, 0x22, 1, 0, b"")
        yield device
    finally:
        for interface in reversed(claimed):
            try:
                usb.util.release_interface(device, interface)
            except usb.core.USBError:
                pass
        for interface in detached:
            try:
                device.attach_kernel_driver(interface)
            except usb.core.USBError:
                pass
        usb.util.dispose_resources(device)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--libusb")
    parser.add_argument("--wait", type=float, default=10)
    parser.add_argument("--seconds", type=float, default=20)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--pd", type=int, choices=[1, 2], help="Single passive capture on CMP input 1/2"
    )
    parser.add_argument(
        "--loader", action="store_true", help="Return to native loader after recording"
    )
    parser.add_argument(
        "--repeat-pd",
        action="store_true",
        help="Repeat the selected 15-second capture every 16 seconds",
    )
    args = parser.parse_args()
    counts: dict[str, int] = {}
    invalid = 0
    with connect(args.libusb, args.wait) as device, args.output.open("w") as output:
        device.write(0x01, b"status\nstream on\n", timeout=2000)
        if args.pd:
            device.write(0x01, f"pd {args.pd}\n".encode(), timeout=2000)
        next_pd = time.monotonic() + 16
        synchronized = False
        pending = bytearray()
        deadline = time.monotonic() + args.seconds
        while time.monotonic() < deadline:
            if args.repeat_pd and args.pd and time.monotonic() >= next_pd:
                device.write(0x01, f"pd {args.pd}\n".encode(), timeout=2000)
                next_pd = time.monotonic() + 16
            try:
                pending.extend(device.read(0x82, 64, timeout=500))
            except usb.core.USBTimeoutError:
                continue
            while b"\n" in pending:
                line, _, rest = pending.partition(b"\n")
                pending = bytearray(rest)
                if not line.strip():
                    continue
                try:
                    record = json.loads(line)
                    kind = record["type"]
                except (ValueError, KeyError, TypeError):
                    if synchronized:
                        invalid += 1
                    continue
                if not synchronized:
                    synchronized = (
                        kind == "reply" and record.get("message") == "km003c-cdc-v1"
                    )
                    if not synchronized:
                        continue
                output.write(json.dumps(record, separators=(",", ":")) + "\n")
                output.flush()
                counts[kind] = counts.get(kind, 0) + 1
            if len(pending) > 4096:
                raise RuntimeError("CDC record exceeds limit")
        device.write(0x01, b"stream off\n", timeout=2000)
        if args.loader:
            device.write(0x01, b"loader\n", timeout=2000)
    print(json.dumps({"records": counts, "invalid_lines": invalid}), file=sys.stderr)
    if not counts.get("adc"):
        raise RuntimeError("No ADC records received")
    if invalid:
        raise RuntimeError("Malformed CDC records received")
    if args.pd and not (
        counts.get("pd_summary")
        or counts.get("pd_capture_error")
        or counts.get("pd_init_error")
    ):
        raise RuntimeError("No PD capture result received")


if __name__ == "__main__":
    main()
