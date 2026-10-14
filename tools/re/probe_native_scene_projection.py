#!/usr/bin/env python3
"""Read-only, fingerprinted native XYZ projection and optional 74-map audit.

Usage: python tools/re/probe_native_scene_projection.py EXE OUTPUT [--game-root ROOT]
Requires Python 3.10+, Capstone and the sibling probes. Only OUTPUT is created,
with exclusive creation; its parent must exist outside all input/game directories.
No game execution, REA invocation, extraction, installation or runtime FPU claim.
"""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import sys

# Imports must never create __pycache__ beside inputs or sibling probes.
sys.dont_write_bytecode = True

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import X86_OP_MEM
from probe_engine_config import read_va
from probe_native_float_conversion import conditional_conversion
from probe_object_anim_links import COLUMN_WIDTHS, decode
from probe_pua import EXE_SHA256, require

WORKSPACE = Path(__file__).resolve().parents[2]
EVIDENCE = WORKSPACE / 're_workspace/native-family-height-20261011-evidence.json'
EVIDENCE_SHA256 = '3caf2767783718d16f8a19e4c93119a0e9034e81a7df4fe5265d056557b62e5e'
EXPECTED_EXE = '6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf'
CONTROLS = (0x037f, 0x007f, 0x027f, 0x077f, 0x0b7f, 0x0f7f)

# This whole contiguous caller slice proves the packed index, stride, stack
# accounting, three separate conversions, and five projection arguments.
CALLER = """0x4b48ad: IMUL EDI, EBX, 0x4c
0x4b48b0: MOV EDI, dword ptr [EDI + 0xa14c06]
0x4b48b6: SAR EDI, 0x10
0x4b48b9: CMP EDI, -0x1
0x4b48bc: JLE 0x004b47d6
0x4b48c2: LEA EAX, [ESP + 0x8]
0x4b48c6: PUSH EAX
0x4b48c7: LEA EAX, [ESP + 0x8]
0x4b48cb: PUSH EAX
0x4b48cc: IMUL EAX, EDI, 0x14
0x4b48cf: PUSH dword ptr [EAX + 0xb18848]
0x4b48d5: MOV dword ptr [ESP + 0x1c], EAX
0x4b48d9: CALL 0x00419c80
0x4b48de: ADD ESP, 0x4
0x4b48e1: PUSH EAX
0x4b48e2: MOV EAX, dword ptr [ESP + 0x1c]
0x4b48e6: PUSH dword ptr [EAX + 0xb18844]
0x4b48ec: CALL 0x00419c80
0x4b48f1: ADD ESP, 0x4
0x4b48f4: PUSH EAX
0x4b48f5: MOV EAX, dword ptr [ESP + 0x20]
0x4b48f9: PUSH dword ptr [EAX + 0xb18840]
0x4b48ff: CALL 0x00419c80
0x4b4904: ADD ESP, 0x4
0x4b4907: PUSH EAX
0x4b4908: CALL 0x00491b50
0x4b490d: IMUL EAX, EBP, 0x12
0x4b4910: MOV EAX, dword ptr [EAX + 0xc2706a]
0x4b4916: ADD ESP, 0x14""".splitlines()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(assembly):
    """Normalize spelling only, retaining register, width and operand order."""
    mnemonic, _, operands = assembly.lower().strip().partition(' ')
    mnemonic = {'jnz': 'jne', 'jz': 'je', 'sal': 'shl'}.get(mnemonic, mnemonic)
    operands = operands.replace('float ptr', 'dword ptr')
    # Ghidra omits the width only on MOV accumulator/absolute-memory forms.
    if mnemonic == 'mov':
        operands = re.sub(r'(?<!ptr )\[(0x[0-9a-f]+)\]', r'dword ptr [\1]', operands)
    operands = re.sub(r'-?0x[0-9a-f]+', lambda m: str(int(m[0], 16)), operands)
    operands = re.sub(r'\s+', '', operands).replace('+0]', ']')
    return mnemonic, operands


def verify_lines(exe, lines, ranges):
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    decoder.detail = True
    verified, index = [], {}
    for line in lines:
        address_text, expected = line.split(': ', 1)
        address = int(address_text, 16)
        containing = [(a, b) for a, b in ranges if a <= address <= b]
        require(len(containing) == 1, f'instruction outside retained body: {line}')
        available = min(15, containing[0][1] - address + 1)
        instruction = next(decoder.disasm(read_va(exe, address, available), address, count=1), None)
        require(instruction is not None and canonical(expected) ==
                canonical(instruction.mnemonic + ' ' + instruction.op_str),
                f'original EXE/retained assembly mismatch at {address_text}')
        require(address not in index, 'duplicate retained instruction address')
        index[address] = instruction
        verified.append({'address': hex(address), 'bytes': instruction.bytes.hex(),
                         'retained': expected, 'capstone': instruction.mnemonic + ' ' + instruction.op_str})
    return verified, index


def contiguous(index, start, end):
    cursor = start
    for address, instruction in sorted(index.items()):
        require(address == cursor, f'missing/overlapping instruction at {cursor:#x}')
        cursor += instruction.size
    require(cursor == end + 1, 'instruction coverage does not match inclusive body')


def caller_stack(index):
    """Replay only the verified straight-line argument preparation, not code execution."""
    stack, esp, eax, calls = {}, 0, None, []
    axes = {0xb18840: 'X', 0xb18844: 'Y', 0xb18848: 'Z'}
    for address, instruction in sorted(index.items()):
        if not 0x4b48c2 <= address <= 0x4b4908:
            continue
        operands = instruction.operands
        mnemonic = instruction.mnemonic
        if mnemonic == 'lea':
            eax = ('pointer_to_original_local', esp + operands[1].mem.disp)
        elif mnemonic == 'imul':
            eax = ('primary_position_byte_offset', operands[2].imm)
        elif mnemonic == 'push':
            if operands[0].type == X86_OP_MEM:
                require(eax == ('primary_position_byte_offset', 20), 'float source lost position offset')
                value = ('float32', axes[operands[0].mem.disp])
            else:
                value = eax
            esp -= 4
            stack[esp] = value
        elif mnemonic == 'mov':
            if operands[0].type == X86_OP_MEM:
                stack[esp + operands[0].mem.disp] = eax
            else:
                eax = stack[esp + operands[1].mem.disp]
        elif mnemonic == 'add':
            esp += operands[1].imm
        elif mnemonic == 'call':
            target = operands[0].imm
            if target == 0x419c80:
                value = stack[esp]
                require(value[0] == 'float32', 'conversion input is not an individual float')
                calls.append({'site': hex(address), 'float_axis': value[1]})
                eax = ('converted_signed32', value[1])
            else:
                require(target == 0x491b50, 'unexpected argument-preparation call')
                arguments = [stack[esp + i * 4] for i in range(5)]
                require(arguments == [('converted_signed32', 'X'), ('converted_signed32', 'Y'),
                                      ('converted_signed32', 'Z'), ('pointer_to_original_local', 4),
                                      ('pointer_to_original_local', 8)], 'actual caller stack order mismatch')
                require([c['float_axis'] for c in calls] == ['Z', 'Y', 'X'] and esp == -20,
                        'conversion order or stack balance mismatch')
                return {'provenance': 'symbolic replay of exact Capstone-verified caller slice',
                        'conversion_calls': calls, 'cdecl_arguments_at_call': arguments,
                        'push_order': list(reversed(arguments)), 'ESP_delta_before_CALL': esp}
        else:
            raise ValueError(f'unhandled caller preparation instruction {address:#x}')
    raise ValueError('missing projection call in caller stack replay')


def inspect_code(exe, retained):
    require(EXE_SHA256 == EXPECTED_EXE and sha(exe) == EXPECTED_EXE, 'unsupported EXE fingerprint')
    require(sha(retained) == EVIDENCE_SHA256, 'retained ten-record bundle fingerprint changed')
    records = json.loads(retained)['records']
    require(len(records) == 10, 'expected latest ten-record bundle')
    functions, indexes = [], {}
    for entry, end in ((0x491b50, 0x491bc1), (0x419c80, 0x419cb6), (0x4b4790, 0x4b50b4)):
        matches = [r for r in records if r['operation'] == 'analyze_function'
                   and r.get('normalized_result', {}).get('procedure', {}).get('address') == hex(entry)]
        require(len(matches) == 1, f'missing/ambiguous retained procedure {entry:#x}')
        record = matches[0]
        require(record['subject']['digest']['sha256'] == EXPECTED_EXE, 'retained subject EXE mismatch')
        native = record['normalized_result']
        body = native['procedure']['body']
        ranges = [(int(r['start'], 16), int(r['end'], 16)) for r in body['ranges']]
        require(body['available'] and body['contains_entry'] and not body['non_contiguous']
                and body['provenance'] == 'ghidra-function-body-address-set'
                and ranges == [(entry, end)] and body['total_bytes'] == end - entry + 1,
                'retained inclusive body/provenance mismatch')
        lines = native['assembly']
        if entry == 0x4b4790:
            require(all(line in lines for line in CALLER), 'caller stack/index contract changed')
            lines = CALLER
        verified, index = verify_lines(exe, lines, ranges)
        contiguous(index, 0x4b48ad if entry == 0x4b4790 else entry,
                   0x4b4918 if entry == 0x4b4790 else end)
        functions.append({'entry': hex(entry), 'evidence_id': record['evidence_id'],
                          'body_ranges_inclusive': body['ranges'], 'body_bytes': body['total_bytes'],
                          'coverage': 'caller slice only' if entry == 0x4b4790 else 'complete body',
                          'instructions': verified})
        indexes[entry] = index
    require(read_va(exe, 0x612c70, 4).hex() == 'fdffff3e', 'signed near-half conversion bias changed')

    # Independently verify the original object validator. Its active-byte base
    # avoids inventing a relative offset from the caller's unaligned load.
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    decoder.detail = True
    validator = next(decoder.disasm(read_va(exe, 0x4ab7ce, 15), 0x4ab7ce, count=1), None)
    require(validator is not None and (validator.mnemonic, validator.op_str) ==
            ('cmp', 'byte ptr [eax*4 + 0xa14bfc], 0'), 'original object validator base changed')
    object_base = validator.operands[0].mem.disp
    caller = indexes[0x4b4790]
    packed_load = caller[0x4b48b0].operands[1]
    require(packed_load.type == X86_OP_MEM and packed_load.size == 4
            and caller[0x4b48b6].operands[1].imm == 16, 'not a signed packed high-half index')
    packed_address = packed_load.mem.disp
    position_offset = packed_address + 2 - object_base
    require(position_offset == 0x0c, 'unexpected derived primary-position offset')
    xyz_addresses = [caller[a].operands[0].mem.disp for a in (0x4b48f9, 0x4b48e6, 0x4b48cf)]
    require(xyz_addresses == [0xb18840, 0xb18844, 0xb18848], 'caller XYZ source changed')
    position_base = xyz_addresses[0] - 4
    require(position_base == 0xb1883c and caller[0x4b48cc].operands[2].imm == 20,
            'position runtime base/stride changed')
    return {
        'exe_sha256': EXPECTED_EXE, 'evidence_sha256': EVIDENCE_SHA256,
        'instruction_assertions': sum(len(f['instructions']) for f in functions) + 1,
        'functions': functions,
        'caller_stack_validation': caller_stack(caller),
        'object_base_validator': {'address': hex(validator.address), 'bytes': validator.bytes.hex(),
                                  'assembly': validator.mnemonic + ' ' + validator.op_str,
                                  'provenance': 'original EXE Capstone; separate from retained bodies'},
        'mapping': {'object_runtime_base': hex(object_base), 'object_runtime_stride': 76,
                    'packed_primary_load': hex(packed_address), 'packed_high_half_byte_delta': 2,
                    'derived_primary_position_runtime_offset': hex(position_offset),
                    'derivation': f'{packed_address:#x} + 2 - {object_base:#x} = {position_offset:#x}',
                    'object_primary_position_disk_offset': 67, 'object_disk_stride': 79,
                    'parent_link_disk_column_zero_based': 13,
                    'position_runtime_base': hex(position_base), 'position_runtime_stride': 20,
                    'XYZ_runtime_addresses': [hex(a) for a in xyz_addresses],
                    'XYZ_runtime_offsets': [4, 8, 12], 'position_disk_stride': 17,
                    'XYZ_disk_offsets': [1, 5, 9]},
        'call_chain': [
            {'caller': '0x4b4790', 'site': '0x4b48d9', 'callee': '0x419c80', 'input': 'primary Z float32'},
            {'caller': '0x4b4790', 'site': '0x4b48ec', 'callee': '0x419c80', 'input': 'primary Y float32'},
            {'caller': '0x4b4790', 'site': '0x4b48ff', 'callee': '0x419c80', 'input': 'primary X float32'},
            {'caller': '0x4b4790', 'site': '0x4b4908', 'callee': '0x491b50',
             'arguments_cdecl': ['converted X', 'converted Y', 'converted Z', 'screenX pointer', 'screenY pointer'],
             'push_order': ['screenY pointer (original ESP+8)', 'screenX pointer (original ESP+4)',
                            'converted Z', 'converted Y', 'converted X'],
             'cleanup': '0x4b4916 ADD ESP,0x14; each conversion separately ADD ESP,4'}],
        'projection': {'globals': {'zoom': '0x771854', 'cameraX': '0x771858', 'cameraY': '0x77185c'},
                       'screenX': 'SAR32(wrap32(X-Z), zoom) - cameraX, wrapped signed32',
                       'screenY': 'SAR32(wrap32(X+Z), wrap32(zoom+1)) - cameraY - heightTerm, wrapped signed32',
                       'heightTerm': 'd=wrap32(zoom-1); d>=0 ? SAR32(Y,d) : SHL32(Y,wrap32(-d))',
                       'normal_zoom_height': 'zoom>=1: Y SAR (zoom-1); zoom=0: Y SHL 1',
                       'worldY_used_static': True,
                       'Y_reads': ['0x491b8e', '0x491baa'], 'Y_subtractions': ['0x491b94', '0x491bb0'],
                       'shift_count': 'x86 CL masked with 31; arithmetic inputs/results signed32 wrap'},
        'float_conversion': {'function': '0x419c80', 'bias_bits': '3efffffd',
                             'operation': 'FSUB signed near-half bias then FISTP',
                             'model': 'sibling probe_native_float_conversion.conditional_conversion',
                             'gameplay_fpu_observed': False}}


def signed32(value):
    return ((value + (1 << 31)) & 0xffffffff) - (1 << 31)


def sar32(value, count):
    return signed32(value) >> (count & 31)


def shl32(value, count):
    return signed32(signed32(value) << (count & 31))


def project(x, y, z, zoom, camera_x=0, camera_y=0):
    x, y, z, zoom, camera_x, camera_y = map(signed32, (x, y, z, zoom, camera_x, camera_y))
    sx = signed32(sar32(signed32(x - z), zoom) - camera_x)
    sy = signed32(sar32(signed32(x + z), signed32(zoom + 1)) - camera_y)
    decrement = signed32(zoom - 1)
    height = sar32(y, decrement) if decrement >= 0 else shl32(y, signed32(-decrement))
    return {'screenX': sx, 'screenY': signed32(sy - height), 'height_term': height,
            'height_branch': 'SAR' if decrement >= 0 else 'SHL',
            'masked_counts': {'X': zoom & 31, 'XZ_sum': (zoom + 1) & 31,
                              'Y': (decrement if decrement >= 0 else -decrement) & 31}}


def model_cases():
    # Explicit expected outputs anchor basic/negative/wrapped boundaries.
    fixed = [
        ('negative-Y zoom0', (10, -3, 2, 0, 0, 0), (8, 12)),
        ('negative-Y zoom1', (10, -3, 2, 1, 0, 0), (4, 6)),
        ('negative-Y zoom2', (10, -3, 2, 2, 0, 0), (2, 3)),
        ('negative XYZ zoom0', (-10, -3, -2, 0, 0, 0), (-8, 0)),
        ('negative XYZ zoom1', (-10, -3, -2, 1, 0, 0), (-4, 0)),
        ('negative XYZ zoom2', (-10, -3, -2, 2, 0, 0), (-2, 0)),
        ('sum overflow', (2147483647, 0, 1, 0, 0, 0), (2147483646, -1073741824)),
        ('difference overflow', (-2147483648, 0, 1, 0, 0, 0), (2147483647, -1073741824)),
        ('camera subtraction wrap', (0, 0, 0, 0, -2147483648, 2147483647), (-2147483648, -2147483647)),
        ('height SHL overflow', (0, 1073741824, 0, 0, 0, 0), (0, -2147483648)),
        ('shift mask zoom32', (10, -3, 2, 32, 0, 0), (8, 7)),
        ('shift mask zoom33', (10, -3, 2, 33, 0, 0), (4, 6)),
        ('negative zoom SHL branch', (10, -3, 2, -1, 0, 0), (0, 24)),
        ('DEC INT_MIN wraps to positive', (10, -3, 2, -2147483648, 0, 0), (8, 7)),
    ]
    cases = []
    for label, args, expected in fixed:
        result = project(*args)
        require((result['screenX'], result['screenY']) == expected, f'functional boundary mismatch: {label}')
        cases.append({'case': label, 'input_X_Y_Z_zoom_cameraX_cameraY': args,
                      'expected_screenX_screenY': expected, 'result': result})
    for zoom in (-2147483648, -33, -32, -1, 0, 1, 2, 31, 32, 33, 63, 2147483647):
        for xyz in ((-2147483648, -2147483648, 2147483647), (2147483647, 2147483647, -2147483648)):
            args = (*xyz, zoom, 2147483647, -2147483648)
            cases.append({'case': 'signed32 extrema and masked shifts',
                          'input_X_Y_Z_zoom_cameraX_cameraY': args, 'result': project(*args)})
    bits = (0, 0x80000000, 0x3f000000, 0xbf000000, 0x3f7fffff, 0xbf7fffff,
            0x3f800000, 0xbf800000, 0x4effffff, 0x4f000000, 0xcf000000)
    conversions = [{'float32_bits': f'{b:08x}',
                    'conditional_controls': {f'{cw:04x}': conditional_conversion(b, cw) for cw in CONTROLS}}
                   for b in bits]
    return {'integer_projection_cases': cases, 'conditional_float_cases': conversions,
            'controls_are_hypothetical': True, 'runtime_rounding_mode_observed': False,
            'overflow_control_note': 'DEC zoom wraps before signed branch, including INT_MIN; not a zoom-range claim'}


def inspect_pair(objects_source, positions_source):
    objects, positions = decode(objects_source), decode(positions_source)
    require(len(objects) >= 16 and len(positions) >= 8, 'short pool header')
    version, count, n0, n1 = struct.unpack_from('<4I', objects)
    pversion, pcount = struct.unpack_from('<2I', positions)
    require(version == pversion == 1 and n0 == n1 == 30 and count <= 14000
            and pcount <= 33000, 'unsupported objects/position layout')
    require(len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'objects length mismatch')
    require(len(positions) == 8 + pcount * 17, 'position length mismatch')
    parent_offset = 16 + count * (79 + sum(COLUMN_WIDTHS[:13]))
    counts = Counter()
    children, unequal = [], []

    def state(slot):
        record = 16 + slot * 79
        position = struct.unpack_from('<h', objects, record + 67)[0]
        result = {'slot': slot, 'active': objects[record] != 0,
                  'uid': struct.unpack_from('<I', objects, record + 3)[0],
                  'type': struct.unpack_from('<H', objects, record + 75)[0],
                  'primary_position_index_signed16': position}
        if not 0 <= position < pcount:
            result['position_status'] = 'missing' if position == -1 else 'out_of_range'
            return result
        start = 8 + position * 17
        xyz_bits = struct.unpack_from('<3I', positions, start + 1)
        xyz = struct.unpack_from('<3f', positions, start + 1)
        result['position_status'] = 'active' if positions[start] else 'inactive'
        result['XYZ_float32_bits'] = [f'{b:08x}' for b in xyz_bits]
        result['XYZ'] = [v if math.isfinite(v) else None for v in xyz]
        result['finite_XYZ'] = all(math.isfinite(v) for v in xyz)
        result['conditional_XYZ_integer'] = {
            f'{cw:04x}': [conditional_conversion(b, cw) if b & 0x7f800000 != 0x7f800000
                         else {'integer': None, 'nonfinite_input': True} for b in xyz_bits]
            for cw in CONTROLS}
        return result

    for slot in range(count):
        active = objects[16 + slot * 79] != 0
        counts['active_objects' if active else 'inactive_objects'] += 1
        parent_slot = struct.unpack_from('<H', objects, parent_offset + slot * 2)[0]
        if parent_slot == 0xffff:
            continue
        counts['declared_children'] += 1
        child = state(slot)
        parent = state(parent_slot) if parent_slot < count else None
        status = ('out_of_range' if parent is None else 'self_link' if parent_slot == slot
                  else 'inactive_parent' if not parent['active'] else 'active_parent')
        counts['parent_' + status] += 1
        # "valid" is an audit predicate, not proof of native render eligibility.
        valid = (active and status == 'active_parent' and child['position_status'] == 'active'
                 and parent['position_status'] == 'active' and child['finite_XYZ'] and parent['finite_XYZ'])
        edge = {'child': child, 'parent_slot': parent_slot, 'parent_status': status,
                'parent': parent, 'valid_for_height_comparison': valid}
        if valid:
            counts['valid_children'] += 1
            dy = child['XYZ'][1] - parent['XYZ'][1]
            edge['child_minus_parent_Y'] = dy
            edge['unequal_Y_numeric'] = dy != 0
            edge['unequal_Y_bits'] = child['XYZ_float32_bits'][1] != parent['XYZ_float32_bits'][1]
            counts['valid_children_unequal_Y' if dy != 0 else 'valid_children_equal_Y'] += 1
            counts['valid_children_negative_Y'] += child['XYZ'][1] < 0
            if dy != 0:
                unequal.append(edge)
        else:
            counts['invalid_for_height_comparison'] += 1
        children.append(edge)
    return {'object_slots': count, 'position_slots': pcount, 'counts': dict(counts),
            'decoded_sha256': {'objects': sha(objects), 'position': sha(positions)},
            'PFIL_wrapped': {'objects': objects_source.startswith(b'PFIL@'),
                             'position': positions_source.startswith(b'PFIL@')},
            'all_child_records': children,
            'representatives': {'first_child': children[0] if children else None,
                                'first_valid_child': next((c for c in children if c['valid_for_height_comparison']), None),
                                'first_unequal_Y_child': unequal[0] if unequal else None},
            'valid_definition': 'distinct in-range parent, both objects active, both primary positions active/in-range, finite XYZ'}


def read_source(path, sources):
    path = path.resolve(strict=True)
    data = path.read_bytes()
    digest = sha(data)
    require(path not in sources or sources[path] == digest, f'source changed between reads: {path}')
    sources[path] = digest
    return data


def scan(root, sources):
    maps_root = (root / 'MAPS').resolve(strict=True)
    require(maps_root.is_dir() and maps_root.is_relative_to(root), 'missing/escaping MAPS directory')
    directories = sorted((p for p in maps_root.iterdir() if p.is_dir()), key=lambda p: p.name)
    reports, missing, errors, totals = [], [], [], Counter()
    for directory in directories:
        require(directory.resolve().is_relative_to(root), 'map directory escapes source root')
        paths = [directory / 'DATA' / name for name in ('objects.dat', 'position.dat')]
        packed, absent = [], []
        for path in paths:
            require(path.resolve().is_relative_to(root), 'pool path escapes source root')
            if path.is_file():
                packed.append(read_source(path, sources))
            else:
                absent.append(path.name)
        if absent:
            missing.append({'map': directory.name, 'missing': absent})
            continue
        try:
            report = inspect_pair(*packed)
            reports.append({'map': directory.name, **report})
            totals.update(report['counts'])
        except (ValueError, struct.error) as exc:
            errors.append({'map': directory.name, 'error': str(exc)})
    return {'expected_native_map_count': 74, 'map_directories_discovered': len(directories),
            'all_74_maps_audited': len(directories) == len(reports) == 74 and not missing and not errors,
            'maps': reports, 'missing_pool_pairs': missing, 'errors': errors, 'totals': dict(totals),
            'originality_verified': False,
            'originality_note': 'All supplied MAPS directories audited; no ISO/baseline comparison. Installed files may have prior edits.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--game-root', type=Path)
    args = parser.parse_args()
    exe = args.exe.resolve(strict=True)
    evidence = EVIDENCE.resolve(strict=True)
    root = args.game_root.resolve(strict=True) if args.game_root else None
    output = args.output.resolve()
    installed = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    forbidden = [exe.parent, evidence.parent, installed]
    if root is not None:
        forbidden.append(root)
    require(not args.output.is_symlink() and not output.exists(), 'output must be a new explicit path')
    require(output.parent.is_dir(), 'output parent must already exist')
    require(all(not output.is_relative_to(p) for p in forbidden),
            'output must be outside EXE/evidence input directories and all game directories')
    sources = {}
    result = inspect_code(read_source(exe, sources), read_source(evidence, sources))
    result['evidence_path'] = str(evidence)
    result['functional_models'] = model_cases()
    if root is not None:
        result['map_audit'] = scan(root, sources)
    result['source_hashes'] = []
    for path, before in sources.items():
        after = sha(path.read_bytes())
        require(before == after, f'source changed during probe: {path}')
        result['source_hashes'].append({'path': str(path), 'before': before, 'after': after, 'unchanged': True})
    result['limitations'] = [
        'Static instruction and disk observations only; game/render/FPU behavior was not executed or observed.',
        'World Y is consumed by this native projection, but parent inheritance, visibility, terrain units and editor equivalence are unproven.',
        'Float conversions are conditional on explicitly hypothetical x87 controls; no gameplay rounding mode claim.',
        'Conversion model excludes traps/exception masks and indefinite out-of-range integer results.',
        'Map audit validity is an explicit diagnostic predicate, not native culling or render eligibility.',
        'Pool version/capacity/length checks are stricter than native; PFIL wrapper/trailing integrity is unproven.',
        'Source hashes compare endpoints; transient concurrent edits and original-media identity are not established.']
    serialized = json.dumps(result, separators=(',', ':'), allow_nan=False) + '\n'
    with output.open('x', encoding='utf-8') as stream:
        stream.write(serialized)
    audit = result.get('map_audit', {})
    print(json.dumps({'output': str(output), 'instruction_assertions': result['instruction_assertions'],
                      'maps': len(audit.get('maps', [])), 'all_74_maps_audited': audit.get('all_74_maps_audited'),
                      'errors': len(audit.get('errors', [])), 'missing_pool_pairs': len(audit.get('missing_pool_pairs', [])),
                      'totals': audit.get('totals', {}), 'sources_unchanged': len(sources),
                      'worldY_used_static': True, 'runtime_rounding_mode_observed': False}))
    return 1 if audit and not audit['all_74_maps_audited'] else 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, TypeError, struct.error) as exc:
        print(json.dumps({'error': str(exc)}), file=sys.stderr)
        raise SystemExit(1)
