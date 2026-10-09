#!/usr/bin/env python3
"""Inspect active records in formatio, lager, biglager across real maps."""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path('tools/re').resolve()))
from inventory_game_directory import pfil_prefix

GAME_ROOT = Path(r"C:\Program Files (x86)\Against Rome")
MAPS_DIR = GAME_ROOT / "MAPS"

def decode_file(p):
    data = p.read_bytes()
    if data.startswith(b'PFIL@'):
        size = struct.unpack_from('<i', data, 16)[0]
        _, raw = pfil_prefix(data, limit=size)
        return raw
    return data

def inspect_formatio(decoded):
    version, count = struct.unpack_from('<II', decoded, 0)
    stride = 25
    active_count = 0
    active_samples = []
    for i in range(count):
        rec = decoded[8 + i * stride : 8 + (i + 1) * stride]
        act = rec[0]
        if act != 0:
            active_count += 1
            if len(active_samples) < 5:
                # unpack fields
                u16_2 = struct.unpack_from('<H', rec, 2)[0]
                u32_4 = struct.unpack_from('<I', rec, 4)[0]
                u32_8 = struct.unpack_from('<I', rec, 8)[0]
                u32_12 = struct.unpack_from('<I', rec, 12)[0]
                tail = rec[16:].hex()
                active_samples.append({
                    'slot': i, 'act': act, 'u16_2': u16_2,
                    'u32_4': u32_4, 'u32_8': u32_8, 'u32_12': u32_12,
                    'tail_hex': tail
                })
    return {'version': version, 'count': count, 'active': active_count, 'samples': active_samples}

def inspect_lager(decoded):
    version, count, extra1, extra2 = struct.unpack_from('<IIII', decoded, 0)
    stride = 45
    active_count = 0
    active_samples = []
    for i in range(count):
        rec = decoded[16 + i * stride : 16 + (i + 1) * stride]
        act = rec[0]
        if act != 0:
            active_count += 1
            if len(active_samples) < 5:
                active_samples.append({
                    'slot': i, 'act': act, 'hex': rec.hex()
                })
    return {'version': version, 'count': count, 'extra': [extra1, extra2], 'active': active_count, 'samples': active_samples}

def inspect_biglager(decoded):
    version, count, subcount, extra = struct.unpack_from('<IIII', decoded, 0)
    # let's see non-zero bytes
    return {'version': version, 'count': count, 'subcount': subcount, 'extra': extra, 'total_len': len(decoded)}

print("=== ENDL_000 ===")
fmt = decode_file(MAPS_DIR / "ENDL_000/DATA/formatio.dat")
print("formatio:", inspect_formatio(fmt))
lag = decode_file(MAPS_DIR / "ENDL_000/DATA/lager.dat")
print("lager:", inspect_lager(lag))
blag = decode_file(MAPS_DIR / "ENDL_000/DATA/biglager.dat")
print("biglager:", inspect_biglager(blag))

print("=== MP_001 ===")
fmt = decode_file(MAPS_DIR / "MP_001/DATA/formatio.dat")
print("formatio:", inspect_formatio(fmt))
lag = decode_file(MAPS_DIR / "MP_001/DATA/lager.dat")
print("lager:", inspect_lager(lag))

print("=== C1_M01 ===")
fmt = decode_file(MAPS_DIR / "C1_M01/DATA/formatio.dat")
print("formatio:", inspect_formatio(fmt))
lag = decode_file(MAPS_DIR / "C1_M01/DATA/lager.dat")
print("lager:", inspect_lager(lag))
