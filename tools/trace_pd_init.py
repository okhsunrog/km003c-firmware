# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "unicorn>=2.1.4",
# ]
# ///
"""Trace CPU MMIO writes of a preserved Thumb initializer, without hardware.

This is an instruction emulator with zero-filled synthetic MMIO, not a model
of DMA, comparator edges, reset register values or real interrupt timing.
"""

import argparse
import hashlib
import json
from pathlib import Path

from unicorn import UC_ARCH_ARM, UC_HOOK_MEM_WRITE, UC_MODE_MCLASS, UC_MODE_THUMB, Uc
from unicorn.arm_const import UC_ARM_REG_LR, UC_ARM_REG_PC, UC_ARM_REG_SP


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "binary", type=Path, help="Plaintext application starting at 0x4000"
    )
    parser.add_argument("--entry", type=lambda s: int(s, 0), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    code = args.binary.read_bytes()
    machine = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
    for address, size in [
        (0, 0x80000),
        (0x1FFF0000, 0x50000),
        (0x40000000, 0x60000),
        (0x42000000, 0x1000000),
        (0xE000E000, 0x2000),
        (0x10000000, 0x1000),
    ]:
        machine.mem_map(address, size)
    machine.mem_write(0x4000, code)
    writes = []

    def on_write(
        cpu: Uc, access: int, address: int, size: int, value: int, context: object
    ) -> None:
        if not 0x40000000 <= address < 0x44000000:
            return
        writes.append(
            {
                "pc": hex(cpu.reg_read(UC_ARM_REG_PC)),
                "address": hex(address),
                "size": size,
                "value": hex(value),
            }
        )
        if 0x42000000 <= address < 0x44000000:
            # Peripheral bit-band writes update the synthetic underlying byte.
            # No other peripheral behavior is modeled.
            byte = 0x40000000 + (address - 0x42000000) // 32
            bit = (address % 32) // 4
            previous = cpu.mem_read(byte, 1)[0]
            cpu.mem_write(
                byte, bytes([(previous & ~(1 << bit)) | ((value & 1) << bit)])
            )

    machine.hook_add(UC_HOOK_MEM_WRITE, on_write)
    machine.reg_write(UC_ARM_REG_SP, 0x2003F000)
    machine.reg_write(UC_ARM_REG_LR, 0x10000001)
    machine.emu_start(args.entry | 1, 0x10000000, count=200000)
    if machine.reg_read(UC_ARM_REG_PC) != 0x10000000:
        raise RuntimeError("Initializer did not return within the instruction limit")
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "pd-init-writes.json").write_text(
        json.dumps(writes, indent=2) + "\n"
    )
    (args.output / "trace-provenance.json").write_text(
        json.dumps(
            {
                "sha256": hashlib.sha256(code).hexdigest(),
                "entry": hex(args.entry),
                "write_count": len(writes),
                "model": "zero-filled MMIO; CPU write trace only",
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
