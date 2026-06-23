import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".codex" / "pydeps"))

import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import X86_OP_IMM, X86_OP_MEM


ROOT = Path(__file__).resolve().parents[2]
EXE = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "AppData/Local/Temp/AgainstRome_RE/Against_Rome.exe"


def find_all(data: bytes, needle: bytes):
    pos = 0
    while True:
        idx = data.find(needle, pos)
        if idx < 0:
            return
        yield idx
        pos = idx + 1


def off_to_rva(pe, off: int):
    for s in pe.sections:
        raw = s.PointerToRawData
        size = max(s.SizeOfRawData, s.Misc_VirtualSize)
        if raw <= off < raw + size:
            return s.VirtualAddress + (off - raw)
    return off


def rva_to_off(pe, rva: int):
    for s in pe.sections:
        va = s.VirtualAddress
        size = max(s.SizeOfRawData, s.Misc_VirtualSize)
        if va <= rva < va + size:
            return s.PointerToRawData + (rva - va)
    return rva


def section_by_name(pe, name: bytes):
    for s in pe.sections:
        if s.Name.rstrip(b"\0") == name:
            return s
    raise KeyError(name)


def main():
    pe = pefile.PE(str(EXE))
    image = pe.OPTIONAL_HEADER.ImageBase
    data = EXE.read_bytes()
    auto = section_by_name(pe, b"AUTO")
    dgroup = section_by_name(pe, b"DGROUP")
    text_start = image + auto.VirtualAddress
    text_end = text_start + auto.Misc_VirtualSize
    dg_start = image + dgroup.VirtualAddress
    dg_end = dg_start + dgroup.Misc_VirtualSize

    targets = {}
    for label in [b"SYSTEM/ress.ini", b"[objres]", b"[volkres]", b"[maxskills]"]:
        for off in find_all(data, label):
            va = image + off_to_rva(pe, off)
            targets[label.decode("ascii")] = va
            print(f"string {label.decode('ascii'):<18} off=0x{off:X} va=0x{va:X}")

    code = data[auto.PointerToRawData : auto.PointerToRawData + auto.SizeOfRawData]
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.detail = True

    refs = []
    near_refs = []
    for insn in md.disasm(code, text_start):
        for op in insn.operands:
            vals = []
            if op.type == X86_OP_IMM:
                vals.append(op.imm & 0xFFFFFFFF)
            elif op.type == X86_OP_MEM:
                if op.mem.disp:
                    vals.append(op.mem.disp & 0xFFFFFFFF)
            for val in vals:
                if dg_start <= val < dg_end:
                    refs.append((insn.address, val, insn.mnemonic, insn.op_str))
                    if any(abs(val - t) <= 0x200 for t in targets.values()):
                        near_refs.append((insn.address, val, insn.mnemonic, insn.op_str))

    print(f"\nDGROUP immediate refs: {len(refs)}")
    print("Nearby ress.ini refs:")
    for addr, val, m, ops in near_refs[:200]:
        print(f"  0x{addr:X}: {m:<7} {ops:<35} ; -> 0x{val:X}")

    print("\nExact target refs:")
    for name, target in targets.items():
        exact = [r for r in refs if r[1] == target]
        print(f"{name}: {len(exact)}")
        for addr, val, m, ops in exact[:50]:
            print(f"  0x{addr:X}: {m:<7} {ops}")

    # Print clusters of DGROUP refs around the ress parser string refs.
    if near_refs:
        print("\nContext around nearby refs:")
        for addr, _, _, _ in near_refs[:20]:
            start = max(text_start, addr - 0x60)
            end = min(text_end, addr + 0x80)
            off = rva_to_off(pe, start - image)
            blob = data[off : off + (end - start)]
            print(f"\n--- around 0x{addr:X} ---")
            for insn in md.disasm(blob, start):
                mark = "=>" if insn.address == addr else "  "
                print(f"{mark} 0x{insn.address:X}: {insn.mnemonic:<7} {insn.op_str}")


if __name__ == "__main__":
    main()
