#!/usr/bin/env python3
"""Read-only proof of native lager/biglager layouts and object ownership frontier."""
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
    regions = ((0x48d2d0, 0x2b5), (0x48d590, 0x140), (0x4ac3e0, 0xb2),
               (0x4ac4a0, 0x39), (0x4d4870, 0x17d), (0x415030, 0x130))
    instructions = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    by_address = {i.address: i for i in instructions}
    checks = ((0x48d3d4, 'add', 'edi, 0x30'),
              (0x48d432, 'mov', 'byte ptr [ebx], al'),
              (0x48d45c, 'mov', 'dword ptr [ebx + 4], eax'),
              (0x48d497, 'mov', 'dword ptr [ebx + 8], eax'),
              (0x48d4bf, 'mov', 'word ptr [ebx + 0xc], ax'),
              (0x48d4c8, 'lea', 'eax, [ebx + 0xe]'),
              (0x48d4d1, 'call', '0x415160'),
              (0x48d514, 'mov', 'word ptr [edi + 0x1a], ax'),
              (0x48d41a, 'mov', 'word ptr [edi - 2], ax'),
              (0x4ac3ef, 'cmp', 'esi, 0xc80'),
              (0x4ac40b, 'mov', 'byte ptr [edx + 0x120fdd4], ah'),
              (0x4ac432, 'mov', 'word ptr [eax + 0x120fde0], cx'),
              (0x4ac456, 'mov', 'word ptr [eax + 0x120fdec], di'),
              (0x4ac46b, 'call', '0x4ac4a0'),
              (0x4ac470, 'mov', 'word ptr [ebx + 0x120fe02], di'),
              (0x4ac4a9, 'cmp', 'edx, 0x20'),
              (0x4ac4ae, 'imul', 'edx, edx, 0x642'),
              (0x4ac4bb, 'mov', 'byte ptr [edx + 0x153a718], ah'),
              (0x4ac4cc, 'mov', 'word ptr [eax + 0x153a718], bx'))
    for address, mnemonic, operands in checks:
        i = by_address.get(address)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'instruction mismatch {address:#x}')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'lager': {'header': [1, 3200, 6, 10], 'record_bytes': 43, 'tail_array_width': 2},
            'biglager': {'header': [1, 32, 800], 'record_bytes': 1601},
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def inspect_maps(root):
    maps, totals = [], Counter()
    for path in sorted(root.glob('MAPS/*/DATA/lager.dat')):
        paths = [path, path.with_name('biglager.dat'), path.with_name('objects.dat')]
        require(all(p.resolve().is_relative_to(root) for p in paths), 'source outside root')
        sources = [p.read_bytes() for p in paths]; lager, big, objects = [decode(p) for p in sources]
        require(len(lager) == 144016 and struct.unpack_from('<4I', lager) == (1, 3200, 6, 10), 'unsupported lager layout')
        require(len(big) == 51244 and struct.unpack_from('<3I', big) == (1, 32, 800), 'unsupported biglager layout')
        version, count, n0, n1 = struct.unpack_from('<4I', objects)
        require(version == 1 and n0 == n1 == 30 and 0 < count <= 14000
                and len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'unsupported object layout')
        offset = 16 + count * (79 + sum(COLUMN_WIDTHS[:5]))
        counts, owners, big_owners, links = Counter(), {}, {}, []
        def member_status(member):
            return 'out_of_range' if not 0 <= member < count else 'active' if objects[16 + member * 79] else 'inactive'
        for slot in range(count):
            if not objects[16 + slot * 79]: continue
            target, = struct.unpack_from('<H', objects, offset + slot * 2)
            if target == 65535: continue
            counts['object_lager_references'] += 1; owners.setdefault(target, []).append(slot)
            status = 'out_of_range' if target >= 3200 else 'active' if lager[16 + target * 43] else 'inactive'
            counts['object_target_' + status] += 1
        for slot in range(3200):
            record = 16 + slot * 43
            if not lager[record]: continue
            counts['active_lager'] += 1
            occupancy, = struct.unpack_from('<h', lager, record + 9)
            members = [m for m in struct.unpack_from('<10h', lager, record + 23) if m != -1]
            target, = struct.unpack_from('<h', lager, 16 + 3200 * 43 + slot * 2)
            counts['nonzero_occupancy'] += occupancy != 0
            counts['unreferenced_lager'] += slot not in owners
            for member in members: counts['small_member_' + member_status(member)] += 1
            if target != -1:
                counts['lager_big_references'] += 1; big_owners.setdefault(target, []).append(slot)
                status = 'out_of_range' if not 0 <= target < 32 else 'active' if big[12 + target * 1601] else 'inactive'
                counts['big_target_' + status] += 1
            links.append({'lager': slot, 'owner_objects': owners.get(slot, []), 'occupancy': occupancy,
                          'small_members': members, 'biglager': target})
        big_links = []
        for slot in range(32):
            record = 12 + slot * 1601
            if not big[record]: continue
            counts['active_biglager'] += 1; counts['unreferenced_biglager'] += slot not in big_owners
            members = [m for m in struct.unpack_from('<800h', big, record + 1) if m != -1]
            for member in members: counts['big_member_' + member_status(member)] += 1
            big_links.append({'biglager': slot, 'lager_users': big_owners.get(slot, []), 'members': members})
        counts['lager_shared_between_objects'] = sum(len(v) > 1 for v in owners.values())
        counts['biglager_shared_between_lagers'] = sum(len(v) > 1 for v in big_owners.values())
        for p, s in zip(paths, sources): require(p.read_bytes() == s, 'source changed during audit')
        totals.update(counts)
        maps.append({'map': path.parent.parent.name, 'counts': dict(counts), 'lager_links': links, 'biglager_links': big_links,
                     'source_sha256': {p.name: hashlib.sha256(s).hexdigest() for p, s in zip(paths, sources)}})
    require(maps, 'no source maps found')
    return {'maps': maps, 'totals': dict(totals), 'limitations': [
        'Static layout/reset and saved-map evidence; not runtime acceptance.',
        'Member consumers use object slots; lifecycle, flags and all six auxiliary fields remain incomplete.',
        'Unreferenced active slots are reported; no automatic deletion or repair is inferred.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, required=True); args = parser.parse_args()
    root, output = args.game_root.resolve(), args.output.resolve()
    require(not output.exists() and not output.is_relative_to(root) and not output.is_relative_to(args.exe.resolve().parent)
            and not output.is_relative_to(Path(r'C:\Program Files (x86)\Against Rome').resolve()), 'output must be new and outside sources')
    source = args.exe.read_bytes(); result = inspect_code(source); result.update(inspect_maps(root))
    require(args.exe.read_bytes() == source, 'EXE changed during audit')
    with output.open('x', encoding='utf-8') as stream: json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({'instruction_assertions': result['instruction_assertions'], 'maps': len(result['maps']), 'totals': result['totals']}))


if __name__ == '__main__': main()
