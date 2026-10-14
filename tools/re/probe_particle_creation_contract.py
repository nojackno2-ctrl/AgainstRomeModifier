#!/usr/bin/env python3
"""Read-only native particle initialization branches, not a full family allocator."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require


def inspect(source):
    require(hashlib.sha256(source).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    instructions = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(source, 0x4db960, 0x4e4), 0x4db960))
    require(sum(i.size for i in instructions) == 0x4e4, 'incomplete recognized body')
    index = {i.address: i for i in instructions}
    checks = [(0x4db9bc, 'call', '0x4db770'), (0x4dba92, 'call', '0x49bd50'),
              (0x4dbabb, 'call', '0x4db800'), (0x4dbadc, 'mov', 'word ptr [ebx + 0x12355dc], ax'),
              (0x4dbb26, 'mov', 'word ptr [ebx + 0x12355d4], dx'),
              (0x4dbb3a, 'cmp', 'eax, 0x40'), (0x4dbbdb, 'test', 'edx, edx'),
              (0x4dbbdd, 'je', '0x4dbce4'),
              (0x4dbc07, 'mov', 'eax, 0x80010000'),
              (0x4dbc36, 'mov', 'ecx, 0xd8f00000'),
              (0x4dbc7b, 'push', '0xc6fffe00'),
              (0x4dbc80, 'push', '0xc61c4000'),
              (0x4dbc85, 'push', '0xc6fffe00'),
              (0x4dbc12, 'mov', 'dword ptr [ebx + 0x1235630], eax'),
              (0x4dbc47, 'mov', 'dword ptr [ebx + 0x1235730], ecx'),
              (0x4dbc8d, 'call', '0x49f5f0'),
              (0x4dbc9c, 'mov', 'word ptr [esi + 0x1235e2e], ax'),
              (0x4dbcc3, 'mov', 'dword ptr [eax + 0x12355d8], ebx'),
              (0x4dbd35, 'mov', 'dword ptr [ebx + 0x1235630], eax'),
              (0x4dbd5b, 'mov', 'dword ptr [ebx + 0x1235730], eax'),
              (0x4dbdfd, 'call', '0x49f5f0'),
              (0x4dbe05, 'mov', 'word ptr [esi + 0x1235e30], ax')]
    for address, mnemonic, operands in checks:
        require((index[address].mnemonic, index[address].op_str) == (mnemonic, operands),
                f'instruction mismatch {address:#x}')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks),
            'fixed_scale': struct.unpack('<f', read_va(source, 0x5f680a, 4))[0],
            'inverse_scale': struct.unpack('<f', read_va(source, 0x5f680e, 4))[0],
            'initial_xyz_fixed': [-2147418112, -655360000, -2147418112],
            'initial_light_xyz': [-32767.0, -10000.0, -32767.0],
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions],
            'limitations': ['definition field names and complete allocation rollback are unproven',
                            'nonzero definition mode resets current count after creating children/lights',
                            'saved family copying must retain all child links, including outside current count',
                            'not original game execution or a complete create/copy implementation']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    source_path, output = args.exe.resolve(), args.output.resolve()
    require(not output.exists() and not output.is_relative_to(source_path.parent),
            'output must be new and outside source directory')
    source = source_path.read_bytes()
    result = inspect(source)
    require(source_path.read_bytes() == source, 'source changed during probe')
    with output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ['instructions', 'limitations']}))


if __name__ == '__main__':
    main()
