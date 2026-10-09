import sys
from pathlib import Path
sys.path.insert(0, str(Path('tools/re').resolve()))
from probe_engine_config import read_va
from probe_pua import EXE_SHA256
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

exe = Path('re_workspace/Against_Rome.exe').read_bytes()
md = Cs(CS_ARCH_X86, CS_MODE_32)

def analyze_reader(name, va, length=0x200):
    code = read_va(exe, va, length)
    insts = list(md.disasm(code, va))
    print(f'=== {name} Reader ({hex(va)}) ===')
    # find loops or fread calls
    for i in insts:
        # show memory reads/writes or calls
        if i.mnemonic in ('call', 'rep', 'movzx', 'imul') or ('[' in i.op_str and any(r in i.op_str for r in ['eax', 'ecx', 'edx', 'esi', 'edi', 'ebx'])):
            print(f'  {i.address:#x}: {i.mnemonic:8} {i.op_str}')

analyze_reader('formatio (0x48d0f0)', 0x48d0f0, 0x180)
analyze_reader('lager (0x48d2d0)', 0x48d2d0, 0x220)
analyze_reader('biglager (0x48d590)', 0x48d590, 0x180)
analyze_reader('stat (0x48e720)', 0x48e720, 0x180)
