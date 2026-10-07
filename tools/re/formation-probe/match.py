"""Measure soldier ground anchors by masked NCC against decoded idle ALR frames.
Python standard library only; accepts explicit offline evidence paths.
"""
import json,struct,zlib,math,argparse
from pathlib import Path

def png(path):
    data=path.read_bytes(); pos=8; compressed=bytearray()
    while pos<len(data):
        n=struct.unpack_from('>I',data,pos)[0]; kind=data[pos+4:pos+8]; b=data[pos+8:pos+8+n]; pos+=12+n
        if kind==b'IHDR': w,h,bits,color,*_=struct.unpack('>IIBBBBB',b)
        if kind==b'IDAT': compressed.extend(b)
    assert bits==8 and color in (2,6)
    channels=3 if color==2 else 4; stride=w*channels
    raw=zlib.decompress(compressed); rows=[]; last=bytes(stride); pos=0
    for y in range(h):
        f=raw[pos]; row=bytearray(raw[pos+1:pos+1+stride]); pos+=stride+1
        for x in range(stride):
            a=row[x-channels] if x>=channels else 0; b=last[x]; c=last[x-channels] if x>=channels else 0
            if f==1: v=a
            elif f==2: v=b
            elif f==3: v=(a+b)//2
            elif f==4:
                p=a+b-c; pa,pb,pc=abs(p-a),abs(p-b),abs(p-c)
                v=a if pa<=pb and pa<=pc else b if pb<=pc else c
            else: v=0
            row[x]=(row[x]+v)&255
        rows.append([(row[x]+row[x+1]+row[x+2])/3 for x in range(0,stride,channels)]); last=row
    return rows

def main():
    p=argparse.ArgumentParser(); p.add_argument('screenshot',type=Path); p.add_argument('frames',type=Path); p.add_argument('--seeds',help='x,y;x,y'); args=p.parse_args()
    scene=png(args.screenshot); frames=json.loads((args.frames/'frames.json').read_text()); templates=[]
    for f in frames:
        rgba=(args.frames/f"soldier-{f['frame']}.rgba").read_bytes(); points=[]
        for y in range(f['Height']):
            for x in range(f['Width']):
                off=(y*f['Width']+x)*4
                if rgba[off+3]: points.append((x-f['AnchorX'],y-f['AnchorY'],sum(rgba[off:off+3])/3))
        mean=sum(v for x,y,v in points)/len(points); points=[(x,y,v-mean) for x,y,v in points]
        templates.append((points,sum(v*v for x,y,v in points)))
    seeds=[(384,272),(448,240),(480,320),(512,272),(512,176),(544,224),(576,176),(608,256),(640,208),(672,256)]
    if args.seeds: seeds=[tuple(map(int,v.split(','))) for v in args.seeds.split(';')]
    for seedx,seedy in seeds:
        best=(-1,None)
        for f,(pts,ss) in enumerate(templates):
            for ay in range(seedy-10,seedy+11):
                for ax in range(seedx-10,seedx+11):
                    s=q=cross=0
                    for dx,dy,t in pts:
                        v=scene[ay+dy][ax+dx]; s+=v; q+=v*v; cross+=v*t
                    variance=q-s*s/len(pts)
                    score=cross/math.sqrt(ss*variance) if variance>0 else 0
                    if score>best[0]: best=(score,(ax,ay,f))
        print(seedx,seedy,'->',best)
if __name__=='__main__': main()
