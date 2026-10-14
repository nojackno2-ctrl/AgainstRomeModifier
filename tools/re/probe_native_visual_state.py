#!/usr/bin/env python3
"""Fingerprint-checked static visual-state assertions and read-only map audit.

Requires Python 3.10+, Capstone, sibling probes and the retained eight-record
Ghidra evidence. Does not execute the game or infer visibility/tick behavior.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

sys.dont_write_bytecode = True

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_object_anim_links import COLUMN_WIDTHS, decode
from probe_pua import EXE_SHA256, require

WORKSPACE = Path(__file__).resolve().parents[2]
EVIDENCE = WORKSPACE / 're_workspace/native-child-render-contract-20261011-evidence.json'
# Offsets below are integers; report includes explicit hex to prevent ambiguity.
AXES_DISK = (13, 17, 15, 19)
AXES_RUNTIME = (0x12, 0x16, 0x14, 0x18)
ANCHORS = {
    0x4b53c0: [
        '0x4b5441: CMP EBX, -0x1', '0x4b5444: JNZ 0x004b5473',
        '0x4b5446: LEA EBX, [ESI*0x4 + 0x0]', '0x4b544d: ADD EBX, ESI',
        '0x4b544f: SHL EBX, 0x2', '0x4b5452: SUB EBX, ESI',
        '0x4b5454: MOV EBX, dword ptr [EBX*0x4 + 0xa14c0c]',
        '0x4b545b: SAR EBX, 0x10', '0x4b545e: CMP EBX, -0x1',
        '0x4b5461: JZ 0x004b5551', '0x4b5467: SHL EBX, 0x5',
        '0x4b546a: MOV EBX, dword ptr [EBX + 0xbb9a62]',
        '0x4b5470: SAR EBX, 0x10'],
    0x48c250: [
        '0x48c2b5: CALL 0x00414df0', '0x48c315: MOV byte ptr [ESI], AL',
        '0x48c31b: CALL 0x00414e90', '0x48c32b: MOV dword ptr [ESI + 0x4], EAX',
        '0x48c332: CALL 0x00414e40', '0x48c342: MOV word ptr [ESI + 0x8], AX',
        '0x48c34a: CALL 0x00414e90', '0x48c35e: MOV dword ptr [ESI + 0xc], EAX',
        '0x48c365: CALL 0x00414e40', '0x48c379: MOV word ptr [ESI + 0x10], AX',
        '0x48c381: CALL 0x00414e40', '0x48c395: MOV word ptr [ESI + 0x12], AX',
        '0x48c39d: CALL 0x00414e40', '0x48c3b1: MOV word ptr [ESI + 0x14], AX',
        '0x48c3b9: CALL 0x00414e40', '0x48c3cd: MOV word ptr [ESI + 0x16], AX',
        '0x48c3d5: CALL 0x00414e40', '0x48c2e0: MOV word ptr [ESI + 0x18], AX',
        '0x48c2e8: ADD ESI, 0x20'],
    0x4b4790: [
        '0x4b4a7e: IMUL EAX, EBX, 0x4c',
        '0x4b4a81: MOV EAX, dword ptr [EAX + 0xa14c0c]',
        '0x4b4a87: SAR EAX, 0x10', '0x4b4a8a: CMP EAX, -0x1',
        '0x4b4a8d: JZ 0x004b4ddb', '0x4b4a93: SHL EAX, 0x5',
        '0x4b4a96: MOV EDX, dword ptr [EAX + 0xbb9a70]',
        '0x4b4a9c: SAR EDX, 0x10',
        '0x4b4a9f: MOV EBP, dword ptr [EAX + 0xbb9a6c]',
        '0x4b4aa5: MOV dword ptr [ESP + 0x44], EDX',
        '0x4b4aa9: MOV EDX, dword ptr [EAX + 0xbb9a6e]',
        '0x4b4aaf: MOV EAX, dword ptr [EAX + 0xbb9a72]',
        '0x4b4ab5: SAR EAX, 0x10',
        '0x4b4ab8: MOV dword ptr [ESP + 0x40], EAX',
        '0x4b4ac1: SAR EDX, 0x10', '0x4b4ac5: SAR EBP, 0x10',
        '0x4b4ac8: MOV dword ptr [ESP + 0x44], EDX',
        '0x4b4b0d: MOV ECX, dword ptr [ESP + 0x48]', '0x4b4b11: PUSH ECX',
        '0x4b4b12: MOV EDI, dword ptr [ESP + 0x54]', '0x4b4b16: PUSH EDI',
        '0x4b4b17: MOV EAX, dword ptr [ESP + 0x54]', '0x4b4b1b: PUSH EAX',
        '0x4b4b1c: MOV EDX, dword ptr [ESP + 0x60]', '0x4b4b20: PUSH EDX',
        '0x4b4b21: PUSH EBP', '0x4b4b2a: CALL 0x004cbf70'],
    0x4cbf70: [
        '0x4cbf77: MOV EDI, dword ptr [ESP + 0x9c]',
        '0x4cbfd8: LEA EAX, [ESP + 0x8]', '0x4cbfdc: PUSH EAX',
        '0x4cbfdd: LEA EAX, [ESP + 0x8]', '0x4cbfe1: PUSH EAX',
        '0x4cbfe2: MOV EBP, dword ptr [ESP + 0xa8]', '0x4cbfe9: PUSH EBP',
        '0x4cbfea: PUSH EDI',
        '0x4cbfeb: MOV EAX, dword ptr [ESP + 0xa8]', '0x4cbff2: PUSH EAX',
        '0x4cbff3: MOV EDX, dword ptr [ESP + 0xa8]', '0x4cbffa: PUSH EDX',
        '0x4cbffb: MOV ECX, dword ptr [ESP + 0xa8]', '0x4cc002: PUSH ECX',
        '0x4cc003: CALL 0x004e6b80'],
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(assembly):
    """Normalize spelling only: Ghidra JNZ/JZ and hex versus decimal operands."""
    mnemonic, _, operands = assembly.lower().partition(' ')
    mnemonic = {'jnz': 'jne', 'jz': 'je'}.get(mnemonic, mnemonic)
    operands = re.sub(r'-?0x[0-9a-f]+', lambda m: str(int(m[0], 16)), operands)
    operands = re.sub(r'\s+', '', operands)
    operands = operands.replace('+0]', ']')
    return mnemonic, operands


def inspect_code(exe, retained):
    require(sha(exe) == EXE_SHA256, 'unsupported EXE fingerprint')
    records = json.loads(retained)['records']
    require(len(records) == 8, 'expected eight retained Ghidra records')
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    functions = []
    for entry, anchors in ANCHORS.items():
        matches = [r for r in records if isinstance(r.get('normalized_result'), dict)
                   and r['normalized_result'].get('procedure', {}).get('address') == hex(entry)
                   and r['normalized_result'].get('assembly')]
        require(len(matches) == 1, f'missing/ambiguous retained body {entry:#x}')
        record = matches[0]
        require(record['subject']['digest']['sha256'] == EXE_SHA256, 'evidence EXE mismatch')
        native = record['normalized_result']
        body = native['procedure']['body']
        require(body['available'] and body['contains_entry']
                and body['provenance'] == 'ghidra-function-body-address-set', 'unproven body')
        ranges = [(int(r['start'], 16), int(r['end'], 16)) for r in body['ranges']]
        # Use provider instruction starts within the complete provider body, not
        # a guessed linear region. Embedded data need not be decoded as code.
        starts = {int(line.split(':', 1)[0], 16) for line in native['assembly']}
        verified = []
        for line in anchors:
            require(line in native['assembly'], f'retained assembly changed: {line}')
            address_text, expected = line.split(': ', 1)
            address = int(address_text, 16)
            containing = [(a, b) for a, b in ranges if a <= address <= b]
            require(address in starts and len(containing) == 1, 'anchor outside Ghidra body')
            available = min(15, containing[0][1] - address + 1)
            instruction = next(decoder.disasm(read_va(exe, address, available), address, count=1), None)
            require(instruction is not None and canonical(expected) ==
                    canonical(instruction.mnemonic + ' ' + instruction.op_str),
                    f'EXE instruction differs from retained anchor {address_text}')
            verified.append({'address': address_text, 'bytes': instruction.bytes.hex(),
                             'assembly': expected,
                             'capstone': instruction.mnemonic + ' ' + instruction.op_str})
        functions.append({'entry': hex(entry), 'evidence_id': record['evidence_id'],
                          'body_ranges_inclusive': body['ranges'], 'anchors': verified})
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': sum(len(f['anchors']) for f in functions),
            'functions': functions,
            'mapping': {'objects_animidx_disk': 71, 'objects_animidx_runtime': '0x12',
                        'anim_alrframe_disk': 5, 'anim_alrframe_runtime': '0x08',
                        'anim_disk_stride': 21, 'anim_runtime_stride': 32,
                        'apt_axes_disk': list(AXES_DISK),
                        'apt_axes_disk_hex': [hex(o) for o in AXES_DISK],
                        'apt_axes_runtime_hex': [hex(o) for o in AXES_RUNTIME],
                        'signed_fields': ['animidx', 'ALRFrame', 'APT axes'],
                        'disk_to_runtime_hex': {'0x00': '0x00', '0x01': '0x04',
                            '0x05': '0x08', '0x07': '0x0c', '0x0b': '0x10',
                            '0x0d': '0x12', '0x0f': '0x14', '0x11': '0x16', '0x13': '0x18'},
                        'apt_argument_order': 'axis0, axis1, axis2, axis3 (cdecl right-to-left pushes)',
                        'frame_fallback': 'argument -1 -> object animidx -> signed ALRFrame'}}


def inspect_pair(objects_source, anim_source):
    objects, anim = decode(objects_source), decode(anim_source)
    require(len(objects) >= 16 and len(anim) >= 8, 'short pool header')
    version, count, n0, n1 = struct.unpack_from('<4I', objects)
    aversion, acount = struct.unpack_from('<2I', anim)
    require(version == aversion == 1 and n0 == n1 == 30 and count <= 14000
            and acount <= 14000, 'unsupported pool layout')
    require(len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'objects length mismatch')
    require(len(anim) == 8 + acount * 25, 'anim length mismatch')
    parent_offset = 16 + count * (79 + sum(COLUMN_WIDTHS[:13]))
    counts, frames, axes, states = Counter(), set(), [set() for _ in AXES_DISK], []

    def state(slot):
        offset = 16 + slot * 79
        active = objects[offset] != 0
        animidx = struct.unpack_from('<h', objects, offset + 71)[0]
        result = {'slot': slot, 'active': active,
                  'uid': struct.unpack_from('<I', objects, offset + 3)[0],
                  'team': struct.unpack_from('<H', objects, offset + 1)[0],
                  'type': struct.unpack_from('<H', objects, offset + 75)[0], 'animidx': animidx}
        if animidx == -1:
            result['anim_link'] = 'missing'
        elif not 0 <= animidx < acount:
            result['anim_link'] = 'invalid'
        else:
            start = 8 + animidx * 21
            result['anim_active'] = anim[start] != 0
            result['anim_link'] = 'active' if result['anim_active'] else 'inactive'
            result['ALRFrame'] = struct.unpack_from('<h', anim, start + 5)[0]
            result['APT_axes'] = [struct.unpack_from('<h', anim, start + o)[0] for o in AXES_DISK]
        return result

    # All populated records and all declared child edges are retained, including
    # inactive children. Stale links are observations, not visibility decisions.
    for slot in range(count):
        child = state(slot)
        parent = struct.unpack_from('<H', objects, parent_offset + slot * 2)[0]
        if not child['active'] and parent == 0xffff:
            continue
        counts['object_states'] += 1
        counts['active_objects' if child['active'] else 'inactive_objects'] += 1
        counts['animlinks_' + child['anim_link']] += 1
        if 'ALRFrame' in child:
            frames.add(child['ALRFrame'])
            counts['negative_ALRFrame'] += child['ALRFrame'] < 0
            for i, value in enumerate(child['APT_axes']):
                axes[i].add(value)
                counts[f'negative_axis{i}'] += value < 0
        child['parent_slot'] = None if parent == 0xffff else parent
        if parent == 0xffff:
            child['parent_link'] = 'root'
        else:
            counts['children'] += 1
            status = 'invalid' if parent >= count else 'self' if parent == slot else (
                'active' if objects[16 + parent * 79] else 'inactive')
            child['parent_link'] = status
            counts['parentlinks_' + status] += 1
            if parent < count:
                child['parent_state'] = state(parent)
        states.append(child)
    return {'object_slots': count, 'anim_slots': acount,
            'active_anim_slots': sum(anim[8 + i * 21] != 0 for i in range(acount)),
            'decoded_sha256': {'objects': sha(objects), 'anim': sha(anim)},
            'pfil_wrapped': {'objects': objects_source.startswith(b'PFIL@'),
                             'anim': anim_source.startswith(b'PFIL@')},
            'counts': dict(counts), 'distinct_ALRFrame': sorted(frames),
            'distinct_APT_axes': [sorted(values) for values in axes],
            'distinct_counts': {'ALRFrame': len(frames), 'APT_axes': [len(values) for values in axes]},
            'root_states': [s for s in states if s['parent_slot'] is None],
            'all_child_states': [s for s in states if s['parent_slot'] is not None]}


def scan(root, sources):
    require((root / 'MAPS').is_dir(), 'missing MAPS directory')
    reports, missing, errors, totals = [], [], [], Counter()
    frames, axes = set(), [set() for _ in AXES_DISK]
    directories = sorted(p for p in (root / 'MAPS').iterdir() if p.is_dir())
    for directory in directories:
        paths = [directory / 'DATA' / name for name in ('objects.dat', 'anim.dat')]
        require(directory.resolve().is_relative_to(root), 'map directory escapes source root')
        absent = [p.name for p in paths if not p.is_file()]
        if absent:
            for path in paths:
                if path.is_file():
                    require(path.resolve().is_relative_to(root), 'pool path escapes source root')
                    read_source(path, sources)
            missing.append({'map': directory.name, 'missing': absent})
            continue
        for path in paths:
            require(path.resolve().is_relative_to(root), 'pool path escapes source root')
        packed = [read_source(path, sources) for path in paths]
        try:
            report = inspect_pair(*packed)
            totals.update(report['counts'])
            frames.update(report['distinct_ALRFrame'])
            for values, observed in zip(axes, report['distinct_APT_axes']):
                values.update(observed)
            reports.append({'map': directory.name, **report})
        except (ValueError, struct.error) as exc:
            errors.append({'map': directory.name, 'error': str(exc)})
    return {'map_directories': len(directories), 'expected_baseline_maps': 74,
            'matches_74_map_baseline': len(directories) == 74,
            'maps': reports, 'missing_pool_pairs': missing, 'errors': errors, 'totals': dict(totals),
            'distinct_ALRFrame': sorted(frames), 'distinct_APT_axes': [sorted(v) for v in axes],
            'distinct_counts': {'ALRFrame': len(frames), 'APT_axes': [len(v) for v in axes]}}


def read_source(path, sources):
    data = path.read_bytes()
    sources[path.resolve()] = sha(data)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--game-root', type=Path)
    args = parser.parse_args()
    exe, output = args.exe.resolve(), args.output.resolve()
    root = args.game_root.resolve() if args.game_root else None
    installed = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    evidence_directory = EVIDENCE.parent.resolve()
    require(not output.exists() and not args.output.is_symlink(), 'output must be a new explicit path')
    require(not output.is_relative_to(installed) and (root is None or not output.is_relative_to(root)),
            'output must be outside source game and installed directories')
    require(not output.is_relative_to(exe.parent) or output.is_relative_to(evidence_directory),
            'output must be outside EXE directory (repository re_workspace is allowed)')
    require(output.parent.is_dir(), 'output parent must already exist')
    sources = {}
    result = inspect_code(read_source(exe, sources), read_source(EVIDENCE, sources))
    result['evidence_path'] = str(EVIDENCE)
    if root:
        result['map_audit'] = scan(root, sources)
    result['source_hashes'] = []
    for path, before in sources.items():
        after = sha(path.read_bytes())
        require(before == after, f'source changed during audit: {path}')
        result['source_hashes'].append({'path': str(path), 'before': before, 'after': after,
                                      'unchanged': True})
    result['limitations'] = [
        'Static EXE assertions and disk snapshots only; no game or runtime execution.',
        'No matrix semantics, parent visibility inheritance, culling, sorting or tick advancement claimed.',
        'animidx is objects disk +71 signed16, never the object slot; frame and axes are signed16.',
        'UID is disk u32 +3 and team u16 +1; parent column13 is a disk link only.',
        'Pool capacity/version/length validation is stricter than the native reader.',
        'PFIL decoded to declared size; wrapper/trailing compressed integrity not established.',
        'Negative frame/axes and inactive/shared links are observations, not corruption diagnoses.',
        'Hash equality checks endpoints only; concurrent transient changes cannot be excluded.']
    with output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, separators=(',', ':'))
        stream.write('\n')
    audit = result.get('map_audit', {})
    print(json.dumps({'output': str(output), 'instruction_assertions': result['instruction_assertions'],
                      'maps': len(audit.get('maps', [])), 'errors': len(audit.get('errors', [])),
                      'missing_pool_pairs': len(audit.get('missing_pool_pairs', [])),
                      'totals': audit.get('totals', {}), 'sources_unchanged': len(sources)}))
    return 1 if audit.get('errors') or audit.get('missing_pool_pairs') else 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, TypeError, struct.error) as exc:
        print(json.dumps({'error': str(exc)}), file=sys.stderr)
        raise SystemExit(1)
