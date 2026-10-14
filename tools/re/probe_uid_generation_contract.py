#!/usr/bin/env python3
"""Verify bounded native UID generation/save/load instructions without execution."""
import argparse
import hashlib
import json
from pathlib import Path

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require


def inspect(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    windows = {}
    for name, address, size in [('generator', 0x4ac4f0, 0x35),
                                ('counter_getter', 0x499240, 7),
                                ('counter_setter', 0x499220, 16),
                                ('engine_writer_counter', 0x489d1a, 16),
                                ('engine_reader_counter', 0x48d8d1, 9)]:
        windows[name] = list(md.disasm(read_va(exe, address, size), address))
    def has(name, mnemonic, operands):
        return any(i.mnemonic == mnemonic and i.op_str == operands for i in windows[name])
    for mnemonic, operands in [('call', '0x499240'), ('inc', 'eax'),
                               ('call', '0x499220'), ('and', 'ebx, 0xffffff'),
                               ('shl', 'eax, 0x18'), ('add', 'eax, ebx')]:
        require(has('generator', mnemonic, operands), 'missing UID generator instruction')
    require(has('counter_getter', 'mov', 'eax, dword ptr [0x771c3c]'), 'unexpected counter getter')
    require(has('counter_setter', 'mov', 'dword ptr [0x771c3c], eax'), 'unexpected counter setter')
    require(has('engine_writer_counter', 'mov', 'edi, dword ptr [0x771c3c]'), 'unexpected counter writer')
    require(has('engine_reader_counter', 'mov', 'dword ptr [edi + 0x454], eax'), 'unexpected counter reader')
    require(0x7717e8 + 0x454 == 0x771c3c, 'counter address mismatch')
    # Decode this complete caller from its entry, never from the middle of an instruction.
    caller = list(md.disasm(read_va(exe, 0x4a9740, 0x5e0), 0x4a9740))
    require(any(i.address == 0x4a9c41 and i.mnemonic == 'push' and i.op_str == '0' for i in caller),
            'missing team-zero create argument')
    require(any(i.address == 0x4a9c84 and i.op_str == '0x4aa780' for i in caller), 'missing native create')
    require(any(i.address == 0x4a9cad and i.op_str == '0x4bd970' for i in caller), 'missing subsequent team setter')
    windows['caller_create_then_set_team'] = [i for i in caller if 0x4a9c30 <= i.address <= 0x4a9cb2]
    return {'exe_sha256': EXE_SHA256, 'counter_global_va': '0x771c3c',
            'engine_version1_counter_offset': 34,
            'instructions': {name: [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in items]
                             for name, items in windows.items()},
            'limitations': ['static bounded instruction windows only; runtime not observed',
                            'offset 34 is the previously traced version-1 field order, not a full engine parser',
                            'native 24-bit wrap collision policy and every caller remain unresolved',
                            'team-zero path does not imply all native creation uses team zero']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    game = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    if output.exists() or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(game):
        parser.error('output must be new and outside source and installed-game directories')
    try:
        result = inspect(args.exe.read_bytes())
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'verified_windows': len(result['instructions']), 'exe_sha256': EXE_SHA256}))
        return 0
    except (OSError, ValueError) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
