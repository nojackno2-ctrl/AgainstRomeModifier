"""只讀指定副本；標準庫 PNG 解碼與 APT 像素定量，不寫原素材。"""
from pathlib import Path
import base64, json, math, struct, subprocess, sys, zlib
from investigate import bmp, pfil

def png(path):
    data=Path(path).read_bytes(); assert data[:8]==b'\x89PNG\r\n\x1a\n'
    i=8; packed=bytearray()
    while i<len(data):
        n=struct.unpack_from('>I',data,i)[0]; kind=data[i+4:i+8]; payload=data[i+8:i+8+n]; i+=12+n
        if kind==b'IHDR': w,h,depth,color,_,_,interlace=struct.unpack('>IIBBBBB',payload)
        if kind==b'IDAT': packed.extend(payload)
    assert depth==8 and color in (2,6) and interlace==0
    channels=3 if color==2 else 4; stride=w*channels; raw=zlib.decompress(packed); rows=[]; previous=bytearray(stride); i=0
    for y in range(h):
        f=raw[i]; row=bytearray(raw[i+1:i+1+stride]); i+=stride+1
        for x in range(stride):
            a=row[x-channels] if x>=channels else 0; b=previous[x]; c=previous[x-channels] if x>=channels else 0
            if f==1: v=a
            elif f==2: v=b
            elif f==3: v=(a+b)//2
            elif f==4:
                p=a+b-c; dif=[abs(p-a),abs(p-b),abs(p-c)]; v=[a,b,c][dif.index(min(dif))]
            elif f==0: v=0
            else: raise ValueError('PNG filter')
            row[x]=(row[x]+v)&255
        rows.append([tuple(row[x:x+3]) for x in range(0,stride,channels)]); previous=row
    return rows

def ambient(table,minute):
    h,m=divmod(minute,60); a=table[0][h]; b=table[0][(h+1)%24]
    return tuple(((x*((60-m)*65536//60)+y*(m*65536//60))>>16)/256 for x,y in zip(a,b))

def shadow(raw,second,x,z):
    t=second*128//86400; w=(second%675)*256//675
    def oc(s,xx,zz): return 255 if raw[32+s*8192+zz*32+(xx>>3)]&(1<<(xx&7)) else 0
    return (oc(t,x,z)*(256-w)+oc((t+1)%128,x,z)*w)>>8

def main():
    root=Path(sys.argv[1]); screen=png(root/'game-house.png')
    result=subprocess.run(['dotnet','run','--project','tools/re/lighting-probe','-c','Release','-p:UseAppHost=false','--',str(root/'apt.dat')],capture_output=True,text=True,check=True)
    sprites=json.loads(next(line for line in result.stdout.splitlines() if line.startswith('[{')))
    gains=[]
    for s,anchor in zip(sprites,[(512,340),(896,532)]):
        w,h=s['Width'],s['Height']; px=s['pixels']; tx=anchor[0]-s['AnchorX']; ty=anchor[1]-s['AnchorY']; pairs=[]
        for y in range(3,h-3):
            for x in range(3,w-3):
                sx,sy=tx+x,ty+y
                if not (0<=sy<min(650,len(screen)) and 0<=sx<len(screen[0])): continue
                # 僅選 sprite 內部不透明像素，排除輪廓混色及 HUD。
                if any((px[(y+dy)*w+x+dx]>>24)!=255 for dx,dy in [(0,0),(-2,0),(2,0),(0,-2),(0,2)]): continue
                c=px[y*w+x]; rgb=((c>>16)&255,(c>>8)&255,c&255)
                if min(rgb)<30 or max(rgb)>240: continue
                pairs.append((rgb,screen[sy][sx],(sx,sy)))
        # 兩次 trim 大殘差，降低其他 sprite 覆蓋和主屋火光動畫影響；完整／trim 都報告。
        def fit(p):
            g=[sum(a[c]*b[c] for a,b,_ in p)/sum(a[c]**2 for a,b,_ in p) for c in range(3)]
            rmse=math.sqrt(sum((a[c]*g[c]-b[c])**2 for a,b,_ in p for c in range(3))/(3*len(p)))
            return g,rmse
        original=fit(pairs); full_count=len(pairs)
        for lo,hi in [(0,400),(400,650)]:
            region=[p for p in pairs if lo<=p[2][1]<hi]
            if region: print('REGION',s['name'],'screenY',lo,hi,'n',len(region),'fit',fit(region))
        for _ in range(2):
            g,_=fit(pairs); pairs.sort(key=lambda p:sum((p[0][c]*g[c]-p[1][c])**2 for c in range(3))); pairs=pairs[:int(len(pairs)*.9)]
        g,rmse=fit(pairs); gains.append(g)
        print('SPRITE',s['name'],'frame',s['frame'],'origin',[tx,ty],'full',full_count,original,'trimmed',len(pairs),'gain',g,'pixelRMSE',rmse)
        for a,b,p in pairs[::max(1,len(pairs)//3)][:3]: print('PAIR',p,'asset',a,'screen',b)
    table=bmp((root/'ENDL_005/daynight.bmp').read_bytes()); raw=pfil((root/'ENDL_005/shadows.dat').read_bytes())
    for name,g in zip(['gerhau00','gerwoh00'],gains):
        best=min((sum((a-b)**2 for a,b in zip(g,ambient(table,m))),m,ambient(table,m)) for m in range(1440))
        print('ROW0_ONLY',name,'bestMinute',best[1],'predicted',best[2],'gainRMSE',math.sqrt(best[0]/3))
    def global_scalar(m):
        base=ambient(table,m); scalars=[min(1,sum(a*b for a,b in zip(g,base))/sum(a*a for a in base)) for g in gains]
        err=sum((g[c]-base[c]*s)**2 for g,s in zip(gains,scalars) for c in range(3))
        return err,m,base,scalars
    best=min(global_scalar(m) for m in range(1440)); print('COMMON_ROW0_INDEPENDENT_SCALARS',best,'gainRMSE',math.sqrt(best[0]/6))
    game_time=pfil((root/'ENDL_005/DATA/gametime.dat').read_bytes()); print('SAVED_TIME_WORDS',struct.unpack('<8I',game_time))
    for minute in [1075,295,best[1],720,1140]: print('SHADOW_BITS',minute,[(x,shadow(raw,minute*60,x,158)) for x in [166,178]])
    for xy in [(400,440),(480,500),(600,500)]:
        sx,sy=xy; dx=sx-512; dy=sy-340; wx=10624+dx+2*dy; wz=10112-dx+2*dy
        print('GROUND',xy,'screen',screen[sy][sx],'worldIfFlat',(wx,wz),'vertex',bmp((root/'ENDL_005/vertex.bmp').read_bytes())[wz//64][wx//64])
    for name in ['game-units.png','game-units-east.png']:
        im=png(root/name); print('DUSK_SCREEN',name,'size',(len(im[0]),len(im)),'fixedScreenSamples',[(xy,im[xy[1]][xy[0]]) for xy in [(400,440),(480,500),(600,500)]])
    payload=[]
    for folder in ['ENDL_000','ENDL_005']:
        for name in ['shadows.dat','daynight.bmp','boden.bmp','vertex.bmp','emboss.bmp','smooth.bmp']:
            payload.append(dict(name=folder+'/'+name,bytes=base64.b64encode(pfil((root/folder/name).read_bytes())).decode()))
    validation=subprocess.run(['dotnet','run','--project','tools/re/lighting-probe','-c','Release','-p:UseAppHost=false','--no-build','--','validate'],input=json.dumps(payload),capture_output=True,text=True,check=True)
    print(validation.stdout,end='')

if __name__=='__main__': main()
