#!/usr/bin/env python3
"""Read-only fingerprint-gated light definition initialization and loading proof."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import decode
from probe_native_light_pool import inspect_maps
from probe_pua import EXE_SHA256, require


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = ((0x4d2fe0, 81), (0x4d3070, 555), (0x4d32a0, 110), (0x480850, 3288))
    ins = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    by_address = {i.address: i for i in ins}
    checks = [(0x4d2fe9, 'mov', 'dword ptr [eax], 0'),
              (0x4d2ff2, 'mov', 'dword ptr [eax - 0x1c], 0x3f800000'),
              (0x4d3007, 'mov', 'dword ptr [eax - 0x10], 0x43fa0000'),
              (0x4d308e, 'jmp', 'dword ptr [edx*4 + 0x4d3040]'),
              (0x4d30e5, 'mov', 'dword ptr [edx + 0x80afcc], eax'),
              (0x4d3119, 'mov', 'dword ptr [edx + 0x80afd0], eax'),
              (0x4d314d, 'mov', 'dword ptr [edx + 0x80afd4], eax'),
              (0x4d3181, 'mov', 'dword ptr [edx + 0x80afd8], eax'),
              (0x4d31b5, 'mov', 'dword ptr [edx + 0x80afdc], eax'),
              (0x4d31e9, 'mov', 'word ptr [edx + 0x80afe0], ax'),
              (0x4d321e, 'mov', 'dword ptr [edx + 0x80afe4], eax'),
              (0x4d3252, 'mov', 'dword ptr [edx + 0x80afe8], eax'),
              (0x4d328e, 'cmp', 'edx, 0x1e'), (0x480ca0, 'call', '0x4d2f90'),
              (0x480da1, 'call', '0x4d32a0')]
    for a, mnemonic, operands in checks:
        i = by_address.get(a)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'unexpected instruction {a:#x}')
    entries = (0x4d3095, 0x4d30bf, 0x4d30f3, 0x4d3127, 0x4d315b,
               0x4d318f, 0x4d31c3, 0x4d31f8, 0x4d322c, 0x4d3260)
    require(struct.unpack('<10I', read_va(exe, 0x4d3040, 40)) == entries, 'unexpected callback table')
    for a, value in ((0x5f31ee, b'lightdef.dau'), (0x5f3384, b'lightdef.d%02ld'),
                     (0x5f658a, b'LD_NoNameEntered'), (0x5f6699, b'[LightDefault]')):
        require(read_va(exe, a, len(value) + 1) == value + b'\0', 'unexpected literal')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'callback_entries': [hex(a) for a in entries],
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins]}


def inspect_definitions(root):
    prefix = 'SYSTEM/DATA_MP/DEFAULTS/'
    config = root / 'SYSTEM/clengine.ini'
    if config.exists():
        packed = config.read_bytes(); in_section = False
        for line in decode(packed).decode('cp1251').splitlines():
            row = line.split(';')[0].strip()
            if row.startswith('['): in_section = row.lower() == '[defaultpath]'
            elif in_section and row:
                prefix = row.replace('\\', '/')
                require(not Path(prefix).is_absolute() and (root / prefix).resolve().is_relative_to(root), 'DefaultPath outside source')
                break
        else: raise ValueError('missing DefaultPath')
        require(config.read_bytes() == packed, 'config source changed during audit')
    definitions, sources = {}, []
    for suffix in ['dau'] + [f'd{i:02}' for i in range(10)]:
        path = root / (prefix + 'lightdef.' + suffix)
        if not path.exists(): continue
        original = path.read_bytes(); in_section = False
        for line in decode(original).decode('cp1251').splitlines():
            row = line.split(';')[0].strip()
            if row.startswith('['):
                if in_section: break
                in_section = row.lower() == '[lightdefault]'; continue
            if not in_section or not row or row.startswith('#'): continue
            parts = [p.strip() for p in row.split(',') if p != '']
            index = int(parts[0]); require(0 <= index < 256, 'definition index outside native range')
            value = definitions.get(index, [0, 1., 1., 1., 500., 0, 0., 0, 'LD_NoNameEntered']).copy()
            for column, item in enumerate(parts[1:10]):
                value[column] = item.split()[0][:30] if column == 8 else float(item) if column in (1, 2, 3, 4, 6) else int(item)
            definitions[index] = value
        require(path.read_bytes() == original, 'definition source changed during audit')
        sources.append({'relative_path': str(path.relative_to(root)), 'sha256': hashlib.sha256(original).hexdigest()})
    maps = inspect_maps(root)
    references = [light['definition'] for m in maps['maps'] for light in m['links']]
    invalid = sorted(set(i for i in references if i not in definitions or definitions[i][0] == 0))
    return {'definition_sources': sources, 'definitions': definitions, 'saved_lights': len(references),
            'referenced_definitions': sorted(set(references)), 'invalid_definitions': invalid,
            'map_sources': [{'map': m['map'], 'source_sha256': m['source_sha256']} for m in maps['maps']],
            'limitations': ['loose DefaultPath files only; runtime VFS unobserved', 'no game execution or complete object-family transaction']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args(); root = args.game_root.resolve(); output = args.output.resolve()
    if output.exists() or output.is_relative_to(root) or output.is_relative_to(args.exe.resolve().parent):
        parser.error('output must be new and outside source directories')
    original = args.exe.read_bytes(); result = inspect_code(original); result.update(inspect_definitions(root))
    require(args.exe.read_bytes() == original, 'EXE changed during audit')
    with output.open('x', encoding='utf-8') as f: json.dump(result, f, indent=2); f.write('\n')
    print(json.dumps({'instructions': result['instruction_assertions'], 'definitions': len(result['definitions']),
                     'saved_lights': result['saved_lights'], 'invalid_definitions': result['invalid_definitions']}))


if __name__ == '__main__': main()
