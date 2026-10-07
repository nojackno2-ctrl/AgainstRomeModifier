"""唯讀 Task B 探針：僅接受明確的副本路徑，不搜尋安裝目錄。"""
from pathlib import Path
import struct, sys, hashlib, collections

def pfil(data):
    if data[:4] != b'PFIL': return data
    size=struct.unpack_from('<I',data,16)[0]
    if size>50*1024*1024: raise ValueError('PFIL size')
    ring=bytearray(b' '*4078+b'\0'*18); r=4078; i=64; out=bytearray()
    while len(out)<size:
        flags=data[i]; i+=1
        for bit in range(8):
            if len(out)==size: break
            if flags & (1<<bit):
                seq=[data[i]]; i+=1
            else:
                a,b=data[i:i+2]; i+=2; pos=a|((b&240)<<4)
                seq=None
            for k in range(1 if seq is not None else (b&15)+3):
                if len(out)==size: break
                v=seq[0] if seq is not None else ring[(pos+k)&4095]
                out.append(v); ring[r]=v; r=(r+1)&4095
    return bytes(out)

def bmp(data):
    w,h=struct.unpack_from('<ii',data,18); off=struct.unpack_from('<I',data,10)[0]
    assert data[:2]==b'BM' and struct.unpack_from('<H',data,28)[0]==24
    stride=(w*3+3)&~3; rows=[]
    for y in range(abs(h)):
        z=y if h<0 else h-1-y
        rows.append([tuple(data[off+z*stride+x*3:off+z*stride+x*3+3][::-1]) for x in range(w)])
    return rows

def disasm(exe,spans):
    import pefile
    from capstone import Cs, CS_ARCH_X86, CS_MODE_32
    data=Path(exe).read_bytes(); pe=pefile.PE(data=data); cs=Cs(CS_ARCH_X86,CS_MODE_32); cs.skipdata=True
    for span in spans:
        start,end=[int(x,0) for x in span.split(':')]; off=pe.get_offset_from_rva(start-pe.OPTIONAL_HEADER.ImageBase)
        print('RANGE',span)
        for ins in cs.disasm(data[off:off+end-start],start): print(f'{ins.address:08X} {ins.mnemonic} {ins.op_str}')

if __name__=='__main__':
    if sys.argv[1]=='disasm': disasm(sys.argv[2],sys.argv[3:])
    else:
        for arg in sys.argv[1:]:
            p=Path(arg); data=p.read_bytes(); raw=pfil(data)
            print(p.name,'stored',len(data),'decoded',len(raw),'sha256',hashlib.sha256(data).hexdigest(),'head',raw[:64].hex())
            if p.suffix=='.bmp':
                rows=bmp(data); print('dimensions',len(rows[0]),len(rows),'channels',[(min(c),max(c),round(sum(c)/len(c),4)) for c in zip(*(px for row in rows for px in row))])
                if len(rows[0])==24:
                    for y,row in enumerate(rows): print('row',y,row)
            elif 'boden.ini' in p.name: print(raw.decode('cp1252'))
            elif 'gametime' in p.name: print('words',struct.unpack('<'+'I'*(len(raw)//4),raw))
            elif 'shadows' in p.name:
                print('header',struct.unpack('<8I',raw[:32])); print('histogram',collections.Counter(raw[32:]).most_common(20))
