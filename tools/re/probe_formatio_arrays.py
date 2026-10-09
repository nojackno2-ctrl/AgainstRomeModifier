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
assert len(tail) == 140000

arr1 = tail[0:56000]          # 14000 * 4 bytes
arr2 = tail[56000:112000]     # 14000 * 4 bytes
arr3 = tail[112000:140000]    # 14000 * 2 bytes

print("Array 1 sample (uint32):", [struct.unpack_from('<I', arr1, i*4)[0] for i in range(10)])
print("Array 2 sample (uint32):", [struct.unpack_from('<I', arr2, i*4)[0] for i in range(10)])
print("Array 3 sample (uint16):", [struct.unpack_from('<H', arr3, i*2)[0] for i in range(10)])
