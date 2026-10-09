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

v, c = struct.unpack_from('<II', data, 0)
print(f"Header: version={v}, count={c}")

# Let's check 15-byte records
stride = 15
print("Checking first 5 records with stride 15:")
for i in range(5):
    chunk = data[8 + i*stride : 8 + (i+1)*stride]
    act = chunk[0]
    u16_val = struct.unpack_from('<H', chunk, 1)[0]
    flt_val = struct.unpack_from('<f', chunk, 3)[0]
    u32_val1 = struct.unpack_from('<I', chunk, 7)[0]
    u32_val2 = struct.unpack_from('<I', chunk, 11)[0]
    print(f"Slot {i}: act={act}, u16={hex(u16_val)}, flt={flt_val}, u32_1={u32_val1}, u32_2={u32_val2}")

# If 14000 * 15 = 210000 bytes:
offset_after = 8 + 14000 * 15
print(f"Offset after primary records: {offset_after} (remaining: {len(data) - offset_after} bytes)")

# What is in remaining 140,000 bytes?
# 140,000 / 14,000 = 10 bytes per slot!
# Let's inspect the remaining 140,000 bytes:
rem = data[offset_after:]
print("First 64 bytes of remaining data:", rem[:64].hex())
