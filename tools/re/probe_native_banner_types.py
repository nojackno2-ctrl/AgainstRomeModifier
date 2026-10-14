#!/usr/bin/env python3
"""Read-only native banner selection proof and saved engine table comparison."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import decode
from probe_pua import EXE_SHA256, require


def inspect_code(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = [(0x4c9d40, 0x5a), (0x46a9c0, 0x62), (0x468fd0, 0xc3),
               (0x4870e0, 0x680), (0x487f70, 0x1dfa), (0x48d6e0, 0x271),
               (0x4441d0, 0x17)]
    instructions = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    by_address = {i.address: i for i in instructions}
    checks = [(0x4c9d49, 'cmp', 'ebx, 8'), (0x4c9d55, 'call', '0x4afd80'),
              (0x4c9d5f, 'je', '0x4c9d4e'), (0x4c9d6c, 'mov', 'dword ptr [eax + 0x771804], ebx'),
              (0x4c9d92, 'mov', 'eax, dword ptr [edx*4 + 0x771804]'),
              (0x46a9ce, 'cmp', 'edx, 8'), (0x46a9d7, 'cmp', 'esi, 0x14'),
              (0x46a9e6, 'mov', 'edx, dword ptr [ecx + 0x73ca88]'),
              (0x46a9f8, 'mov', 'edx, dword ptr [ecx + 0x73ca8c]'),
              (0x46aa10, 'mov', 'eax, dword ptr [edx + esi*4 + 0x73b3f8]'),
              (0x468feb, 'call', '0x432620'), (0x468ffc, 'mov', 'dword ptr [eax*4 + 0x73ca88], edx'),
              (0x46907f, 'add', 'eax, 4'), (0x469089, 'call', '0x567320'),
              (0x48768b, 'call', '0x46a9c0'), (0x487696, 'call', '0x4c9d40'),
              (0x48773f, 'push', '-1'), (0x487741, 'push', '8'), (0x487743, 'call', '0x4c9d40'),
              (0x489d49, 'mov', 'ebp, dword ptr [ebx + 0x771804]'),
              (0x489d54, 'call', '0x414db0'), (0x489d5c, 'cmp', 'ebx, 0x24'),
              (0x48d8fe, 'push', '9'), (0x48d900, 'lea', 'eax, [edi + 0x1c]'),
              (0x48d905, 'call', '0x4152f0'), (0x4441d4, 'push', '0'),
              (0x4441de, 'call', '0x5675f0')]
    for a, mnemonic, operands in checks:
        i = by_address.get(a)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'unexpected instruction {a:#x}')
    require(read_va(exe, 0x5e00a9, 16) == b'GER\0KEL\0HUN\0ROM\0', 'unexpected tribe strings')
    require(read_va(exe, 0x5ee694, 5) == b'none\0', 'unexpected banner sentinel')
    require(struct.unpack('<5I', read_va(exe, 0x468fb0, 20)) ==
            (0x468fea, 0x469005, 0x469028, 0x46904b, 0x469070), 'unexpected team parser cases')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'engine_marker_table_offset': 54, 'entries': 9,
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def sections(text):
    result, key = {}, None
    for line in text.splitlines():
        line = line.split(';', 1)[0].strip()
        if line.startswith('['):
            key = line.lower()
            # Product/probe policy: do not silently merge ambiguous duplicate sections.
            require(key not in result, f'duplicate section {key}')
            result[key] = []
        elif line and key is not None:
            result[key].append(line)
    return result


def inspect_maps(root):
    source_hashes = {}
    def read(path):
        packed = path.read_bytes()
        source_hashes[path] = hashlib.sha256(packed).hexdigest()
        return decode(packed)
    engine_config = sections(read(root / 'SYSTEM/clengine.ini').decode('cp1252'))
    prefix = engine_config['[defaultpath]'][0].replace('\\', '/')
    require(not Path(prefix).is_absolute() and '..' not in Path(prefix).parts, 'unsafe definition prefix')
    names, active, definitions = {}, {}, {}
    for suffix in ['objdef.dau'] + [f'objdef.d{i:02}' for i in range(10)]:
        path = root / (prefix + suffix)
        if not path.exists(): continue
        for row in sections(read(path).decode('cp1252')).get('[objectdefaults]', []):
            columns = [v for v in row.split(',') if v]
            index = int(columns[0]); previous = definitions.setdefault(index, {})
            require(0 <= index < 2500, 'unsupported definition index')
            if len(columns) > 1: previous['active'] = int(columns[1]) != 0
            if len(columns) > 52: previous['name'] = columns[52].strip().lower()
    for index, definition in definitions.items():
        if definition.get('name'):
            names.setdefault(definition['name'], []).append(index)
        active[index] = definition.get('active', False)
    banner = sections(read(root / 'SYSTEM/banner.ini').decode('cp1252'))
    totals, maps = Counter(), []
    for path in sorted((root / 'MAPS').glob('*/DATA/engine.dat')):
        engine = read(path)
        require(len(engine) == 94 and struct.unpack_from('<I', engine)[0] == 1, 'unsupported engine layout')
        saved = struct.unpack_from('<9i', engine, 54)
        team_path = path.with_name('team.dat')
        rows = sections(read(team_path).decode('cp1252')).get('[teamdata]', []) if team_path.exists() else []
        teams = []
        for row in rows:
            columns = [v.strip() for v in row.split(',') if v]
            require(len(columns) >= 6, 'incomplete team row')
            team, version = int(columns[0]), int(columns[5])
            require(0 <= team < 8 and 0 <= version < 10, 'unsupported team/banner selection')
            tribe = {'GER': 0, 'KEL': 1, 'HUN': 2, 'ROM': 3}.get(columns[1].upper(), 0)
            candidates = banner.get(f'[volk{tribe:02}_obdef_bver{version:02}]', [])
            alias = candidates[0].lower() if candidates else 'none'
            matches = names.get(alias, [])
            require(len(matches) <= 1, 'ambiguous requested banner definition name')
            candidate = matches[0] if matches else -1
            accepted = candidate >= 0 and active.get(candidate, False)
            status = 'same' if accepted and saved[team] == candidate else 'different' if accepted else 'setter-no-op'
            totals[status] += 1
            teams.append({'team': team, 'tribe': tribe, 'banner_version': version, 'name': alias,
                          'candidate_type': candidate, 'saved_type': saved[team], 'status': status})
        if not rows: totals['missing-team-data'] += 1
        totals['maps'] += 1; totals['team8_' + str(saved[8])] += 1
        maps.append({'map': path.parent.parent.name, 'saved_types': saved, 'teams': teams})
    for path, digest in source_hashes.items():
        require(hashlib.sha256(path.read_bytes()).hexdigest() == digest, 'source changed during audit')
    return {'maps': maps, 'totals': dict(totals),
            'sources': [{'path': str(p.relative_to(root)), 'sha256': h} for p, h in source_hashes.items()],
            'limitations': ['loose-file definitions only; no runtime VFS observation',
                            'invalid/none candidate makes native setter a no-op; not an unconditional reset',
                            'missing team files are reported; no invented defaults or game execution',
                            'saved differences are observations, not corruption verdicts']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('game_root', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--exe', required=True, type=Path)
    args = parser.parse_args(); root = args.game_root.resolve(); output = args.output.resolve()
    require(not output.exists() and not output.is_relative_to(root)
            and not output.is_relative_to(args.exe.resolve().parent), 'output must be new and outside sources')
    exe = args.exe.read_bytes(); result = inspect_code(exe); result.update(inspect_maps(root))
    require(args.exe.read_bytes() == exe, 'EXE changed during audit')
    with output.open('x', encoding='utf-8') as f: json.dump(result, f, indent=2); f.write('\n')
    print(json.dumps({'instruction_assertions': result['instruction_assertions'], 'totals': result['totals']}))


if __name__ == '__main__': main()
