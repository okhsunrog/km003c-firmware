# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "pyusb>=1.3.1",
# ]
# ///
"""Verify the experimental KM003C CDC echo over libusb (no serial driver needed)."""

import argparse
import json
import time
from pathlib import Path

import usb.backend.libusb1
import usb.core
import usb.util


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--libusb", help="Explicit path to a native libusb shared library"
    )
    parser.add_argument(
        "--wait", type=float, default=30, help="Discovery timeout in seconds"
    )
    parser.add_argument("--output", type=Path, help="Write verified results as JSON")
    args = parser.parse_args()
    backend = (
        usb.backend.libusb1.get_backend(find_library=lambda _: args.libusb)
        if args.libusb
        else usb.backend.libusb1.get_backend()
    )
    if backend is None:
        raise RuntimeError(
            "Cannot load libusb; supply its native library path with --libusb"
        )
    deadline = time.monotonic() + args.wait
    device = None
    while time.monotonic() < deadline:
        try:
            candidate = usb.core.find(
                idVendor=0x5FC9, idProduct=0x0063, backend=backend
            )
            if (
                candidate
                and candidate.iProduct == 2
                and usb.util.get_string(candidate, candidate.iProduct)
                == "KM003C Rust USB test"
            ):
                device = candidate
                break
        except (usb.core.USBError, ValueError):
            pass
        time.sleep(0.1)
    if device is None:
        raise RuntimeError("Rust CDC did not enumerate before the discovery deadline")
    claimed = []
    detached = []
    results = []
    try:
        for interface in (0, 1):
            if device.is_kernel_driver_active(interface):
                device.detach_kernel_driver(interface)
                detached.append(interface)
            usb.util.claim_interface(device, interface)
            claimed.append(interface)
        device.ctrl_transfer(0x21, 0x22, 1, 0, b"")  # CDC SET_CONTROL_LINE_STATE: DTR
        for size in (1, 23, 64, 7, 64, 17, 128, 256, 512):
            payload = bytes((index * 17 + size) % 256 for index in range(size))
            if device.write(0x01, payload, timeout=2000) != size:
                raise RuntimeError(f"Short CDC write for {size}-byte payload")
            received = bytearray()
            while len(received) < size:
                received.extend(device.read(0x82, size - len(received), timeout=2000))
            if received != payload:
                raise RuntimeError(f"CDC echo mismatch for {size}-byte payload")
            results.append({"bytes": size, "matched": True})
            print(f"echo matched {size}", flush=True)
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
    record = {"transport": "libusb CDC ACM; DTR=1", "results": results}
    if args.output:
        args.output.write_text(json.dumps(record, indent=2) + "\n")
    print("PASS: CDC echo", flush=True)


if __name__ == "__main__":
    main()
