# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "pyelftools>=0.33",
#     "unicorn>=2.1.4",
# ]
# ///
"""Execute compiled Thumb-2 PD scanners on the host differential corpus.

No USB/device access. ELF loading and native instruction emulation validate the
actual target kernels, including read/write bounds and output canaries.
"""

import argparse
import struct
from pathlib import Path

from elftools.elf.elffile import ELFFile
from unicorn import (
    Uc,
    UC_ARCH_ARM,
    UC_MODE_THUMB,
    UC_MODE_MCLASS,
    UC_HOOK_MEM_READ,
    UC_HOOK_MEM_WRITE,
)
from unicorn.arm_const import (
    UC_ARM_REG_R0,
    UC_ARM_REG_R1,
    UC_ARM_REG_R2,
    UC_ARM_REG_R3,
    UC_ARM_REG_SP,
    UC_ARM_REG_LR,
    UC_ARM_REG_R4,
    UC_ARM_REG_R5,
    UC_ARM_REG_R6,
    UC_ARM_REG_R7,
    UC_ARM_REG_R8,
    UC_ARM_REG_R9,
    UC_ARM_REG_R10,
    UC_ARM_REG_R11,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("elf", type=Path)
    parser.add_argument("vectors", type=Path)
    parser.add_argument(
        "--kernel", action="append", help="Validate only this named target kernel"
    )
    parser.add_argument(
        "--profile", action="store_true", help="Exercise non-null DWT timestamp output"
    )
    args = parser.parse_args()
    cpu = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
    cpu.mem_map(0, 0x80000)
    cpu.mem_map(0x1FFF8000, 0x30000)
    cpu.mem_map(0x10000000, 0x1000)
    cpu.mem_map(0xE0001000, 0x1000)
    cpu.mem_write(0xE0001004, struct.pack("<I", 12345))
    with args.elf.open("rb") as stream:
        elf = ELFFile(stream)
        for section in elf.iter_sections():
            if (
                section["sh_flags"] & 2
                and section["sh_type"] != "SHT_NOBITS"
                and section["sh_size"]
            ):
                cpu.mem_write(section["sh_addr"], section.data())
        symtab = elf.get_section_by_name(".symtab")
        entries = {
            n: symtab.get_symbol_by_name(n)[0]["st_value"]
            for n in args.kernel
            or [
                "km_pd_symbols_scalar",
                "km_pd_symbols_dsp_word",
                "km_pd_symbols_dsp_half",
                "km_pd_symbols_dsp4_word",
                "km_pd_symbols_dsp4_half",
                "km_pd_bytes_scalar",
                "km_pd_bytes_unroll",
                "km_pd_bytes_hoist",
                "km_pd_bytes_pair",
                "km_pd_bytes_preamble",
                "km_pd_bytes_hoist_preamble",
            ]
        }
    src, dst, ret, stack = 0x20020002, 0x20022004, 0x10000000, 0x2001F000
    profile = 0x20023004
    saved = [
        UC_ARM_REG_R4,
        UC_ARM_REG_R5,
        UC_ARM_REG_R6,
        UC_ARM_REG_R7,
        UC_ARM_REG_R8,
        UC_ARM_REG_R9,
        UC_ARM_REG_R10,
        UC_ARM_REG_R11,
    ]
    bounds = [0, 69]

    def check_read(cpu, access, address, size, value, user):
        if (
            0x20020000 <= address < 0x20021000
            and not src <= address <= src + bounds[0] - size
        ):
            raise RuntimeError(
                f"input read outside slice: {address:x}+{size}, length {bounds[0]}"
            )

    def check_write(cpu, access, address, size, value, user):
        if (
            0x20022000 <= address < 0x20023000
            and not dst <= address <= dst + bounds[1] - size
        ):
            raise RuntimeError(f"output write outside buffer: {address:x}+{size}")

    cpu.hook_add(UC_HOOK_MEM_READ, check_read)
    cpu.hook_add(UC_HOOK_MEM_WRITE, check_write)
    vectors = 0
    for line in args.vectors.read_text().splitlines():
        status, symbols, values = line.split("\t")
        data = [int(s) for s in values.split(",")] if values else []
        payload = struct.pack(f"<{len(data)}H", *data)
        bounds[0] = len(payload)
        cpu.mem_write(src, payload)
        for name, address in entries.items():
            fused = name.startswith("km_pd_bytes_")
            bounds[1] = 35 if fused else 69
            cpu.mem_write(dst - 4, b"\xa5" * 80)
            cpu.mem_write(profile - 4, b"\xa5" * 16)
            for i, reg in enumerate(saved):
                cpu.reg_write(reg, 0x12340000 + i)
            for reg, value in [
                (UC_ARM_REG_R0, src),
                (UC_ARM_REG_R1, src + len(payload)),
                (UC_ARM_REG_R2, dst),
                (UC_ARM_REG_R3, profile if args.profile else 0),
                (UC_ARM_REG_SP, stack),
                (UC_ARM_REG_LR, ret | 1),
            ]:
                cpu.reg_write(reg, value)
            cpu.emu_start(address | 1, ret, count=200000)
            actual = cpu.reg_read(UC_ARM_REG_R0)
            assert cpu.reg_read(UC_ARM_REG_SP) == stack, f"{name}: SP changed"
            assert all(
                cpu.reg_read(reg) == 0x12340000 + i for i, reg in enumerate(saved)
            ), f"{name}: callee-saved register changed"
            if args.profile:
                assert bytes(cpu.mem_read(profile, 4)) == struct.pack("<I", 12345)
                assert bytes(cpu.mem_read(profile - 4, 4)) == b"\xa5" * 4
                assert bytes(cpu.mem_read(profile + 8, 4)) == b"\xa5" * 4
            expected = int(status, 16)
            expected_bytes = bytes.fromhex(symbols)
            # The fused kernel rejects an invalid 4b5b symbol immediately;
            # the symbol scanner defers that error to Rust validation.
            if fused:
                nibbles = {
                    s: n
                    for n, s in enumerate(
                        [30, 9, 20, 21, 10, 11, 14, 15, 18, 19, 22, 23, 26, 27, 28, 29]
                    )
                }
                bad = any(s not in nibbles for s in expected_bytes if s != 13)
                if expected & 0x80000000 or bad:
                    if not actual & 0x80000000:
                        raise RuntimeError(
                            f"{name} vector {vectors}: accepted damaged symbols"
                        )
                    expected = actual
                    expected_bytes = b""
                else:
                    payload_symbols = expected_bytes[:-1]
                    expected_bytes = bytes(
                        nibbles[payload_symbols[i]]
                        | (nibbles[payload_symbols[i + 1]] << 4)
                        for i in range(0, len(payload_symbols) - 1, 2)
                    ) + (b"\x0d" if expected_bytes else b"")
            if actual != expected:
                raise RuntimeError(
                    f"{name} vector {vectors}: status {actual:x} != {expected:x}"
                )
            if bytes(cpu.mem_read(dst, len(expected_bytes))) != expected_bytes:
                raise RuntimeError(f"{name} vector {vectors}: symbols differ")
            if (
                bytes(cpu.mem_read(dst - 4, 4)) != b"\xa5" * 4
                or bytes(cpu.mem_read(dst + bounds[1], 7)) != b"\xa5" * 7
            ):
                raise RuntimeError(f"{name} vector {vectors}: output canary changed")
        vectors += 1
    print(
        f"PASS: {len(entries)} target scanners, {vectors} vectors, bounded input/output"
    )


if __name__ == "__main__":
    main()
