#!/usr/bin/env python3
"""Verify native null auxiliary-template branches without executing the game."""
import argparse
import hashlib
import json
from pathlib import Path

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require


def inspect(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    ins = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, 0x4aa780, 1703), 0x4aa780))
    by_address = {i.address: i for i in ins}
    checks = [(0x4aa78f, 'mov', 'edx, 0xffffffff'),
              (0x4aa7a0, 'mov', 'dword ptr [esp + 0x14], edx'),
              (0x4aa7a4, 'mov', 'dword ptr [esp + 0xc], edx'),
              (0x4aa7a8, 'mov', 'dword ptr [esp + 0x18], edx'),
              (0x4aa9a2, 'test', 'ecx, ecx'), (0x4aa9a4, 'je', '0x4aa9df'),
              (0x4aa9aa, 'call', '0x4ab500'),
              (0x4aa9ed, 'mov', 'edx, dword ptr [esp + 0x14]'),
              (0x4aa9f1, 'mov', 'word ptr [eax*4 + 0xa14c0e], dx'),
              (0x4aa9f9, 'cmp', 'dword ptr [esp + 0x3c], 0'),
              (0x4aa9fe, 'je', '0x4aaa39'), (0x4aaa00, 'call', '0x4ab550'),
              (0x4aaa47, 'mov', 'edx, dword ptr [esp + 0xc]'),
              (0x4aaa4f, 'mov', 'word ptr [eax*4 + 0xa14c10], dx'),
              (0x4aaa57, 'test', 'ebx, ebx'), (0x4aaa59, 'je', '0x4aaa94'),
              (0x4aaa5f, 'call', '0x4ab5b0'),
              (0x4aaaa2, 'mov', 'edx, dword ptr [esp + 0x18]'),
              (0x4aaaa6, 'mov', 'word ptr [eax*4 + 0xa14c14], dx')]
    for address, mnemonic, operands in checks:
        i = by_address.get(address)
        require(i is not None and i.mnemonic == mnemonic and i.op_str == operands,
                f'unexpected instruction at {address:#x}')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins],
            'serialized_links': {'anim': 71, 'gfxtype': 73, 'action': 77},
            'limitations': ['static create function only; no game execution or runtime observation',
                            'serialized link offsets rely on the separately verified objects reader',
                            'null pointers skip pool allocation and retain -1 locals/FFFF links',
                            'does not establish which archetypes require these auxiliary pools',
                            'does not reconstruct templates for damaged/missing source pools']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    game = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    if output.exists() or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(game):
        parser.error('output must be new and outside source and installed-game directories')
    result = inspect(args.exe.read_bytes())
    with output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({'exe_sha256': EXE_SHA256, 'instruction_assertions': result['instruction_assertions']}))


if __name__ == '__main__':
    main()
