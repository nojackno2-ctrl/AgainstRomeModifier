#!/usr/bin/env python3
"""Read-only afstr parser/create proof and definition-to-position audit."""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import struct

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import decode, COLUMN_WIDTHS
from probe_pua import EXE_SHA256, require


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    table = struct.unpack('<209I', read_va(exe, 0x4b1130, 209 * 4))
    require(table[60] == 0x4b21bc, 'unexpected afstr parser case')
    require(table[19] == 0x4b18c1 and table[21] == 0x4b1930, 'unexpected vitals parser cases')
    require(table[4] == 0x4b157a and table[152] == 0x4b35bb, 'unexpected scalar parser cases')
    regions = [(0x4aa780, 1703), (0x4b1480, 0x29), (0x4b21bc, 0x38),
               (0x480850, 0x1000), (0x47e850, 890), (0x4b4200, 95), (0x4afe10, 864), (0x567d50, 0xd2), (0x42a230, 0xfa), (0x4b18c1, 0xa6), (0x4a9740, 0x609), (0x4b157a, 0x37), (0x4b35bb, 0x37), (0x4bb5d0, 0xd7)]
    instructions = [i for address, size in regions
                    for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, address, size), address)]
    by_address = {i.address: i for i in instructions}
    checks = [(0x4b149a, 'cmp', 'edx, 0xd0'),
              (0x4b14a2, 'jmp', 'dword ptr [edx*4 + 0x4b1130]'),
              (0x4b21cb, 'call', '0x567320'),
              (0x4b21d0, 'imul', 'edx, dword ptr [0x1fc933c], 0x2a4'),
              (0x4b21e4, 'mov', 'word ptr [edx + 0xc64922], ax'),
              (0x4aa917, 'call', '0x4afd80'),
              (0x4aa921, 'je', '0x4aa96c'),
              (0x4aa923, 'imul', 'ebx, ebx, 0x2a4'),
              (0x4aa929, 'cmp', 'word ptr [ebx + 0xc64922], 0'),
              (0x4aa931, 'je', '0x4aa96c'),
              (0x4aa933, 'call', '0x4ab4a0'),
              (0x4aa93e, 'mov', 'eax, dword ptr [esp + 0x34]'),
              (0x4aa942, 'push', 'eax'),
              (0x4aa944, 'call', '0x4aae40'),
              (0x47eaa6, 'push', '0x5f29ad'), (0x47eac2, 'push', '0x770de8'),
              (0x47eac9, 'call', '0x414cf0'), (0x4b4206, 'push', '0x770de8'),
              (0x4b420b, 'push', '0x5f5eda'), (0x4b4219, 'call', '0x572f50'),
              (0x4b4232, 'push', '0x4b1480'), (0x4b4246, 'push', '0x5f5ee1'),
              (0x4b424c, 'call', '0x42a230'), (0x480c91, 'call', '0x4afdc0'),
              (0x480d1b, 'push', '0x5f3187'), (0x480d20, 'call', '0x4b4200'),
              (0x480f42, 'xor', 'ebx, ebx'), (0x480f6b, 'push', '0x5f3359'),
              (0x480f75, 'call', '0x5c5de0'), (0x480f80, 'call', '0x4b4200'),
              (0x4810e8, 'inc', 'ebx'), (0x4810f1, 'cmp', 'ebx, 0xa'),
              (0x4810f4, 'jl', '0x480f4c'), (0x4afec2, 'mov', 'dword ptr [ebx + 0x64], 0'),
              (0x4aff39, 'mov', 'byte ptr [ebx], 0'),
              (0x567df7, 'je', '0x567e05'), (0x567dfb, 'je', '0x567e10'),
              (0x567dff, 'jmp', '0x567de6'), (0x567e1c, 'mov', 'edi, eax'),
              (0x42a2ae, 'call', '0x567d50'), (0x42a2bc, 'jle', '0x42a2fd'),
              (0x42a2df, 'call', 'ebp'), (0x42a2f5, 'inc', 'ebx'),
              (0x4b18c9, 'mov', 'edx, 0x5f5c0f'), (0x4b18e9, 'mov', 'dword ptr [edx + 0xc648e8], eax'),
              (0x4b1938, 'mov', 'edx, 0x5f5c17'), (0x4b1958, 'mov', 'dword ptr [edx + 0xc648f0], eax'),
              (0x4a99d5, 'fild', 'dword ptr [eax + 0xc648e8]'), (0x4a99db, 'fstp', 'dword ptr [esp + 4]'),
              (0x4a99df, 'mov', 'esi, 0x3f800000'), (0x4a99e5, 'mov', 'dword ptr [esp + 0xc], esi'),
              (0x4a99ff, 'fild', 'dword ptr [eax + 0xc648f0]'), (0x4a9a05, 'fstp', 'dword ptr [esp + 0xc]'),
              (0x4a9a09, 'mov', 'ebp, 0x3f800000'), (0x4a9a0f, 'mov', 'dword ptr [esp + 0x14], ebp'),
              (0x4afe63, 'mov', 'dword ptr [ebx + 0x2c], 0'), (0x4afe70, 'mov', 'dword ptr [ebx + 0x34], 0'),
              (0x4b1582, 'mov', 'edx, 0x5f5bd4'), (0x4b15a2, 'mov', 'dword ptr [edx + 0xc648c8], eax'),
              (0x4b35c3, 'mov', 'edx, 0x5f5df9'), (0x4b35e3, 'mov', 'dword ptr [edx + 0xc64ae8], eax'),
              (0x4a9a25, 'mov', 'eax, dword ptr [eax + 0xc648c8]'), (0x4a9a2b, 'mov', 'dword ptr [esp + 0x14], eax'),
              (0x4a9af8, 'mov', 'eax, dword ptr [eax + 0xc64ae8]'), (0x4a9afe, 'mov', 'dword ptr [esp + 0x4c], eax'),
              (0x4afe2b, 'mov', 'dword ptr [ebx + 0xc], 0'), (0x4b0077, 'mov', 'dword ptr [ebx + 0x22c], 0'),
              (0x4bb661, 'fld', 'dword ptr [edx + 0xf5ffa8]'), (0x4bb66d, 'jae', '0x4bb623'),
              (0x4bb68b, 'xor', 'ecx, ecx'), (0x4bb693, 'fmul', 'dword ptr [0x771c68]')]
    for address, mnemonic, operands in checks:
        instruction = by_address.get(address)
        require(instruction is not None and instruction.mnemonic == mnemonic
                and instruction.op_str == operands, f'unexpected instruction at {address:#x}')
    for address, expected in [(0x5f3359, b'objdef.d%02ld'), (0x5f3187, b'objdef.dau'), (0x5f29ad, b'[DefaultPath]'), (0x5f5c0f, b'%ld'), (0x5f5c17, b'%ld'), (0x5f5bd4, b'%f'), (0x5f5df9, b'%ld')]:
        require(read_va(exe, address, 32).split(b'\0')[0] == expected, 'unexpected source string')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'parser_case': 60, 'runtime_field': '0xc64922 + type * 0x2a4',
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def inspect_maps(root):
    prefix = 'SYSTEM/DATA_MP/DEFAULTS/'
    config = root / 'SYSTEM/clengine.ini'
    if config.exists():
        lines = decode(config.read_bytes()).decode('cp1252').splitlines()
        section = False
        for line in lines:
            row = line.split(';')[0].strip()
            if row.startswith('['):
                section = row.lower() == '[defaultpath]'
            elif section and row:
                relative = Path(row.replace('\\', '/'))
                require(not relative.is_absolute(), 'unsupported absolute DefaultPath')
                prefix = row.replace('\\', '/')
                require((root / relative).resolve().is_relative_to(root.resolve()), 'DefaultPath outside game root')
                break
        else:
            raise ValueError('missing DefaultPath')
    definitions, sources = {}, []
    for filename in ['objdef.dau'] + [f'objdef.d{i:02d}' for i in range(10)]:
        source = (root / (prefix + filename)).resolve()
        if not source.exists():
            continue
        raw = source.read_bytes()
        text = decode(raw).decode('cp1252')
        section = False
        for line in text.splitlines():
            row = line.split(';')[0].strip()
            if row.startswith('['):
                if section:
                    break
                section = row.lower() == '[objectdefaults]'
                continue
            if not section or not row:
                continue
            columns = [c.strip() for c in row.split(',') if c != '']
            if not columns:
                continue
            type_id = int(columns[0])
            require(0 <= type_id < 2500, 'invalid type')
            previous = definitions.get(type_id, {'active': False, 'afstr': 0, 'initial_hp': 0, 'initial_morale': 0, 'initial_moves': 0.0, 'initial_maxruhm': 0}).copy()
            if len(columns) > 1:
                flag = int(columns[1]); require(0 <= flag <= 255, 'invalid active flag')
                previous['active'] = flag != 0
            if len(columns) > 60:
                afstr = int(columns[60]); require(-32768 <= afstr <= 32767, 'invalid afstr')
                previous['afstr'] = afstr
            for column, key in [(19, 'initial_hp'), (21, 'initial_morale'), (152, 'initial_maxruhm')]:
                if len(columns) > column:
                    value = int(columns[column]); require(-2147483648 <= value <= 2147483647, 'invalid signed32 vital')
                    previous[key] = value
            if len(columns) > 4:
                value = float(columns[4]); require(math.isfinite(value), 'nonfinite moves')
                value = struct.unpack('<f', struct.pack('<f', value))[0]
                require(math.isfinite(value), 'float32 moves overflow')
                previous['initial_moves'] = value
            definitions[type_id] = previous
        require(source.read_bytes() == raw, 'source changed during audit')
        sources.append({'file': str(source.relative_to(root.resolve())), 'sha256': hashlib.sha256(raw).hexdigest()})
    require(definitions, 'no effective definitions')
    maps = []
    for path in sorted((root / 'MAPS').glob('*/DATA/objects.dat')):
        packed = path.read_bytes()
        objects = decode(packed)
        require(len(objects) >= 16, 'short objects header')
        version, count, name0, name1 = struct.unpack_from('<4I', objects)
        require(version == 1 and 0 < count <= 14000 and name0 == name1 == 30
                and len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'invalid objects layout')
        column10 = 16 + count * (79 + sum(COLUMN_WIDTHS[:10]))
        counts = Counter()
        mismatches = []
        for slot in range(count):
            record = 16 + slot * 79
            if not objects[record]:
                continue
            type_id = struct.unpack_from('<H', objects, record + 75)[0]
            third = struct.unpack_from('<H', objects, column10 + slot * 2)[0]
            definition = definitions.get(type_id)
            if definition is None or not definition['active']:
                mismatches.append({'slot': slot, 'type': type_id, 'reason': 'invalid-type'})
                continue
            required = definition['afstr'] != 0
            counts['requires-third' if required else 'two-positions'] += 1
            if required != (third != 0xffff):
                mismatches.append({'slot': slot, 'type': type_id, 'third': third, 'afstr': definition['afstr']})
        require(hashlib.sha256(path.read_bytes()).digest() == hashlib.sha256(packed).digest(), 'source changed during audit')
        maps.append({'map': path.parent.parent.name, 'source_sha256': hashlib.sha256(packed).hexdigest(),
                     'counts': dict(counts), 'mismatches': mismatches})
    return {'definition_sources': sources, 'definitions': len(definitions),
            'third_position_definitions': {str(k): v for k, v in definitions.items() if v['active'] and v['afstr']},
            'maps': maps,
            'mismatch_count': sum(len(row['mismatches']) for row in maps),
            'active_objects': sum(sum(row['counts'].values()) for row in maps)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--exe', required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    installed = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    if output.exists() or output.is_relative_to(args.game_root.resolve()) or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(installed):
        parser.error('output must be new and outside source and installed-game directories')
    result = inspect_code(args.exe.read_bytes())
    result.update(inspect_maps(args.game_root))
    result['limitations'] = ['static code and disk snapshots; no runtime execution',
                            'configured loose DefaultPath and objdef.d00..d09 only; virtual directory policies/archives and runtime selection unobserved',
                            'afstr nonzero controls third-position allocation; its full gameplay semantics are unknown',
                            'other type-dependent object and auxiliary initialization remains incomplete']
    with output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({k: result[k] for k in ['exe_sha256', 'instruction_assertions', 'definitions', 'active_objects', 'mismatch_count']}))


if __name__ == '__main__':
    main()
