#!/usr/bin/env python3
"""Fingerprint-gated, read-only native light layout and object backlink audit."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import decode, COLUMN_WIDTHS
from probe_pua import EXE_SHA256, require

WIDTHS = (4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2)


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = ((0x48ab70, 1350), (0x49f480, 0x50), (0x49f500, 0xd2),
               (0x49f700, 0xe0), (0x48bae0, 1501), (0x4aa780, 0x680), (0x4b230c, 56))
    ins = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    by_address = {i.address: i for i in ins}
    reads = (0x48abee, 0x48ac30, 0x48ac80, 0x48acd2, 0x48ad24, 0x48ad74,
             0x48adc6, 0x48ae18, 0x48ae68, 0x48aeba, 0x48aefd, 0x48af4d,
             0x48af8f, 0x48afd1, 0x48b014, 0x48b04f)
    reader_types = ('0x414e90',) + ('0x414f10',) * 8 + ('0x414e40', '0x414f10',
                    '0x414e90', '0x414e90', '0x414e40', '0x414e40', '0x414e40')
    checks = [(a, 'call', target) for a, target in zip(reads, reader_types)]
    checks += [(0x49f502, 'call', '0x49f480'), (0x49f515, 'call', '0x49f700'),
               (0x48bf92, 'add', 'eax, 0x24'), (0x48bf97, 'call', '0x415160'),
               (0x4aad9b, 'mov', 'word ptr [ebx + 0xa14c20], ax'),
               (0x4aadb9, 'mov', 'word ptr [eax*4 + 0x7f9f24], di'),
               (0x49f741, 'mov', 'word ptr [eax*4 + 0xa14c20], 0xffff'),
               (0x4b2334, 'mov', 'word ptr [edx + 0xc6492e], ax'),
               (0x4aad81, 'push', 'dword ptr [eax*4 + 0xb18848]'),
               (0x4aad88, 'push', 'dword ptr [eax*4 + 0xb18844]'),
               (0x4aad8f, 'push', 'dword ptr [eax*4 + 0xb18840]'),
               (0x4aad96, 'call', '0x49f5f0')]
    for a, mnemonic, operands in checks:
        i = by_address.get(a)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'unexpected instruction {a:#x}')
    callbacks = [i.address for i in ins if 0x48bae0 <= i.address < 0x48bae0 + 1501
                 and i.mnemonic == 'call' and i.op_str in ('0x415160', '0x4152f0')]
    require(len(callbacks) == 18 and callbacks[12] == 0x48bf97, 'unexpected object column12 order')
    require(struct.unpack('<209I', read_va(exe, 0x4b1130, 836))[66] == 0x4b230c, 'unexpected lidef callback')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'light_column_widths': WIDTHS, 'object_backlink_column': 12,
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins]}


def inspect_maps(root):
    maps, totals = [], Counter()
    for path in sorted((root / 'MAPS').glob('*/DATA/light.dat')):
        objects_path = path.with_name('objects.dat')
        require(path.resolve().is_relative_to(root) and objects_path.resolve().is_relative_to(root), 'source outside root')
        sources = [path.read_bytes(), objects_path.read_bytes()]
        light, objects = [decode(data) for data in sources]
        require(len(light) == 57352 and struct.unpack_from('<2I', light) == (1, 1024), 'unsupported light layout')
        version, count, n0, n1 = struct.unpack_from('<4I', objects)
        require(version == 1 and 0 < count <= 14000 and n0 == n1 == 30
                and len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'unsupported objects layout')
        backlink_offset = 16 + count * (79 + sum(COLUMN_WIDTHS[:12]))
        offsets = [8 + 1024 * sum(WIDTHS[:column]) for column in range(16)]
        counts, links = Counter(), []
        for slot in range(1024):
            if not struct.unpack_from('<I', light, offsets[0] + slot * 4)[0]: continue
            counts['active'] += 1
            owner, parent, definition = [struct.unpack_from('<h', light, offsets[c] + slot * 2)[0] for c in (13, 14, 15)]
            for name, target in (('owner', owner), ('parent', parent)):
                status = 'none' if target == -1 else 'out_of_range' if not 0 <= target < count else 'inactive' if not objects[16 + target * 79] else 'valid'
                counts[f'{name}_{status}'] += 1
                if name == 'owner' and status == 'valid':
                    back = struct.unpack_from('<h', objects, backlink_offset + target * 2)[0]
                    counts['backlink_match' if back == slot else 'backlink_mismatch'] += 1
            links.append({'light': slot, 'owner': owner, 'parent': parent, 'definition': definition})
        for source_path, source in zip((path, objects_path), sources):
            require(source_path.read_bytes() == source, 'source changed during audit')
        totals.update(counts)
        maps.append({'map': path.parent.parent.name, 'source_sha256': dict(zip(('light', 'objects'),
                     (hashlib.sha256(data).hexdigest() for data in sources))), 'counts': dict(counts), 'links': links})
    require(maps, 'no source maps found')
    return {'maps': maps, 'totals': dict(totals), 'limitations': ['static code and saved maps only; no runtime acceptance',
            'batch light model is not a complete cross-pool object-family transaction']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args(); root = args.game_root.resolve(); output = args.output.resolve()
    if output.exists() or output.is_relative_to(root) or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(Path(r'C:\Program Files (x86)\Against Rome').resolve()):
        parser.error('output must be new and outside source and installed-game directories')
    source = args.exe.read_bytes()
    result = inspect_code(source); result.update(inspect_maps(root))
    require(args.exe.read_bytes() == source, 'EXE changed during audit')
    with output.open('x', encoding='utf-8') as f: json.dump(result, f, indent=2); f.write('\n')
    print(json.dumps({'instruction_assertions': result['instruction_assertions'], 'maps': len(result['maps']), 'totals': result['totals']}))


if __name__ == '__main__': main()
