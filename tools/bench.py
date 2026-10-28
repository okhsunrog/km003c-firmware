"""Short bench capture command; all long paths and host defaults live here.

Run with uv run tools/bench.py NAME [--pd 1|2]. Hardware writes/firmware updates
are deliberately outside this read/record helper; --loader is an explicit reset.
"""

import argparse
import ctypes
import os
import subprocess
from ctypes.util import find_library
from pathlib import Path


def libusb() -> str | None:
    if value := os.environ.get("LIBUSB_LIBRARY"):
        return value
    if value := find_library("usb-1.0"):
        return value
    # NixOS libraries need not appear in the conventional dynamic linker cache.
    for path in sorted(
        Path("/nix/store").glob("*-libusb-*/lib/libusb-1.0.so"), reverse=True
    ):
        try:
            ctypes.CDLL(str(path))
        except OSError:
            continue
        return str(path)
    return None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("name", help="Capture name relative to the bench directory")
    parser.add_argument("--pd", choices=["1", "2", "auto"])
    parser.add_argument("--seconds", type=float, default=30)
    parser.add_argument("--pause-read", type=float, default=0)
    parser.add_argument("--loader", action="store_true")
    parser.add_argument(
        "--dry-run", action="store_true", help="Print command without accessing USB"
    )
    args = parser.parse_args()
    name = Path(args.name)
    if name.is_absolute() or ".." in name.parts:
        parser.error("name must stay within the bench directory")
    root = Path(
        os.environ.get(
            "KM003C_BENCH_ROOT",
            str(Path.home() / "tmp_zfs/codex_tmp/km003c-rust-bringup/bench"),
        )
    )
    output = (root / name).with_suffix(".ndjson")
    command = [
        "uv",
        "run",
        str(Path(__file__).with_name("usb_telemetry.py")),
        "--seconds",
        str(args.seconds),
        "--output",
        str(output),
    ]
    if native := libusb():
        command += ["--libusb", native]
    if args.pd:
        command += ["--pd", str(args.pd), "--pd-continuous"]
    if args.pause_read:
        command += ["--pause-read", str(args.pause_read)]
    if args.loader:
        command.append("--loader")
    if args.dry_run:
        import shlex

        print(shlex.join(command))
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    print(f"Recording {output}", flush=True)
    raise SystemExit(subprocess.call(command))


if __name__ == "__main__":
    main()
