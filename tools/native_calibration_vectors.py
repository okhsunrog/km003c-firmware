# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "unicorn>=2.1.4",
# ]
# ///
"""Generate correction vectors by executing the preserved stock 2.0.7 code.

Synthetic RAM and an instruction limit; no hardware or native USB access.
Stop before queue/timestamp processing, after instantaneous V/I correction.
"""

import argparse
import hashlib
from pathlib import Path
import struct

from unicorn import UC_ARCH_ARM, UC_HOOK_CODE, UC_MODE_MCLASS, UC_MODE_THUMB, Uc
from unicorn.arm_const import (
    UC_ARM_REG_LR,
    UC_ARM_REG_PC,
    UC_ARM_REG_R0,
    UC_ARM_REG_R1,
    UC_ARM_REG_SP,
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument(
        "--settings",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "crates/km003c-drivers/tests/data/synthetic-calibration.bin",
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    code = args.binary.read_bytes()
    expected = "c63057eaf08b8625e7ac2fc7270c06d75515e78fdbdd46080bdf4b696984ec67"
    digest = hashlib.sha256(code).hexdigest()
    if digest != expected:
        raise ValueError(
            "Unsupported image hash: addresses are specific to the preserved installed 2.0.7 image"
        )
    settings = args.settings.read_bytes()
    if len(settings) != 96:
        raise ValueError("Expected a 96-byte settings-A block")
    cpu = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
    for address, size in [(0, 0x80000), (0x1FFF0000, 0x50000), (0x10000000, 0x1000)]:
        cpu.mem_map(address, size)
    cpu.mem_write(0x4000, code)
    cpu.mem_write(0x1FFFB5A8, settings)

    def stop(machine: Uc, address: int, size: int, data: object) -> None:
        if address == 0x4D244:
            machine.emu_stop()

    cpu.hook_add(UC_HOOK_CODE, stop)
    lines = [
        f"# Firmware SHA256 {digest}",
        f"# Settings SHA256 {hashlib.sha256(settings).hexdigest()}",
        "# Native entry 0x17b1c; stop 0x4d244; input V_uV,I_uA,output V_uV,I_uA",
    ]
    volts = [
        0,
        1499999,
        1500000,
        14999999,
        15000000,
        24999999,
        25000000,
        34999999,
        35000000,
        44999999,
        45000000,
        50000000,
    ]
    currents = [
        -8200001,
        -8200000,
        -8199999,
        -6100001,
        -6100000,
        -6099999,
        -4100001,
        -4100000,
        -4099999,
        -2100001,
        -2100000,
        -2099999,
        -1,
        0,
        1,
        2099999,
        2100000,
        2100001,
        4099999,
        4100000,
        4100001,
        6099999,
        6100000,
        6100001,
        8199999,
        8200000,
        8200001,
    ]
    for volts_in in volts:
        for current_in in currents:
            for register, value in [
                (UC_ARM_REG_SP, 0x2001FFF0),
                (UC_ARM_REG_LR, 0x10000001),
                (UC_ARM_REG_R0, volts_in),
                (UC_ARM_REG_R1, current_in & 0xFFFFFFFF),
            ]:
                cpu.reg_write(register, value)
            cpu.emu_start(0x17B1D, 0x10000000, count=50000)
            if cpu.reg_read(UC_ARM_REG_PC) != 0x4D244:
                raise RuntimeError("Native correction did not reach the expected stop")
            voltage, current = struct.unpack("<Ii", cpu.mem_read(0x1FFFACF0, 8))
            lines.append(f"{volts_in},{current_in},{voltage},{current}")
    args.output.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
