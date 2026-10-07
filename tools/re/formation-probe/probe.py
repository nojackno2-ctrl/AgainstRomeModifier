"""Read-only formation evidence probe. Never searches an installed game directory.
Requires pefile/capstone on PYTHONPATH. Use explicit EXE/PFIL paths.
"""
import argparse, hashlib, struct
from pathlib import Path

def unpack(data):
    if data[:4] != b'PFIL':
        return data
    size = struct.unpack_from('<I', data, 16)[0]
    if size > 50 * 1024 * 1024:
        raise ValueError('PFIL size exceeds research limit')
    ring = bytearray(b' ' * 4078 + bytes(18))
    out = bytearray(); ip = 64; r = 4078
    while len(out) < size:
        flags = data[ip]; ip += 1
        for bit in range(8):
            if len(out) == size: break
            if flags & (1 << bit):
                values = [data[ip]]; ip += 1
                offset = None
            else:
                a,b = data[ip:ip+2]; ip += 2
                offset = a | ((b & 240) << 4)
                values = range((b & 15)+3)
            for k in values:
                value = k if offset is None else ring[(offset+k)&4095]
                out.append(value); ring[r]=value; r=(r+1)&4095
                if len(out)==size: break
    return bytes(out)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('path',type=Path)
    p.add_argument('--range',action='append',default=[])
    p.add_argument('--dump',default='0:256')
    args=p.parse_args(); data=args.path.read_bytes()
    print('SHA256',hashlib.sha256(data).hexdigest())
    if args.range:
        import pefile
        from capstone import Cs,CS_ARCH_X86,CS_MODE_32
        pe=pefile.PE(data=data); base=pe.OPTIONAL_HEADER.ImageBase
        cs=Cs(CS_ARCH_X86,CS_MODE_32)
        for span in args.range:
            start,end=[int(v,0) for v in span.split(':')]
            off=pe.get_offset_from_rva(start-base)
            for i in cs.disasm(data[off:off+end-start],start):
                print(f'{i.address:08x} {i.mnemonic:8s} {i.op_str}')
    else:
        raw=unpack(data); print('payload length',len(raw))
        start,end=[int(v,0) for v in args.dump.split(':')]
        for off in range(start,min(end,len(raw)),16):
            b=raw[off:off+16]; print(f'{off:06x}',b.hex(' '),repr(b))
if __name__=='__main__': main()
