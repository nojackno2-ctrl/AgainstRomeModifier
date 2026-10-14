#!/usr/bin/env python3
"""Read-only typus creation proof; saved field values are observations, not invariants."""
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

WIDTHS = (9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 2, 2)


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    require(struct.unpack('<209I', read_va(exe, 0x4b1130, 836))[54] == 0x4b206c, 'unexpected typus case')
    require(struct.unpack('<7I', read_va(exe, 0x4d7810, 28)) ==
            (0x4d7855, 0x4d787f, 0x4d78c1, 0x4d790b, 0x4d794d, 0x4d798f, 0x4d79d1), 'unexpected typus callbacks')
    regions = [(0x480850, 0x1000), (0x4afe10, 864), (0x4b206c, 0x37),
               (0x4a9740, 0x609), (0x4d7830, 0x1e3), (0x4d7a20, 0x5c),
               (0x4d7b20, 0x31), (0x4d7b70, 0x17), (0x4d7b90, 0x23), (0x4d7bd0, 0x3d)]
    instructions = [i for a, size in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, size), a)]
    by_address = {i.address: i for i in instructions}
    checks = [(0x480ca5, 'call', '0x4d7b70'), (0x480dc7, 'push', '0x5f3212'),
              (0x480dcc, 'call', '0x4d7a20'), (0x480fe3, 'push', '0x5f3394'),
              (0x480ff8, 'call', '0x4d7a20'), (0x4810e8, 'inc', 'ebx'),
              (0x4810f1, 'cmp', 'ebx, 0xa'), (0x4810f4, 'jl', '0x480f4c'),
              (0x4aff9b, 'mov', 'word ptr [ebx + 0xbe], 0xffff'),
              (0x4b2074, 'mov', 'edx, 0x5f5c99'), (0x4b2094, 'mov', 'word ptr [edx + 0xc6497a], ax'),
              (0x4a9a32, 'mov', 'word ptr [esp + 0x1c], si'), (0x4a9a41, 'call', '0x4afd80'),
              (0x4a9a53, 'mov', 'esi, dword ptr [esi + 0xc64978]'), (0x4a9a59, 'sar', 'esi, 0x10'),
              (0x4a9a5d, 'call', '0x4d7b20'), (0x4a9a67, 'je', '0x4a9a88'),
              (0x4a9a77, 'test', 'byte ptr [eax*4 + 0x9f5031], 4'),
              (0x4a9a7f, 'je', '0x4a9a88'), (0x4a9a81, 'mov', 'word ptr [esp + 0x1c], 0x64'),
              (0x4d784e, 'jmp', 'dword ptr [edx*4 + 0x4d7810]'),
              (0x4d7887, 'push', '0x5f676c'), (0x4d78b3, 'mov', 'dword ptr [edx + 0x9f502c], eax'),
              (0x4d7913, 'push', '0x5f6773'), (0x4d793f, 'mov', 'dword ptr [edx + 0x9f5030], eax'),
              (0x4d7a26, 'push', '0x770de8'), (0x4d7a2b, 'push', '0x5f677f'),
              (0x4d7a39, 'call', '0x572f50'), (0x4d7a52, 'push', '0x4d7830'),
              (0x4d7a5e, 'push', '0x9c4'), (0x4d7a63, 'push', '0x5f6786'), (0x4d7a69, 'call', '0x42a230'),
              (0x4d7b26, 'jl', '0x4d7b4e'), (0x4d7b28, 'cmp', 'edx, 0x9c4'),
              (0x4d7b3e, 'cmp', 'dword ptr [eax*4 + 0x9f502c], 0'),
              (0x4d7b74, 'call', '0x4d7b90'), (0x4d7b7d, 'cmp', 'ebx, 0x9c4'),
              (0x4d7baa, 'call', '0x4d7bd0'), (0x4d7be6, 'mov', 'dword ptr [ebx], 0'),
              (0x4d7bf6, 'lea', 'edx, [ebx + 0x10]'), (0x4d7bfc, 'mov', 'dword ptr [eax], 0'),
              (0x4d7c04, 'jne', '0x4d7bf9')]
    for address, mnemonic, operands in checks:
        i = by_address.get(address)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'unexpected instruction {address:#x}')
    strings = [(0x5f3212, b'typus.dau'), (0x5f3394, b'typus.d%02ld'), (0x5f677f, b'%s%s'),
               (0x5f6786, b'[TypusDefaults]'), (0x5f676c, b'%ld'), (0x5f6773, b'%x'), (0x5f5c99, b'%ld')]
    for address, expected in strings:
        require(read_va(exe, address, 32).split(b'\0')[0] == expected, 'unexpected literal')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks), 'literal_assertions': len(strings),
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def inspect_maps(root):
    prefix = 'SYSTEM/DATA_MP/DEFAULTS/'
    config = root / 'SYSTEM/clengine.ini'
    if config.exists():
        section = False
        for line in decode(config.read_bytes()).decode('cp1252').splitlines():
            row = line.split(';')[0].strip()
            if row.startswith('['): section = row.lower() == '[defaultpath]'
            elif section and row:
                prefix = row.replace('\\', '/')
                require(not Path(prefix).is_absolute() and (root / prefix).resolve().is_relative_to(root), 'invalid DefaultPath')
                break
        else: raise ValueError('missing DefaultPath')
    tables, sources = {}, []
    for family, heading in [('objdef', '[objectdefaults]'), ('typus', '[typusdefaults]')]:
        table = tables[family] = {}
        for file in [family + '.dau'] + [f'{family}.d{i:02d}' for i in range(10)]:
            path = (root / (prefix + file)).resolve()
            require(path.is_relative_to(root), 'source outside root')
            if not path.exists(): continue
            packed = path.read_bytes(); section = False
            for line in decode(packed).decode('cp1252').splitlines():
                row = line.split(';')[0].strip()
                if row.startswith('['):
                    if section: break
                    section = row.lower() == heading
                    continue
                if not section or not row: continue
                c = [v.strip() for v in row.split(',') if v != '']
                if not c: continue
                id = int(c[0]); require(0 <= id < 2500, 'invalid table index')
                entry = table.setdefault(id, {'active': False, 'index': -1, 'flags': 0})
                if len(c) > 1: entry['active'] = int(c[1]) != 0
                if family == 'objdef' and len(c) > 54: entry['index'] = int(c[54])
                if family == 'typus' and len(c) > 3: entry['flags'] = int(c[3], 16)
            require(path.read_bytes() == packed, 'definition changed during audit')
            sources.append({'file': str(path.relative_to(root)), 'sha256': hashlib.sha256(packed).hexdigest()})
    observations, maps = Counter(), []
    for path in sorted((root / 'MAPS').glob('*/DATA/objects.dat')):
        data_path = path.with_name('objdata.dat')
        require(path.resolve().is_relative_to(root) and data_path.resolve().is_relative_to(root), 'map outside root')
        packed, data_packed = path.read_bytes(), data_path.read_bytes()
        objects, data = decode(packed), decode(data_packed)
        version, count, name0, name1 = struct.unpack_from('<4I', objects)
        require(version == 1 and 0 < count <= 14000 and name0 == name1 == 30 and len(objects) == 16 + count*(79+sum(COLUMN_WIDTHS)), 'invalid objects')
        require(struct.unpack_from('<2I', data) == (1, count) and len(data) == 8 + count*sum(WIDTHS), 'invalid objdata')
        counts = Counter()
        for slot in range(count):
            record = 16 + slot*79
            if not objects[record]: continue
            type_id = struct.unpack_from('<H', objects, record+75)[0]
            entry = tables['objdef'].get(type_id, {})
            typus = tables['typus'].get(entry.get('index', -1), {})
            expected = 100 if entry.get('active') and typus.get('active') and typus.get('flags', 0)&0x400 else 0
            target = struct.unpack_from('<H', objects, 16+count*(79+COLUMN_WIDTHS[0])+slot*2)[0]
            require(target < count, 'invalid objdata link')
            actual = struct.unpack_from('<H', data, 8+count*sum(WIDTHS[:4])+target*2)[0]
            key = f'create={expected},saved={actual}'
            counts[key] += 1; observations[key] += 1
        require(path.read_bytes() == packed and data_path.read_bytes() == data_packed, 'map changed during audit')
        maps.append({'map': path.parent.parent.name, 'observations': dict(counts),
                     'objects_sha256': hashlib.sha256(packed).hexdigest(), 'objdata_sha256': hashlib.sha256(data_packed).hexdigest()})
    return {'sources': sources, 'definitions': len(tables['objdef']), 'typus_records': len(tables['typus']),
            'maps': maps, 'observations': dict(observations), 'active_objects': sum(observations.values())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args(); root = args.game_root.resolve(); output = args.output.resolve()
    if output.exists() or output.is_relative_to(root) or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(Path(r'C:\Program Files (x86)\Against Rome').resolve()):
        parser.error('output must be new and outside source and installed-game directories')
    result = inspect_code(args.exe.read_bytes()); result.update(inspect_maps(root))
    result['limitations'] = ['static code and disk observations; no runtime proof', 'saved field may change after creation; differences are not validation errors', 'loose DefaultPath layers only; VFS runtime selection unobserved', 'field1c gameplay name and post-create consumers unverified']
    with output.open('x', encoding='utf-8') as f: json.dump(result, f, indent=2); f.write('\n')
    print(json.dumps({k: result[k] for k in ('instruction_assertions', 'literal_assertions', 'definitions', 'typus_records', 'active_objects', 'observations')}))


if __name__ == '__main__': main()
