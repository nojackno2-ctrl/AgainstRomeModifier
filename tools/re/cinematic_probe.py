#!/usr/bin/env python3
"""Read-only EXE registration/VM and authorized BCI evidence probe.

Dependencies: pefile, capstone. See docs/reverse-engineering/cinematic-camera.md.
Output is a report on stdout; the executable is read as bytes, never launched.
"""
import sys, struct, pathlib, hashlib, re
import pefile, capstone
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]))
import bcitool
import argparse
parser=argparse.ArgumentParser(description='Read-only cinematic native/BCI evidence; never runs the EXE.')
parser.add_argument('--exe', type=pathlib.Path, required=True)
parser.add_argument('--samples', type=pathlib.Path, required=True)
args=parser.parse_args()
exe=args.exe
data=exe.read_bytes(); pe=pefile.PE(data=data); base=pe.OPTIONAL_HEADER.ImageBase
md=capstone.Cs(capstone.CS_ARCH_X86,capstone.CS_MODE_32)
def off(va): return pe.get_offset_from_rva(va-base)
def va(o): return base+pe.get_rva_from_offset(o)
def string(a):
    o=off(a); return data[o:data.index(b'\0',o)].decode('latin1')
def dump(a,n):
    print(f'\nDIS {a:#x}')
    for ins in md.disasm(data[off(a):off(a)+n],a): print(f'{ins.address:08x} {ins.bytes.hex():24s} {ins.mnemonic} {ins.op_str}')
print('EXE SHA256',hashlib.sha256(data).hexdigest())
regs=[]
for m in re.finditer(rb'\x6a\x00\x68(.{4})\x68(.{4})\x68(.{4})(?:\x09\xc3)?\xe8(.{4})',data,re.S):
    try:
        sig,func,name=struct.unpack('<III',m.group(1)+m.group(2)+m.group(3)); name=string(name); sig=string(sig)
        target=va(m.start())+len(m.group())+struct.unpack('<i',m.group(4))[0]
        if target==0x5b1410: regs.append((name,sig,func,va(m.start()),m.group().hex()))
    except (ValueError,IndexError,UnicodeError,pefile.PEFormatError): pass
print('REGISTERED',len(regs))
print('ZOOM_UPPER_BOUND',struct.unpack_from('<f',data,off(0x5f4b3d))[0])
for name in ['s_setCamera','s_disableGUI']:
    print('EXACT_NAME_PRESENT', name, (name.encode()+b'\0') in data)
for r in regs:
    if re.search('camera|lgc.*(Engine|Fade)|showTextBox|playVoice|disableGUI|conWaitTime|conMoveTo',r[0],re.I): print('REG',r[0],r[1],hex(r[2]),hex(r[3]),r[4])
for a,n in [(0x54c400,81),(0x54c010,39),(0x4989d0,28),(0x54c620,28),(0x54c1f0,19),(0x498a30,80),(0x54c890,24),(0x521f10,37),(0x5b1700,390),(0x5b6590,320)]: dump(a,n)
for op in [16,17,64,65,66,67,68,69,70,73,76,128,131]:
    handler=struct.unpack_from('<I',data,off(0x5b199c)+(op-1)*4)[0]
    print('HANDLER',op,hex(handler));
    if op in [67,128]: dump(handler,340)
root=args.samples
for p in sorted(root.rglob('*.bci')):
    raw=p.read_bytes(); b=bcitool.Bci(raw)
    print('\nBCI',p.relative_to(root),'SHA256',hashlib.sha256(raw).hexdigest(),'codeSize',b.codeSize,'constants',b.symCount)
    print('RELEVANT_CONSTANTS',[(i,s) for i,s in enumerate(b.names) if re.search('camera|cutscene|lgc|fade|message|showText',s,re.I)])
    if p.parent.parent.name in ['ENDL_000','ENDL_005'] and len(b.code)>=0x1ba14:
        print('MESSAGE_STATEMENT_WORDS', list(struct.unpack('<8i', b.code[0x1b9f4:0x1ba14])))
    print('DOUBLE_IMMEDIATES',[(hex(o),t) for o,op,t in bcitool.disasm(b,0,len(b.code)//4) if op==67][:8])
