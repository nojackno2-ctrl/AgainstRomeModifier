import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".codex" / "pydeps"))

import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import X86_OP_IMM, X86_OP_MEM


EXE = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "AppData/Local/Temp/AgainstRome_RE/Against_Rome.exe"


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
    code = data[auto.PointerToRawData : auto.PointerToRawData + auto.SizeOfRawData]
    text_start = image + auto.VirtualAddress

    imports = {}
    for entry in getattr(pe, "DIRECTORY_ENTRY_IMPORT", []):
        dll = entry.dll.decode(errors="ignore")
        for imp in entry.imports:
            name = imp.name.decode(errors="ignore") if imp.name else f"ord_{imp.ordinal}"
            imports[imp.address] = f"{dll}!{name}"

    interesting = [
        "CreateFile", "ReadFile", "WriteFile", "CloseHandle",
        "fopen", "fread", "fscanf", "sscanf", "strstr", "strcmp", "lstrcmp",
        "GetPrivateProfile", "WritePrivateProfile",
    ]
    print("Interesting imports:")
    for addr, name in sorted(imports.items()):
        if any(tok.lower() in name.lower() for tok in interesting):
            print(f"  0x{addr:X} {name}")

    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.detail = True
    calls = []
    for insn in md.disasm(code, text_start):
        if insn.mnemonic not in ("call", "jmp"):
            continue
        for op in insn.operands:
            target = None
            if op.type == X86_OP_IMM:
                target = op.imm & 0xFFFFFFFF
            elif op.type == X86_OP_MEM and op.mem.disp:
                target = op.mem.disp & 0xFFFFFFFF
            if target in imports and any(tok.lower() in imports[target].lower() for tok in interesting):
                calls.append((insn.address, imports[target], insn.mnemonic, insn.op_str))

    print(f"\nInteresting import calls/jumps: {len(calls)}")
    for addr, name, m, ops in calls[:300]:
        print(f"  0x{addr:X}: {m:<5} {ops:<18} {name}")


if __name__ == "__main__":
    main()
