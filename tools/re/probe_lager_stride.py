#!/usr/bin/env python3
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path('tools/re').resolve()))
from inventory_game_directory import pfil_prefix

p = Path(r"C:\Program Files (x86)\Against Rome\MAPS\ENDL_000\DATA\lager.dat")
data = p.read_bytes()
if data.startswith(b'PFIL@'):
    size = struct.unpack_from('<i', data, 16)[0]
    _, data = pfil_prefix(data, limit=size)

v, c, e1, e2 = struct.unpack_from('<IIII', data, 0)
print(f"Header: version={v}, count={c}, extra1={e1}, extra2={e2}")

# Main records: 3200 records. Stride in file?
# 1 byte (active) + 4 bytes (u32) + 4 bytes (float) + 2 bytes (u16) + e1 bytes + e2 * 2 bytes
# 1 + 4 + 4 + 2 + 6 + 10*2 = 37 bytes? Or is e1 = 12?
# Let's check remaining length: len(data) - 16 = 144000.
# If tail is 3200 * 2 = 6400 bytes, then main records = 144000 - 6400 = 137600 bytes.
# 137600 / 3200 = 43 bytes!
# If record is 43 bytes: 1 + 4 + 4 + 2 + e1(12?) + 20 = 43?
# Let's inspect the first active slot in lager.dat across multiple strides:

for stride_cand in (43, 45, 41):
    print(f"\n--- Testing stride {stride_cand} ---")
    active_count = 0
    for i in range(c):
        offset = 16 + i * stride_cand
        if offset + stride_cand <= len(data):
            act = data[offset]
            if act != 0:
                active_count += 1
    print(f"Active count with stride {stride_cand}: {active_count}")
