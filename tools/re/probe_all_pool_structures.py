#!/usr/bin/env python3
"""Read-only structural probe of all 24 native data pools across maps."""
import hashlib
import json
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path('tools/re').resolve()))
from inventory_game_directory import pfil_prefix

GAME_ROOT = Path(r"C:\Program Files (x86)\Against Rome")
MAPS_DIR = GAME_ROOT / "MAPS"

FILES_24 = [
    'light.dat', 'gametime.dat', 'rain.dat', 'hagel.dat', 'snow.dat', 'flash.dat',
    'objects.dat', 'position.dat', 'anim.dat', 'gfxtype.dat', 'action.dat', 'objdata.dat',
    'hirarchy.dat', 'formatio.dat', 'lager.dat', 'engine.dat', 'fow.dat', 'fowreq.dat',
    'way.dat', 'particle.dat', 'explos.dat', 'hitex.dat', 'stat.dat', 'biglager.dat'
]

def decode_file(data):
    if data.startswith(b'PFIL@'):
        size = struct.unpack_from('<i', data, 16)[0]
        _, raw = pfil_prefix(data, limit=size)
        return True, raw
    return False, data

def analyze_sample_map(map_name):
    map_dir = MAPS_DIR / map_name / "DATA"
    if not map_dir.is_dir():
        return None
    summary = {}
    for fn in FILES_24:
        p = map_dir / fn
        if not p.is_file():
            summary[fn] = {'exists': False}
            continue
        raw_bytes = p.read_bytes()
        is_pfil, decoded = decode_file(raw_bytes)
        
        info = {
            'exists': True,
            'is_pfil': is_pfil,
            'raw_len': len(raw_bytes),
            'decoded_len': len(decoded),
        }
        if len(decoded) >= 8:
            v1, v2 = struct.unpack_from('<II', decoded, 0)
            info['header_u32_0'] = v1
            info['header_u32_1'] = v2
        if len(decoded) >= 12:
            info['header_u32_2'] = struct.unpack_from('<I', decoded, 8)[0]
        if len(decoded) >= 16:
            info['header_u32_3'] = struct.unpack_from('<I', decoded, 12)[0]
        summary[fn] = info
    return summary

if __name__ == '__main__':
    maps_to_test = ['ENDL_000', 'ENDL_005', 'MP_001', 'C1_M01']
    res = {}
    for m in maps_to_test:
        r = analyze_sample_map(m)
        if r:
            res[m] = r
    print(json.dumps(res, indent=2))
