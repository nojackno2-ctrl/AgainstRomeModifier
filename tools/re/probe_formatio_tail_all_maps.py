#!/usr/bin/env python3
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path('tools/re').resolve()))
from inventory_game_directory import pfil_prefix

p = Path(r"C:\Program Files (x86)\Against Rome\MAPS\ENDL_000\DATA\formatio.dat")
data = p.read_bytes()
if data.startswith(b'PFIL@'):
    size = struct.unpack_from('<i', data, 16)[0]
    _, data = pfil_prefix(data, limit=size)

tail = data[210008:]
print("Tail length:", len(tail))
# Check if there are non-zero bytes in tail across all 74 maps
maps_dir = Path(r"C:\Program Files (x86)\Against Rome\MAPS")
nonzeros = {}
for m in maps_dir.iterdir():
    f = m / "DATA/formatio.dat"
    if f.is_file():
        d = f.read_bytes()
        if d.startswith(b'PFIL@'):
            sz = struct.unpack_from('<i', d, 16)[0]
            _, d = pfil_prefix(d, limit=sz)
        t = d[210008:]
        nz = sum(1 for b in t if b != 0)
        if nz > 0:
            nonzeros[m.name] = nz

print("Maps with non-zero bytes in formatio tail:", nonzeros)
