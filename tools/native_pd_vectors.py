# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "unicorn>=2.1.4",
# ]
# ///
"""Execute stock 2.0.7 PD encoding in Unicorn, with synthetic CRC MMIO only.

No USB/hardware access. The exact firmware hash is required. Timer/DMA, GPIO,
analog levels and interrupt timing are NOT emulated by these offline vectors.
"""

import argparse
import hashlib
import os
import random
import struct
import zlib
from pathlib import Path

from unicorn import (
    UC_ARCH_ARM,
    UC_HOOK_MEM_READ,
    UC_HOOK_MEM_WRITE,
    UC_MODE_MCLASS,
    UC_MODE_THUMB,
    Uc,
)
from unicorn.arm_const import (
    UC_ARM_REG_LR,
    UC_ARM_REG_PC,
    UC_ARM_REG_R0,
    UC_ARM_REG_R1,
    UC_ARM_REG_R2,
    UC_ARM_REG_R3,
    UC_ARM_REG_SP,
)

SHA256 = "c63057eaf08b8625e7ac2fc7270c06d75515e78fdbdd46080bdf4b696984ec67"
RETURN = 0x10000000
INPUT = 0x20020000
OUTPUT = 0x20021000


class Encoder:
    def __init__(self, binary: bytes):
        if hashlib.sha256(binary).hexdigest() != SHA256:
            raise ValueError(
                "Addresses are specific to the preserved installed stock 2.0.7 image"
            )
        self.cpu = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
        for address, size in [
            (0, 0x80000),
            (0x1FFF0000, 0x50000),
            (RETURN, 0x1000),
            (0x40000000, 0x60000),
            (0x42000000, 0x1000000),
        ]:
            self.cpu.mem_map(address, size)
        self.cpu.mem_write(0x4000, binary)
        self.cpu.reg_write(UC_ARM_REG_SP, 0x2003F000)
        # Run the actual scatter-load/decompression table, stopping before main.
        self.cpu.emu_start(0x4C49, 0x4C6A, count=2_000_000)
        self.check_return(0x4C6A)
        # Native initializer selects CRC32 with reflected input/output and XOR.
        self.call(0x5DC8)
        assert self.cpu.mem_read(0x40008C00, 4) == struct.pack("<I", 0x1E)
        self.crc = 0
        self.cpu.hook_add(UC_HOOK_MEM_WRITE, self.write_crc)
        self.cpu.hook_add(UC_HOOK_MEM_READ, self.read_crc)

    def check_return(self, expected: int):
        if self.cpu.reg_read(UC_ARM_REG_PC) != expected:
            raise RuntimeError("Native code did not reach its bounded stop")

    def call(self, address: int):
        self.cpu.reg_write(UC_ARM_REG_SP, 0x2003F000)
        self.cpu.reg_write(UC_ARM_REG_LR, RETURN | 1)
        self.cpu.emu_start(address | 1, RETURN, count=100_000)
        self.check_return(RETURN)

    def write_crc(self, cpu, access, address, size, value, context):
        # CRC peripheral model: native writes the seed/result register and
        # feeds little-endian byte/word data into DAT0. zlib is independent of
        # the Rust CRC implementation; it models CR=0x1e, not all CRC modes.
        if address == 0x40008C04:
            self.crc = value ^ 0xFFFFFFFF
        elif address == 0x40008C80:
            self.crc = zlib.crc32(value.to_bytes(size, "little"), self.crc)
        elif 0x40000000 <= address < 0x44000000:
            raise RuntimeError(f"Unexpected encoder MMIO write: {address:#x}")

    def read_crc(self, cpu, access, address, size, value, context):
        if address == 0x40008C04:
            cpu.mem_write(address, struct.pack("<I", self.crc))

    def encode(self, selector: int, wire: bytes) -> list[int]:
        self.cpu.mem_write(INPUT, wire + b"\0" * 8)
        for register, value in [
            (UC_ARM_REG_R0, OUTPUT),
            (UC_ARM_REG_R1, selector),
            (UC_ARM_REG_R2, INPUT),
            (UC_ARM_REG_R3, len(wire)),
        ]:
            self.cpu.reg_write(register, value)
        self.call(0x1FFF8184)
        count = self.cpu.reg_read(UC_ARM_REG_R0)
        if not 0 < count <= 860:
            raise RuntimeError(f"Unexpected encoded length: {count}")
        if selector < 5:
            appended = bytes(self.cpu.mem_read(INPUT + len(wire), 4))
            assert appended == struct.pack("<I", zlib.crc32(wire))
        return list(struct.unpack(f"<{count}H", self.cpu.mem_read(OUTPUT, count * 2)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path(
            os.environ.get(
                "KM003C_STOCK_207",
                str(
                    Path.home()
                    / "tmp_zfs/codex_tmp/km003c-rust-bringup/adc-207/verified-original-app.bin"
                ),
            )
        ),
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    encoder = Encoder(args.binary.read_bytes())
    random_source = random.Random(207)
    lines = [
        f"# Stock 2.0.7 SHA256 {SHA256}",
        "# selector<TAB>wire_hex<TAB>TimerA2 PERAR values (not measured edge intervals)",
        "# CRC32 MMIO is modeled with zlib; no DMA, timer or electrical model.",
    ]
    for selector in range(7):
        if selector >= 5:
            lines.append(
                f"{selector}\t-\t" + ",".join(map(str, encoder.encode(selector, b"")))
            )
            continue
        for objects in range(8):
            header = 0x81 if objects == 0 else (objects << 12) | 0x81
            for pattern in range(4):
                payload = [0, 255, None, None][pattern]
                data = bytes(
                    payload
                    if payload is not None
                    else (i * 17 if pattern == 2 else random_source.randrange(256))
                    & 255
                    for i in range(objects * 4)
                )
                wire = struct.pack("<H", header) + data
                periods = encoder.encode(selector, wire)
                lines.append(
                    f"{selector}\t{wire.hex()}\t" + ",".join(map(str, periods))
                )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(lines) + "\n")
    print(f"Wrote {len(lines) - 3} native vectors to {args.output}")


if __name__ == "__main__":
    main()
