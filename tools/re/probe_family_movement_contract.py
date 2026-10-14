#!/usr/bin/env python3
"""Read-only secondary-position and signed 16.16 particle movement evidence."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import decode
from probe_pua import EXE_SHA256, require


def inspect(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    instructions = [i for a, n in ((0x4bd3a0, 265), (0x48dd90, 0x475), (0x4dbe60, 0xb00))
                    for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    index = {i.address: i for i in instructions}
    checks = [(0x4bd3cd, 'mov', 'ebx, dword ptr [eax*4 + 0xa14c08]'),
              (0x4bd3d4, 'sar', 'ebx, 0x10'),
              (0x4bd3f7, 'mov', 'dword ptr [esp + 0x18], 0x42000000'),
              (0x4bd3ff, 'mov', 'eax, dword ptr [0x16121e0]'),
              (0x4bd404, 'shl', 'eax, 6'), (0x4bd407, 'sub', 'eax, 0x20'),
              (0x4bd43a, 'mov', 'dword ptr [esp + 0x20], 0x42000000'),
              (0x4bd442, 'mov', 'eax, dword ptr [0x16121e4]'),
              (0x4bd447, 'shl', 'eax, 6'), (0x4bd44a, 'sub', 'eax, 0x20'),
              (0x4bd478, 'mov', 'dword ptr [eax*4 + 0xb18840], edx'),
              (0x4bd483, 'mov', 'dword ptr [eax*4 + 0xb18844], edx'),
              (0x4bd48e, 'mov', 'dword ptr [eax*4 + 0xb18848], edx'),
              (0x48dfc3, 'mov', 'dword ptr [edi + 0x5c], eax'),
              (0x48dfea, 'mov', 'dword ptr [edi + 0x15c], eax'),
              (0x48e014, 'mov', 'dword ptr [edi + 0x25c], eax'),
              (0x4dc6a7, 'mov', 'edx, dword ptr [edx + 0x1235830]'),
              (0x4dc6ad, 'sar', 'edx, 0x10'),
              (0x4dc6cb, 'mov', 'edx, dword ptr [edx + 0x1235730]'),
              (0x4dc6d1, 'sar', 'edx, 0x10'),
              (0x4dc6ef, 'mov', 'edx, dword ptr [edx + 0x1235630]'),
              (0x4dc6f5, 'sar', 'edx, 0x10'), (0x4dc70d, 'call', '0x49f660')]
    for address, mnemonic, operands in checks:
        i = index.get(address)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'instruction mismatch {address:#x}')
    require(struct.unpack('<f', read_va(exe, 0x5f6072, 4))[0] == 32., 'clamp constant mismatch')
    require(struct.unpack('<f', read_va(exe, 0x5f6812, 4))[0] == 65536., 'fixed scale mismatch')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'secondary_position_runtime_offset': 14, 'child_serialized_xyz_offsets': [1, 5, 9],
            'fixed_scale': 65536, 'secondary_xz_clamp': '32..mapDimension*64-32',
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def audit(root):
    maps, total = [], Counter()
    for path in sorted(root.glob('MAPS/*/DATA/particle.dat')):
        packed = path.read_bytes(); payload = decode(packed)
        require(len(payload) == 2322444 and struct.unpack_from('<3I', payload) == (1, 1024, 64), 'unsupported particle layout')
        counts = Counter(); minimum = [None] * 3; maximum = [None] * 3
        for slot in range(1024):
            record = 12 + slot * 2252
            if not struct.unpack_from('<H', payload, record)[0]: continue
            counts['active_emitters'] += 1
            for child in range(64):
                offset = record + 12 + child * 35
                if not payload[offset]: continue
                counts['saved_active_children'] += 1
                xyz = struct.unpack_from('<3i', payload, offset + 1)
                counts['fractional_children'] += any(value & 65535 for value in xyz)
                counts['negative_children'] += any(value < 0 for value in xyz)
                for axis, value in enumerate(xyz):
                    minimum[axis] = value if minimum[axis] is None else min(value, minimum[axis])
                    maximum[axis] = value if maximum[axis] is None else max(value, maximum[axis])
        require(path.read_bytes() == packed, 'source changed during audit')
        total.update(counts)
        maps.append({'map': path.parent.parent.name, 'sha256': hashlib.sha256(packed).hexdigest(),
                     'counts': dict(counts), 'fixed_xyz_min': minimum, 'fixed_xyz_max': maximum})
    require(maps, 'no original maps found')
    return {'maps': maps, 'totals': dict(total)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path); parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    args = parser.parse_args(); exe = args.exe.resolve(); root = args.game_root.resolve(); output = args.output.resolve()
    require(not output.exists() and not output.is_relative_to(root) and not output.is_relative_to(exe.parent), 'output must be new and outside sources')
    source = exe.read_bytes(); result = inspect(source); result.update(audit(root))
    require(exe.read_bytes() == source, 'EXE changed during audit')
    result['limitations'] = ['static instructions and saved state, not game execution',
        'all saved active children audited, not the runtime current-count loop',
        'float-to-fixed rounding, APT projection and full family movement remain unimplemented']
    with output.open('x', encoding='utf-8') as stream: json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({'assertions': result['instruction_assertions'], 'maps': len(result['maps']), 'totals': result['totals']}))


if __name__ == '__main__': main()
