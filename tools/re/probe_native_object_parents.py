#!/usr/bin/env python3
"""Read-only proof and audit of objects column13 parent links."""
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


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = [(0x48bae0, 1501), (0x4a9740, 0x609), (0x4c9790, 0x89),
               (0x4c9820, 0x1bb), (0x4c9db0, 0xb7), (0x4c9e70, 0x140),
               (0x4c9610, 0x17a), (0x4b37e7, 0x70), (0x4b3bc7, 0x70)]
    ins = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    by_address = {i.address: i for i in ins}
    checks = [(0x48bfbe, 'add', 'eax, 0x30'), (0x48bfc3, 'call', '0x415160'),
              (0x4a9cb6, 'call', '0x4c9590'), (0x4a9cbf, 'call', '0x4c9790'),
              (0x4a9cc8, 'call', '0x4c9db0'), (0x4a9cd1, 'call', '0x4c9610'),
              (0x4a9cda, 'call', '0x4c9820'), (0x4a9ce3, 'call', '0x4c9e70'),
              (0x4a9cee, 'call', '0x4e1470'),
              (0x4c97c8, 'mov', 'eax, dword ptr [eax + 0xc64b06]'),
              (0x4c97fa, 'mov', 'eax, dword ptr [esi + 0xa14c2a]'), (0x4c9808, 'call', '0x4acb00'),
              (0x4c9884, 'mov', 'ebp, dword ptr [ebx + 0xc64b06]'),
              (0x4c989a, 'mov', 'eax, dword ptr [ebx + 0xc64b08]'),
              (0x4c98c6, 'call', '0x4c84e0'), (0x4c9981, 'call', '0x4a9740'),
              (0x4c99ba, 'mov', 'word ptr [esi + 0xa14c2c], ax'),
              (0x4c9e0e, 'call', '0x4c9d80'), (0x4c9e3f, 'mov', 'eax, dword ptr [esi + 0xa14c2a]'),
              (0x4c9e4d, 'call', '0x4acb00'), (0x4c9eef, 'call', '0x4c7cf0'),
              (0x4c9f74, 'call', '0x4a9740'), (0x4c9f9f, 'mov', 'word ptr [eax*4 + 0xa14c2c], di'),
              (0x4c966d, 'mov', 'ebp, dword ptr [ebx + 0xc64b02]'),
              (0x4c9677, 'call', '0x4d2f50'), (0x4c9751, 'call', '0x49f5f0'),
              (0x4b380f, 'mov', 'word ptr [edx + 0xc64b04], ax'),
              (0x4b3847, 'mov', 'word ptr [edx + 0xc64b06], ax'),
              (0x4b3bef, 'mov', 'word ptr [edx + 0xc64b08], ax'),
              (0x4b3c27, 'mov', 'word ptr [edx + 0xc64b0a], ax')]
    for a, mnemonic, operands in checks:
        i = by_address.get(a)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'unexpected instruction {a:#x}')
    table = struct.unpack('<209I', read_va(exe, 0x4b1130, 836))
    require([table[c] for c in (162, 163, 180, 181)] == [0x4b37e7, 0x4b381f, 0x4b3bc7, 0x4b3bff], 'unexpected parser cases')
    # Reader array order includes the three dword columns; column13 is the fourteenth callback.
    callbacks = []
    for i in ins:
        if 0x48bae0 <= i.address < 0x48bae0+1501 and i.mnemonic == 'call' and i.op_str in ('0x415160', '0x4152f0'):
            callbacks.append(i.address)
    require(len(callbacks) == 18 and callbacks[13] == 0x48bfc3, 'unexpected serialized array order')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks), 'column': 13,
            'runtime_field': 'object+0x30', 'reader_array_callbacks': [hex(a) for a in callbacks],
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins]}


def inspect_maps(root):
    maps, total = [], Counter()
    for path in sorted((root / 'MAPS').glob('*/DATA/objects.dat')):
        require(path.resolve().is_relative_to(root), 'source outside root')
        packed = path.read_bytes(); objects = decode(packed)
        version, count, n0, n1 = struct.unpack_from('<4I', objects)
        require(version == 1 and 0 < count <= 14000 and n0 == n1 == 30 and len(objects) == 16+count*(79+sum(COLUMN_WIDTHS)), 'invalid objects layout')
        offset = 16+count*(79+sum(COLUMN_WIDTHS[:13]))
        counts, parents, links = Counter(), set(), []
        for slot in range(count):
            if not objects[16+slot*79]: continue
            parent = struct.unpack_from('<H', objects, offset+slot*2)[0]
            if parent == 0xffff: continue
            parents.add(parent); counts['children'] += 1
            status = 'out-of-range' if parent >= count else 'self' if parent == slot else 'inactive' if not objects[16+parent*79] else 'valid'
            counts[status] += 1
            links.append({'child': slot, 'parent': parent, 'status': status, 'type': struct.unpack_from('<H', objects, 16+slot*79+75)[0]})
        counts['parents'] = len(parents); total.update(counts)
        require(path.read_bytes() == packed, 'source changed during audit')
        maps.append({'map': path.parent.parent.name, 'sha256': hashlib.sha256(packed).hexdigest(), 'counts': dict(counts), 'links': links})
    return {'maps': maps, 'totals': dict(total), 'limitations': ['static code and disk snapshots; no game execution', 'APT projection and effect pool transactions incomplete', 'parent and child linkage proven; full attached-object creation remains unimplemented']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args(); root = args.game_root.resolve(); output = args.output.resolve()
    if output.exists() or output.is_relative_to(root) or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(Path(r'C:\Program Files (x86)\Against Rome').resolve()):
        parser.error('output must be new and outside source and installed-game directories')
    result = inspect_code(args.exe.read_bytes()); result.update(inspect_maps(root))
    with output.open('x', encoding='utf-8') as f: json.dump(result, f, indent=2); f.write('\n')
    print(json.dumps({'instruction_assertions': result['instruction_assertions'], 'maps': len(result['maps']), 'totals': result['totals']}))


if __name__ == '__main__': main()
