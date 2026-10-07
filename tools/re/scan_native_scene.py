"""Read-only native scene investigation against an explicit repository EXE copy.

Requires pefile and capstone. Output is analysis evidence, not an API contract.
Ranges are virtual addresses: --range 0x400000:0x400100 (repeatable).
"""
import argparse
import hashlib
import re
from pathlib import Path

import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import X86_OP_IMM, X86_OP_MEM


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path)
    parser.add_argument("--range", action="append", default=[], dest="ranges")
    parser.add_argument("--target", action="append", default=[], help="Additional virtual address to find references to")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.output and args.output.resolve() == args.exe.resolve():
        parser.error("Output must not overwrite the input executable")
    data = args.exe.read_bytes()
    pe = pefile.PE(data=data)
    if pe.FILE_HEADER.Machine != 0x14C:
        parser.error("Expected an x86 PE executable")
    base = pe.OPTIONAL_HEADER.ImageBase
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    decoder.detail = True
    decoder.skipdata = True
    report = [f"EXE: {args.exe.resolve()}", f"SHA256: {hashlib.sha256(data).hexdigest()}",
              "Static candidate references; linear decoding is not function-boundary proof."]
    targets = {}
    for match in re.finditer(rb"[ -~]{5,}", data):
        value = match.group()
        if not any(token in value.lower() for token in
                   (b".alr", b".apt", b"textureeditor", b"animation:", b"vertexcolors")):
            continue
        address = base + pe.get_rva_from_offset(match.start())
        targets[address] = value.decode("ascii")
        report.append(f"STRING 0x{address:08X} {targets[address]}")
    for value in args.target:
        address = int(value, 0)
        targets[address] = f"requested target 0x{address:08X}"
    for section in pe.sections:
        if not section.Characteristics & 0x20000000:
            continue
        for instruction in decoder.disasm(section.get_data(), base + section.VirtualAddress):
            if instruction.id == 0:  # Capstone skip-data pseudo-instruction has no operands.
                continue
            values = set()
            for operand in instruction.operands:
                if operand.type == X86_OP_IMM:
                    values.add(operand.imm & 0xFFFFFFFF)
                elif operand.type == X86_OP_MEM:
                    values.add(operand.mem.disp & 0xFFFFFFFF)
            for value in values & targets.keys():
                report.append(f"XREF 0x{instruction.address:08X} {instruction.mnemonic} {instruction.op_str} -> {targets[value]}")
    for span in args.ranges:
        start, end = (int(part, 0) for part in span.split(":"))
        section = next((s for s in pe.sections if base + s.VirtualAddress <= start <
                        base + s.VirtualAddress + s.SizeOfRawData), None)
        if section is None or end <= start or end > base + section.VirtualAddress + section.SizeOfRawData:
            parser.error(f"Range must stay within one file-backed PE section: {span}")
        offset = pe.get_offset_from_rva(start - base)
        report.append(f"RANGE 0x{start:08X}:0x{end:08X}")
        for instruction in decoder.disasm(data[offset:offset + end - start], start):
            report.append(f"0x{instruction.address:08X} {instruction.bytes.hex(' '):<30} {instruction.mnemonic} {instruction.op_str}")
    output = "\n".join(report) + "\n"
    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
