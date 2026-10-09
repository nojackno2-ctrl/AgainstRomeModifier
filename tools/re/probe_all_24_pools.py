import sys, json, hashlib
from pathlib import Path

# Add tools/re to path
sys.path.insert(0, str(Path('tools/re').resolve()))
from probe_engine_config import read_va
from probe_pua import EXE_SHA256
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

exe = Path('re_workspace/Against_Rome.exe').read_bytes()
assert hashlib.sha256(exe).hexdigest() == EXE_SHA256

md = Cs(CS_ARCH_X86, CS_MODE_32)
code = read_va(exe, 0x48e960, 0xc00)
insts = list(md.disasm(code, 0x48e960))

files_24 = [
    'light.dat', 'gametime.dat', 'rain.dat', 'hagel.dat', 'snow.dat', 'flash.dat',
    'objects.dat', 'position.dat', 'anim.dat', 'gfxtype.dat', 'action.dat', 'objdata.dat',
    'hirarchy.dat', 'formatio.dat', 'lager.dat', 'engine.dat', 'fow.dat', 'fowreq.dat',
    'way.dat', 'particle.dat', 'explos.dat', 'hitex.dat', 'stat.dat', 'biglager.dat'
]

results = []
idx = 0
for file_name in files_24:
    while idx < len(insts) and not (insts[idx].mnemonic == 'call' and insts[idx].op_str == '0x415400'):
        idx += 1
    fopen_idx = idx
    reader_va = None
    buf_va = None
    k = fopen_idx + 1
    last_mov_eax = None
    while k < len(insts) and k < fopen_idx + 40:
        if insts[k].mnemonic == 'mov' and insts[k].op_str.startswith('eax, 0x'):
            last_mov_eax = insts[k].op_str.split(', ')[1]
        elif insts[k].mnemonic == 'push' and insts[k].op_str.startswith('0x'):
            val = int(insts[k].op_str, 16)
            if val >= 0x700000:
                buf_va = hex(val)
        elif insts[k].mnemonic == 'push' and insts[k].op_str == 'eax' and last_mov_eax:
            val = int(last_mov_eax, 16)
            if val >= 0x700000:
                buf_va = hex(val)
        elif insts[k].mnemonic == 'call' and insts[k].op_str not in ('0x4156d0', '0x415400', '0x415420', '0x5c5de0'):
            reader_va = insts[k].op_str
            break
        k += 1
    results.append({'file': file_name, 'reader': reader_va, 'buffer': buf_va})
    idx = k

for r in results:
    print(f"{r['file']:15} reader={r['reader']:10} buffer={r['buffer']}")
