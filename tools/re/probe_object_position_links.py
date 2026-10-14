#!/usr/bin/env python3
"""Read-only three-position link audit of native DATA pools, without game execution."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from probe_object_anim_links import COLUMN_WIDTHS, decode
from probe_pua import require


def inspect_native(exe):
    from capstone import Cs, CS_ARCH_X86, CS_MODE_32
    from probe_engine_config import read_va
    from probe_pua import EXE_SHA256
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    windows = {name: list(md.disasm(read_va(exe, address, size), address))
               for name, address, size in [('create', 0x4aa780, 1703),
                                           ('reader', 0x48bae0, 1501),
                                           ('release', 0x4abb80, 822),
                                           ('objdata_scalar_consumer', 0x4bb5d0, 215)]}
    # Match complete decoded instructions from function entry, rather than byte substrings.
    checks = [('create', 0x4aa89d, 'call', '0x4ab4a0'),
              ('create', 0x4aa8da, 'call', '0x4ab4a0'),
              ('create', 0x4aa929, 'cmp', 'word ptr [ebx + 0xc64922], 0'),
              ('create', 0x4aa933, 'call', '0x4ab4a0'),
              ('create', 0x4aa99a, 'mov', 'word ptr [eax*4 + 0xa14c0c], dx'),
              ('reader', 0x48bf3a, 'add', 'eax, 0x10'),
              ('reader', 0x48bf3f, 'call', '0x415160'),
              ('release', 0x4abcd7, 'call', '0x4abec0'),
              ('release', 0x4abce9, 'call', '0x4abec0'),
              ('release', 0x4abcfb, 'call', '0x4abec0'),
              ('objdata_scalar_consumer', 0x4bb661, 'fld', 'dword ptr [edx + 0xf5ffa8]')]
    for name, address, mnemonic, operands in checks:
        require(any(i.address == address and i.mnemonic == mnemonic and i.op_str == operands
                    for i in windows[name]), f'native instruction mismatch at {address:#x}')
    return {'exe_sha256': EXE_SHA256,
            'instructions': {name: [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}'
                                    for i in ins] for name, ins in windows.items()},
            'limitations': ['four static function bodies; no game execution',
                            'column 10 mapping uses reader array order and established column widths',
                            'archetype flag names and third-position runtime consumers remain unresolved']}


def inspect_links(objects, positions):
    objects, positions = decode(objects), decode(positions)
    require(len(objects) >= 16 and len(positions) >= 8, 'short pool header')
    version, count, n0, n1 = struct.unpack_from('<4I', objects)
    pv, pc = struct.unpack_from('<2I', positions)
    require(version == pv == 1 and 0 < count <= 14000 and n0 == n1 == 30,
            'unsupported objects/position layout')
    require(0 < pc <= 33000 and len(positions) == 8 + pc * 17, 'position length mismatch')
    require(len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'objects length mismatch')
    third = 16 + count * (79 + sum(COLUMN_WIDTHS[:10]))
    uses, thirds, types = Counter(), [], Counter()
    invalid, inactive = [], []
    for slot in range(count):
        record = 16 + slot * 79
        if not objects[record]:
            continue
        links = (*struct.unpack_from('<2H', objects, record + 67),
                 struct.unpack_from('<H', objects, third + slot * 2)[0])
        if links[2] != 0xffff:
            thirds.append({'object_slot': slot, 'position_slot': links[2],
                           'type': struct.unpack_from('<H', objects, record + 75)[0]})
            types[thirds[-1]['type']] += 1
        for link_index, target in enumerate(links):
            if target == 0xffff:
                continue
            item = {'object_slot': slot, 'link_index': link_index, 'position_slot': target}
            if target >= pc:
                invalid.append(item)
            else:
                uses[target] += 1
                if not positions[8 + target * 17]:
                    inactive.append(item)
    return {'objects_decoded_sha256': hashlib.sha256(objects).hexdigest(),
            'positions_decoded_sha256': hashlib.sha256(positions).hexdigest(),
            'third_position_links': thirds, 'third_position_types': dict(types),
            'out_of_range_links': invalid, 'inactive_target_links': inactive,
            'shared_position_slots': {str(k): v for k, v in sorted(uses.items()) if v > 1}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, help='also verify fingerprinted native instruction paths')
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    game = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    if output.exists() or output.is_relative_to(root) or output.is_relative_to(game):
        parser.error('output must be new and outside source directory')
    require((root / 'MAPS').is_dir(), 'missing MAPS directory')
    native = inspect_native(args.exe.read_bytes()) if args.exe else None
    maps, missing = [], []
    for directory in sorted((root / 'MAPS').iterdir()):
        if not directory.is_dir() or directory.is_symlink():
            continue
        paths = [directory / 'DATA' / name for name in ('objects.dat', 'position.dat')]
        if not all(p.is_file() for p in paths):
            missing.append(directory.name)
            continue
        require(all(p.resolve().is_relative_to(root) for p in paths), 'pool path escapes source')
        maps.append({'map': directory.name, **inspect_links(*(p.read_bytes() for p in paths))})
    with output.open('x', encoding='utf-8') as stream:
        json.dump({'maps': maps, 'missing_pool_pairs': missing, 'native': native,
                  'limitations': ['snapshot of three DATA position links only',
                  'third link is zero-based objects column 10, runtime +0x10',
                  'shared slots are observations, not established corruption',
                  'does not determine archetype requirements, runtime use or creation semantics']}, stream, indent=2)
        stream.write('\n')
    print(json.dumps({'maps': len(maps),
                      'third_links': sum(len(m['third_position_links']) for m in maps),
                      'out_of_range': sum(len(m['out_of_range_links']) for m in maps),
                      'inactive': sum(len(m['inactive_target_links']) for m in maps),
                      'shared_slots': sum(len(m['shared_position_slots']) for m in maps)}))


if __name__ == '__main__':
    main()
