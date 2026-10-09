"""Read-only, identifier-only probe of the two explicitly allowed TEMP copies.
Never traverses the installed game directory or writes to the evidence copies.
Output is derived evidence, not an asset dump.
"""
import sys, os, re, pathlib, hashlib, collections, struct
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from bcitool import pfil_decompress, Bci, disasm
root=pathlib.Path(os.environ['TEMP'])/'ArmGameCompare_20261007'
native=pathlib.Path(os.environ['TEMP'])/'ArmNativeAssets_20261007'
def plain(p):
 b=p.read_bytes(); return pfil_decompress(b) if b[:4]==b'PFIL' else b
a=plain(root/'SYSTEM/CLAK/cl_scint.ini').decode('cp1251')
sections=dict((k,v) for k,v in re.findall(r'(?ms)^\[([^\]]+)\]\s*\n(.*?)(?=^\[|\Z)',a.replace('\r','')))
print('ALIASES\n'+ '\n'.join(l for l in sections['ObjDefName'].splitlines() if l.startswith('ALL_') or re.search('MIN|GOL|LAG',l)))
print('ANIMAL SCRIPTS\n'+ '\n'.join(l for l in sections['ObjDefScript'].splitlines() if l.startswith('ALL_')))
names=[]
for l in (native/'objdef.txt').read_text().splitlines():
 f=[x.strip() for x in l.split(',')]
 if len(f)>52 and f[0].isdigit(): names.append((int(f[0]),f[52]))
print('ANIMALS',[(i,n) for i,n in names if n.startswith('FigTie')])
print('RESOURCES/CAMP CANDIDATES',[(i,n) for i,n in names if re.search(r'Baum|Tann|Eich|Bir|Stein|Fels|Feld|Gold|Mine|Zelt|Lager|Hoehl|Nest|Wolf|Bae|Rau|Eber',n,re.I)])
for map in ['ENDL_000','ENDL_005']:
 print('\nMAP',map)
 stats=collections.Counter()
 for p in sorted((root/map).glob('*.sdl')):
  t=plain(p).decode('cp1251').replace('\r','')
  for idx,body in re.findall(r'(?ms)^\[object(\d+)\]\s*\n(.*?)(?=^\[|\Z)',t):
   fields=dict(re.findall(r'(?m)^[ \t]*(\w+)[ \t]*=[ \t]*([^\n]*)',body))
   n=fields.get('namedef','') or dict(names).get(int(fields.get('def','-1')), '<unknown>'); team=fields.get('team','<absent>'); onload=fields.get('onload','<absent>')
   stats[(n,team,onload)]+=1
   if n.startswith('FigTie') or re.search('Mine|Gold|Feld|Zelt',n): print(p.name,idx,fields)
 print('SDL GROUPS',sorted(stats.items()))
 print('SDL TOTAL',sum(stats.values()),'TEAM/ONLOAD', dict(collections.Counter({(t,o):sum(v for (n,tt,oo),v in stats.items() if tt==t and oo==o) for n,t,o in stats})))
 data=plain(root/map/'DATA/objects.dat')
 count=struct.unpack_from('<I',data,4)[0]; active=collections.Counter()
 for slot in range(count):
  off=16+slot*79
  if data[off]:
   type_id=struct.unpack_from('<H',data,off+75)[0]
   team=struct.unpack_from('<H',data,off+1)[0]
   active[(dict(names).get(type_id,'<unknown>'),team)]+=1
 print('DATA TOTAL',sum(active.values()),'TEAMS',dict(collections.Counter({t:sum(v for (n,tt),v in active.items() if tt==t) for n,t in active})))
 print('DATA TANNEN',sum(v for (n,t),v in active.items() if re.fullmatch(r'LanGerNad\d\d_Tanne_(gross|mittel|klein)',n)))
 print('DATA ANIMALS',[(k,v) for k,v in active.items() if k[0].startswith('FigTie')])
 print('DATA RESOURCES',[(k,v) for k,v in active.items() if k[0].startswith('LanGerNad') or re.search('Laubbaum|Weizen|Mine|Goldschmiede',k[0])])
 b=Bci((root/map/'SCRIPT/ak_level.bci').read_bytes())
 print('BCI names', [n for n in b.names if re.search('ALL_|Tie|spawn|creat|ress|reg|wood|food|gold|mine|tree|wait|onload|sdl',n,re.I)])
 rows=disasm(b,0,len(b.words()))
 for j,(off,op,txt) in enumerate(rows):
  if op==128 and re.search('s_createObj>|s_createUnitAndMems>|s_setVillageTemplate>',txt):
   print('CALL CONTEXT', '\n'.join(f'{o:06x} {t}' for o,_,t in rows[max(0,j-12):j+4]))
for p in [root/'SYSTEM/CLAK/cl_scint.ini',native/'objdef.txt',root/'ENDL_000/SCRIPT/ak_level.bci',root/'ENDL_005/SCRIPT/ak_level.bci']:
 print('SHA256',p.relative_to(root) if p.is_relative_to(root) else p.name,hashlib.sha256(p.read_bytes()).hexdigest())

# Recheck publishable identifier constants against the allowed copies, without shipping assets.
repo=pathlib.Path(__file__).resolve().parents[2]
catalog=(repo/'src.MapEditor.Modules/WildLair/ResourceRegenerationPlanner.cs').read_text(encoding='utf-8-sig')
actual_aliases=dict(re.findall(r'(?m)^([A-Z0-9_]+)[ \t]*=[ \t]*(\S+)',sections['ObjDefName']))
rows=re.findall(r'new\("([^"]+)", NativeResourceKind\.(\w+), (\d+)(?:, "([^"]+)")?\)',catalog)
assert len(rows)==64, "Unexpected resource catalog size"
for name,kind,index,alias in rows:
 assert dict(names).get(int(index))==name, (index,name)
 if alias: assert actual_aliases.get(alias)==name, (alias,name)
print('RESOURCE CATALOG VERIFIED',len(rows),'definitions',sum(bool(r[3]) for r in rows),'aliases')

fixture=(repo/'tests/AgainstRomeMapEditor.Modules.Tests/NeutralLairAndRegenTests.cs').read_text(encoding='utf-8-sig')
fixture=fixture.split('ScriptObjectAliases.Parse("""',1)[1].split('""");',1)[0].split('[ObjDefScript]')[0]
fixture_aliases=dict(re.findall(r'(?m)^[ \t]*([A-Z0-9_]+)[ \t]*=[ \t]*(\S+)',fixture))
for alias,name in fixture_aliases.items():
 assert actual_aliases.get(alias)==name,(alias,name)
print('TEST FIXTURE VERIFIED',len(fixture_aliases),'real alias mappings')
