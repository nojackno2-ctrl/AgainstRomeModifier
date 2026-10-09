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

print("Total decoded bytes:", len(data))
v, c = struct.unpack_from('<II', data, 0)
print(f"Header: version={v}, count={c}")
# If there are 14000 records of 15 bytes each: 14000 * 15 = 210000 bytes
# What is left? 350008 - 8 - 210000 = 140000 bytes = 14000 * 10 bytes!
# Or is it 14000 * 25 bytes contiguous records?
# Let's check record 0, 1, 2, 3 in contiguous 25-byte layout vs split layout:

for i in range(10):
    chunk25 = data[8 + i*25 : 8 + (i+1)*25]
    print(f"Slot {i} (stride 25): {chunk25.hex()}")
