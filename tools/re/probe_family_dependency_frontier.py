#!/usr/bin/env python3
"""Read-only proof of pool dependencies still preventing original object-family editing."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import COLUMN_WIDTHS, decode
from probe_pua import EXE_SHA256, require


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = ((0x48bae0, 1501), (0x4abb80, 822), (0x4db800, 0x130), (0x4ac3e0, 0xaa))
    ins = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    index = {i.address: i for i in ins}
    checks = ((0x48be5e, 'add', 'eax, 0x20'), (0x48be63, 'call', '0x415160'),
              (0x48bf66, 'add', 'eax, 0x22'), (0x48bf6b, 'call', '0x415160'),
              (0x4abd6c, 'mov', 'eax, dword ptr [esi + 0xa14c1a]'), (0x4abd79, 'call', '0x4ac3e0'),
              (0x4abd90, 'mov', 'eax, dword ptr [esi + 0xa14c1c]'), (0x4abd9d, 'call', '0x4db800'),
              (0x4ac40b, 'mov', 'byte ptr [edx + 0x120fdd4], ah'),
              (0x4db82c, 'mov', 'word ptr [ebx + 0x12355d4], dx'),
              (0x4db83b, 'mov', 'eax, dword ptr [ebx + 0x12355da]'),
              (0x4db841, 'sar', 'eax, 0x10'), (0x4db8fc, 'call', '0x49f700'),
              (0x4db923, 'call', '0x4acb00'))
    for a, mnemonic, operands in checks:
        require((index[a].mnemonic, index[a].op_str) == (mnemonic, operands), f'instruction mismatch {a:#x}')
    callbacks = [i.address for i in ins if 0x48bae0 <= i.address < 0x48bae0 + 1501
                 and i.mnemonic == 'call' and i.op_str in ('0x415160', '0x4152f0')]
    require(len(callbacks) == 18 and callbacks[5] == 0x48be63 and callbacks[11] == 0x48bf6b, 'column order mismatch')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'known_dependencies': {'object_column5': 'lager.dat runtime+20', 'object_column11': 'particle.dat runtime+22',
                                   'particle_owner': 'runtime+8 signed16; reset queues owner deletion',
                                   'particle_sublights': '64 signed16 indices at runtime+85c; reset calls light release'},
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins]}


def inspect_maps(root):
    maps, totals = [], Counter()
    for path in sorted(root.glob('MAPS/*/DATA/objects.dat')):
        packed = path.read_bytes(); data = decode(packed)
        version, n, n0, n1 = struct.unpack_from('<4I', data)
        require(version == 1 and n0 == n1 == 30 and 0 < n <= 14000 and len(data) == 16 + n * (79 + sum(COLUMN_WIDTHS)), 'unsupported objects')
        offsets, cursor = [], 16 + n * 79
        for width in COLUMN_WIDTHS: offsets.append(cursor); cursor += n * width
        def col(column, slot): return struct.unpack_from('<H', data, offsets[column] + slot * 2)[0]
        children = {}
        for slot in range(n):
            if data[16 + slot * 79] and col(13, slot) != 65535: children.setdefault(col(13, slot), []).append(slot)
        counts, families = Counter(), []
        for parent, members in sorted(children.items()):
            require(parent < n and data[16 + parent * 79], 'invalid saved parent')
            protected = {str(k): col(k, parent) for k in (2, 3, 4, 5, 8, 11) if col(k, parent) != 65535}
            counts['parents'] += 1; counts['children'] += len(members)
            counts['parents_with_lager'] += '5' in protected
            counts['children_with_particle'] += sum(col(11, slot) != 65535 for slot in members)
            counts['unprotected_parent_with_particle_child'] += not protected and any(col(11, slot) != 65535 for slot in members)
            families.append({'parent': parent, 'uid': struct.unpack_from('<I', data, 16 + parent * 79 + 3)[0],
                             'protected_columns': protected, 'children': members,
                             'child_particle_slots': [col(11, slot) for slot in members]})
        require(path.read_bytes() == packed, 'source changed during audit')
        totals.update(counts)
        maps.append({'map': path.parent.parent.name, 'source_sha256': hashlib.sha256(packed).hexdigest(), 'counts': dict(counts), 'families': families})
    require(maps, 'no original maps found')
    return {'maps': maps, 'totals': dict(totals), 'limitations': ['static code and saved map evidence; not game execution',
            'current family delete supports known object/light pools only; original particle/lager dependencies remain unimplemented']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path); parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args(); root = args.game_root.resolve(); output = args.output.resolve(); source = args.exe.read_bytes()
    require(not output.exists() and not output.is_relative_to(root) and not output.is_relative_to(args.exe.resolve().parent), 'output must be new and outside sources')
    result = inspect_code(source); result.update(inspect_maps(root))
    require(args.exe.read_bytes() == source, 'EXE source changed during audit')
    with output.open('x', encoding='utf-8') as stream: json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({'instructions': result['instruction_assertions'], 'maps': len(result['maps']), 'totals': result['totals']}))


if __name__ == '__main__': main()
