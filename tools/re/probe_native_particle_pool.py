#!/usr/bin/env python3
"""Read-only native particle layout, object backlinks and child-light audit."""
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

CAPACITY, CHILDREN, RECORD = 1024, 64, 2252
LENGTH = 12 + CAPACITY * (RECORD + 16)


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    instructions = [i for a, n in ((0x48dd90, 0x475), (0x4db800, 0x130))
                    for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    index = {i.address: i for i in instructions}
    checks = [(0x48de5e, 'add', 'ebp, 0x8e0'),
              (0x48deb4, 'mov', 'word ptr [ebp], ax'),
              (0x48dee0, 'mov', 'dword ptr [ebp + 4], eax'),
              (0x48df07, 'mov', 'word ptr [ebp + 8], ax'),
              (0x48df1f, 'mov', 'dword ptr [ebp + 0xc], eax'),
              (0x48df93, 'mov', 'byte ptr [edi + ebp + 0x1c], al'),
              (0x48df69, 'mov', 'word ptr [edi + 0x85c], ax'),
              (0x48de7f, 'add', 'eax, 0x10'), (0x48de84, 'call', '0x415320'),
              (0x48e156, 'add', 'eax, 0x14'), (0x48e15b, 'call', '0x4152f0'),
              (0x48e185, 'add', 'eax, 0x18'), (0x48e18a, 'call', '0x4152f0'),
              (0x48e1b4, 'add', 'eax, 0x8dc'), (0x48e1bb, 'call', '0x4152f0'),
              (0x4db80c, 'cmp', 'esi, 0x400'), (0x4db871, 'mov', 'ebp, 0x1e'),
              (0x4db876, 'mov', 'word ptr [eax + 0x12355dc], bx'),
              (0x4db87d, 'mov', 'dword ptr [eax + 0x1235eb0], ebp'),
              (0x4db8fc, 'call', '0x49f700'), (0x4db923, 'call', '0x4acb00')]
    reads = [i.op_str for i in instructions if 0x48de2e <= i.address <= 0x48e124
             and i.mnemonic == 'call' and i.op_str in ('0x414df0', '0x414e40', '0x414e90')]
    require(reads == ['0x414e40', '0x414e90', '0x414e40', '0x414e90', '0x414df0']
            + ['0x414e90'] * 7 + ['0x414e40'] * 3, 'serialized read width order mismatch')
    for address, mnemonic, operands in checks:
        i = index.get(address)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'instruction mismatch {address:#x}')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'record_bytes': RECORD, 'child_bytes': 35, 'payload_bytes': LENGTH,
            'parallel_runtime_offsets': [0x10, 0x14, 0x18, 0x8dc],
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def inspect_maps(root):
    maps, totals = [], Counter()
    for path in sorted(root.glob('MAPS/*/DATA/particle.dat')):
        paths = [path, path.with_name('objects.dat'), path.with_name('light.dat')]
        require(all(p.resolve().is_relative_to(root) for p in paths), 'source outside root')
        sources = [p.read_bytes() for p in paths]; particle, objects, light = [decode(s) for s in sources]
        require(len(particle) == LENGTH and struct.unpack_from('<3I', particle) == (1, CAPACITY, CHILDREN), 'unsupported particle layout')
        version, count, n0, n1 = struct.unpack_from('<4I', objects)
        require(version == 1 and n0 == n1 == 30 and 0 < count <= 14000
                and len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'unsupported objects layout')
        require(len(light) == 57352 and struct.unpack_from('<2I', light) == (1, 1024), 'unsupported light layout')
        backlink = 16 + count * (79 + sum(COLUMN_WIDTHS[:11]))
        counts, links, owners, light_users = Counter(), [], {}, {}
        for slot in range(CAPACITY):
            record = 12 + slot * RECORD
            active, = struct.unpack_from('<H', particle, record)
            if not active: continue
            counts['active_emitters'] += 1; owner, = struct.unpack_from('<h', particle, record + 6)
            status = 'none' if owner == -1 else 'out_of_range' if not 0 <= owner < count else 'inactive' if not objects[16 + owner * 79] else 'valid'
            counts['owner_' + status] += 1
            if status == 'valid':
                back, = struct.unpack_from('<H', objects, backlink + owner * 2)
                counts['backlink_match' if back == slot else 'backlink_mismatch'] += 1
            if owner >= 0: owners.setdefault(owner, []).append(slot)
            children = []
            for child in range(CHILDREN):
                target, = struct.unpack_from('<h', particle, record + 12 + child * 35 + 33)
                if target == -1: continue
                status = 'out_of_range' if not 0 <= target < 1024 else 'active' if struct.unpack_from('<I', light, 8 + target * 4)[0] else 'inactive'
                counts['child_light_' + status] += 1
                light_users.setdefault(target, set()).add(slot)
                children.append({'child': child, 'light': target, 'status': status})
            links.append({'slot': slot, 'active': active, 'owner': owner, 'children': children})
        for slot in range(count):
            if not objects[16 + slot * 79]: continue
            target, = struct.unpack_from('<H', objects, backlink + slot * 2)
            if target == 65535: continue
            counts['object_particle_references'] += 1
            status = 'out_of_range' if target >= 1024 else 'inactive' if not struct.unpack_from('<H', particle, 12 + target * RECORD)[0] else 'active'
            counts['object_target_' + status] += 1
        counts['shared_owner_objects'] = sum(len(slots) > 1 for slots in owners.values())
        counts['lights_shared_between_emitters'] = sum(len(slots) > 1 for slots in light_users.values())
        for source_path, source in zip(paths, sources): require(source_path.read_bytes() == source, 'source changed during audit')
        totals.update(counts)
        maps.append({'map': path.parent.parent.name, 'counts': dict(counts), 'links': links,
                     'source_sha256': {p.name: hashlib.sha256(s).hexdigest() for p, s in zip(paths, sources)}})
    require(maps, 'no source maps found')
    return {'maps': maps, 'totals': dict(totals), 'limitations': [
        'Static reader/reset and saved-map evidence; not particle simulation or game execution.',
        'Unknown child fields are retained, not given speculative semantic names.',
        'Layout verification does not prove complete create/copy or lager ownership closure.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, required=True); args = parser.parse_args()
    root, output = args.game_root.resolve(), args.output.resolve()
    require(not output.exists() and not output.is_relative_to(root)
            and not output.is_relative_to(args.exe.resolve().parent)
            and not output.is_relative_to(Path(r'C:\Program Files (x86)\Against Rome').resolve()), 'output must be new and outside sources')
    source = args.exe.read_bytes(); result = inspect_code(source); result.update(inspect_maps(root))
    require(args.exe.read_bytes() == source, 'EXE changed during audit')
    with output.open('x', encoding='utf-8') as stream: json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({'instruction_assertions': result['instruction_assertions'], 'maps': len(result['maps']), 'totals': result['totals']}))


if __name__ == '__main__': main()
