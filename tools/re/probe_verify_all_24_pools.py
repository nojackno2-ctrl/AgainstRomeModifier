#!/usr/bin/env python3
"""Full structural validation of all 24 data pools across all 74 native maps."""
import hashlib
import json
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path('tools/re').resolve()))
from inventory_game_directory import pfil_prefix

GAME_ROOT = Path(r"C:\Program Files (x86)\Against Rome")
MAPS_DIR = GAME_ROOT / "MAPS"

# Exact decoded sizes for the 24 pools across standard maps
EXPECTED_SIZES = {
    'light.dat': 57352,      # 8 + 1024 * 56
    'gametime.dat': 32,      # 32 bytes fixed header & environment state
    'rain.dat': 262164,     # 20 + 262144 (512x512 grid or 16384 * 16)
    'hagel.dat': 229396,    # 20 + 229376
    'snow.dat': 65556,      # 20 + 65536
    'flash.dat': 23312,     # 16 + 100 * 232 or 16 subslots
    'objects.dat': 1694016, # 16 + 14000 * 121
    'position.dat': 561008, # 8 + 33000 * 17
    'anim.dat': 350008,     # 8 + 14000 * 21 + 14000*2 + 14000*2
    'gfxtype.dat': 238008,  # 8 + 14000 * 15 + 14000*2
    'action.dat': 350012,   # 12 + 14000 * 25
    'objdata.dat': 1722008, # 8 + 14000 * 123
    'hirarchy.dat': 336012, # 12 + 3200 * 103 + 3200*2
    'formatio.dat': 350008, # 8 + 14000 * 15 + 14000*4 + 14000*4 + 14000*2
    'lager.dat': 144016,    # 16 + 3200 * 45
    'engine.dat': 94,       # 94 bytes fixed state
    'fow.dat': 66053,       # 5 + 256*256 fog grid
    'fowreq.dat': 4616,     # 8 + 256 * 18
    'way.dat': 791052,      # 12 + 768 waypaths
    'particle.dat': 2322444,# 12 + 1024 * 2268
    'explos.dat': 54008,    # 8 + 1000 * 54
    'hitex.dat': 40008,     # 8 + 1000 * 40
    'stat.dat': 6540,       # 12 + 8 teams * 816
    'biglager.dat': 51244,  # 16 + 32 * 1600.875
}

def decode_file(data):
    if data.startswith(b'PFIL@'):
        size = struct.unpack_from('<i', data, 16)[0]
        _, raw = pfil_prefix(data, limit=size)
        return raw
    return data

def run_validation():
    total_maps = 0
    total_files_checked = 0
    size_matches = 0
    version_matches = 0
    anomalies = []

    for map_dir in sorted(MAPS_DIR.iterdir()):
        if not map_dir.is_dir():
            continue
        data_dir = map_dir / "DATA"
        if not data_dir.is_dir():
            continue
        total_maps += 1
        for fn, expected_sz in EXPECTED_SIZES.items():
            fpath = data_dir / fn
            if not fpath.is_file():
                anomalies.append(f"Missing {fn} in {map_dir.name}")
                continue
            total_files_checked += 1
            raw = fpath.read_bytes()
            decoded = decode_file(raw)
            if len(decoded) == expected_sz:
                size_matches += 1
            else:
                anomalies.append(f"{map_dir.name}/{fn} size mismatch: got {len(decoded)}, expected {expected_sz}")
            
            # Check version
            if len(decoded) >= 4:
                ver = struct.unpack_from('<I', decoded, 0)[0]
                if ver == 1:
                    version_matches += 1
                else:
                    anomalies.append(f"{map_dir.name}/{fn} unsupported version: {ver}")

    report = {
        'total_maps': total_maps,
        'total_files_checked': total_files_checked,
        'expected_total_files': total_maps * 24,
        'size_matches': size_matches,
        'version_matches': version_matches,
        'anomaly_count': len(anomalies),
        'anomalies_sample': anomalies[:10]
    }
    return report

if __name__ == '__main__':
    rep = run_validation()
    print(json.dumps(rep, indent=2))
    # Write report to re_workspace
    out_file = Path('re_workspace/all-24-pools-validation-report.json')
    out_file.write_text(json.dumps(rep, indent=2), encoding='utf8')
