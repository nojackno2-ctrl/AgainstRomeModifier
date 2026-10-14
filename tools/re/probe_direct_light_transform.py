#!/usr/bin/env python3
"""Read-only native owner-light x87 symbolic transform proof; not runtime emulation."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from collections import Counter
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require
from probe_object_anim_links import decode


def inspect(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = ((0x49f810, 618), (0x4bd320, 124), (0x4bd5f0, 98),
               (0x4afe10, 864), (0x4b2344, 56), (0x4b2412, 110), (0x5c5d46, 63), (0x48c0c0, 339))
    ins = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    index = {i.address: i for i in ins}
    checks = ((0x4bd347, 'mov', 'ebx, dword ptr [eax*4 + 0xa14c06]'),
              (0x4bd34e, 'sar', 'ebx, 0x10'),
              (0x4bd617, 'mov', 'ebx, dword ptr [eax*4 + 0xa14c06]'),
              (0x4bd61e, 'sar', 'ebx, 0x10'),
              (0x4bd637, 'fld', 'dword ptr [eax*4 + 0xb1884c]'),
              (0x4b236c, 'mov', 'word ptr [edx + 0xc64930], ax'),
              (0x4b243a, 'mov', 'dword ptr [edx + 0xc64934], eax'),
              (0x4b2471, 'mov', 'dword ptr [edx + 0xc64938], eax'),
              (0x4afede, 'mov', 'word ptr [ebx + 0x74], 0'),
              (0x4afee4, 'mov', 'dword ptr [ebx + 0x78], 0'),
              (0x4afeeb, 'mov', 'dword ptr [ebx + 0x7c], 0'),
              (0x5c5d46, 'fcos', ''), (0x5c5d50, 'fsin', ''),
              (0x49f9d7, 'call', '0x49f660'),
              (0x48c1e3, 'call', '0x414f10'), (0x48c154, 'mov', 'dword ptr [esi + 0x10], eax'))
    for a, mnemonic, operands in checks:
        require((index[a].mnemonic, index[a].op_str) == (mnemonic, operands), f'instruction mismatch {a:#x}')
    table = struct.unpack('<209I', read_va(exe, 0x4b1130, 836))
    require((table[67], table[70], table[71]) == (0x4b2344, 0x4b2412, 0x4b2449), 'callback mismatch')
    constants = {a: struct.unpack('<f', read_va(exe, a, 4))[0] for a in (0x5f4bc0, 0x5f4bb4, 0x5f4bb8, 0x5f4bbc)}
    require(tuple(read_va(exe, a, 4).hex() for a in constants) == ('0000b4c2', '610b363b', '00000040', 'db0f4940'), 'constant mismatch')
    memory = {'dword ptr [esp + 0x18]': 'rotation', 'dword ptr [edx + 0xc64938]': 'lightang',
              'dword ptr [edx + 0xc64934]': 'lightrad', 'word ptr [edx + 0xc64930]': 'lihei',
              'dword ptr [esp + 0x20]': 'Y', 'dword ptr [esp + 4]': 'X', 'dword ptr [esp]': 'Z'}
    memory.update({f'dword ptr [{a:#x}]': value for a, value in constants.items()})
    stack, stored = [], {}
    def st(operand):
        return 0 if operand == 'st(0)' else int(operand.removeprefix('st(').removesuffix(')'))
    for i in ins:
        if not 0x49f95d <= i.address < 0x49f9cb: continue
        op, arg = i.mnemonic, i.op_str
        if op in ('fld', 'fild'):
            stack.insert(0, stack[st(arg)] if arg.startswith('st(') else memory[arg])
        elif op == 'fxch':
            k = st(arg); stack[0], stack[k] = stack[k], stack[0]
        elif op in ('fadd', 'fsub', 'fmul'):
            value = stack[st(arg)] if arg.startswith('st(') else memory[arg]
            stack[0] = (op, stack[0], value)
        elif op == 'fmulp':
            k = st(arg); stack[k] = ('fmul', stack[k], stack[0]); stack.pop(0)
        elif op == 'call':
            require(arg in ('0x5c5d50', '0x5c5d46'), 'unexpected transform call')
            stack[0] = ('sin' if arg == '0x5c5d50' else 'cos', stack[0])
        elif op == 'fstp':
            if arg.startswith('st('): stack[st(arg)] = stack[0]
            else: stored[arg] = stack[0]
            stack.pop(0)
        else: raise ValueError(f'unhandled transform instruction {i.address:#x} {op} {arg}')
    theta = ('fmul', ('fmul', ('fmul', ('fsub', ('fadd', 'rotation', -90.), 'lightang'), constants[0x5f4bb4]), 2.), constants[0x5f4bbc])
    expected = {'dword ptr [esp + 4]': ('fadd', ('fmul', ('sin', theta), 'lightrad'), 'X'),
                'dword ptr [esp + 0x20]': ('fadd', 'lihei', 'Y'),
                'dword ptr [esp]': ('fadd', ('fmul', 'lightrad', ('cos', theta)), 'Z')}
    require(stored == expected and not stack, 'symbolic XYZ stack proof failed')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'callback_fields': [67, 70, 71], 'owner_position_runtime_offset': 0xa14c06 + 2 - 0xa14bfc,
            'constants': {hex(a): v for a, v in constants.items()}, 'symbolic_xyz': stored,
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins],
            'limitations': ['symbolic instruction proof, not game execution', 'no x87 rounding/control-word equivalence claim']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path); parser.add_argument('output', type=Path)
    parser.add_argument('--game-root', type=Path)
    args = parser.parse_args(); source = args.exe.resolve(); output = args.output.resolve()
    require(not output.exists() and not output.is_relative_to(source.parent), 'output must be new and outside source directory')
    before = source.read_bytes(); result = inspect(before)
    if args.game_root:
        counts, maps = Counter(), []
        for path in sorted(args.game_root.resolve().glob('MAPS/*/DATA/position.dat')):
            packed = path.read_bytes(); data = decode(packed)
            version, n = struct.unpack_from('<2I', data)
            require(version == 1 and 0 < n <= 33000 and len(data) == 8 + n * 17, 'unsupported position format')
            rotations = [struct.unpack_from('<f', data, 8 + slot * 17 + 13)[0] for slot in range(n) if data[8 + slot * 17]]
            counts.update(rotations)
            require(path.read_bytes() == packed, 'position source changed during audit')
            maps.append({'map': path.parent.parent.name, 'sha256': hashlib.sha256(packed).hexdigest(), 'active': len(rotations)})
        require(maps, 'no position maps found')
        result['position_sources'] = maps
        result['saved_rotation_observations'] = {'min': min(counts), 'max': max(counts), 'common': counts.most_common(12)}
    require(source.read_bytes() == before, 'source changed during proof')
    with output.open('x', encoding='utf-8') as stream: json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({key: result[key] for key in ('instruction_assertions', 'callback_fields', 'owner_position_runtime_offset')}))


if __name__ == '__main__': main()
