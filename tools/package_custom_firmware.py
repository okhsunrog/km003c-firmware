"""Create an offline .mencrypt package; validate it with the Rust CLI before use."""

import argparse
import struct
from pathlib import Path

from Crypto.Cipher import AES

from km003c_analysis.device import MEMORY_READ_KEY
from km003c_analysis.device.protocol import crc32


def package(data: bytes) -> bytes:
    """Package a linked application, preserving its embedded identity."""
    if not 0x460 <= len(data) <= 0x6FFFE:
        raise ValueError("Application size outside loader limits")
    if data[0x420:0x424] != struct.pack("<I", 0x4000):
        raise ValueError("Expected application base 0x4000")
    if data[0x428:0x430] != b"\xff" * 8:
        raise ValueError("Expected uncommitted size/CRC placeholders")
    identity = []
    for offset in (0x430, 0x43C, 0x448):
        field = data[offset : offset + 12].split(b"\0", 1)[0]
        if not 0 < len(field) < 12:
            raise ValueError("Invalid embedded identity length")
        identity.append(field.decode("ascii"))
    if identity[0] != "KM003C":
        raise ValueError("Unexpected model")
    fields = [
        "Firmware file",
        "ChargerLab",
        "V1.0",
        *identity,
        f"{crc32(data):08X}",
        str(len(data)),
        "",
        "Custom Rust bring-up image",
    ]
    out = bytearray(struct.pack(">I", len(fields)))
    for value in fields:
        encoded = value.encode("utf-16-be")
        out.extend(struct.pack(">I", len(encoded)))
        out.extend(encoded)
    padded = data + b"\xff" * (-len(data) % 16)
    encrypted = AES.new(MEMORY_READ_KEY, AES.MODE_ECB).encrypt(padded)
    out.extend(struct.pack(">I", len(encrypted)))
    out.extend(encrypted)
    return bytes(out)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_bytes(package(args.binary.read_bytes()))
