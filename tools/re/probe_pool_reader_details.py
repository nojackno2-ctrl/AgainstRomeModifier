import sys, struct
from pathlib import Path
sys.path.insert(0, str(Path('tools/re').resolve()))
from probe_engine_config import read_va
from probe_pua import EXE_SHA256
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

exe = Path('re_workspace/Against_Rome.exe').read_bytes()
md = Cs(CS_ARCH_X86, CS_MODE_32)

targets = {
    'formatio': 0x48d0f0,
    'lager': 0x48d2d0,
    'biglager': 0x48d590,
    'way': 0x48dba0,
    'particle': 0x48dd90,
    'stat': 0x48e720,
    'light': 0x48ab70,
    'gametime': 0x48b0c0,
}

for name, va in targets.items():
    code = read_va(exe, va, 0x180)
    insts = list(md.disasm(code, va))
    print(f'==============================')
    print(f'=== {name} Reader ({hex(va)}) ===')
    print(f'==============================')
    for i in insts[:30]:
        print(f'{i.address:#x}: {i.bytes.hex():14} {i.mnemonic:8} {i.op_str}')
