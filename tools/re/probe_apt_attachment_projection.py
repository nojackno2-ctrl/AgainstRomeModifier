#!/usr/bin/env python3
"""Read-only native APT attachment projection instruction proof, one known EXE."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require


def inspect(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = [(0x4e5420, 0xda), (0x4e6ec0, 0x89), (0x419c80, 0x37),
               (0x49c270, 0x21), (0x49c2a0, 0x2b), (0x4c82a0, 0x222),
               (0x4e5710, 0xce0), (0x4c84e0, 0x1f0),
               (0x4c7cf0, 0x190)]
    ins = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(exe, a, n), a)]
    by_address = {i.address: i for i in ins}
    checks = [(0x4e5430, 'mov', 'edi, dword ptr [0x626928]'),
              (0x4e5436, 'mov', 'esi, dword ptr [edx + 0x64]'),
              (0x4e5439, 'mov', 'ebx, dword ptr [edx + 0x60]'),
              (0x4e543c, 'sub', 'ecx, esi'), (0x4e543e, 'sub', 'eax, ebx'),
              (0x4e5444, 'shl', 'ecx, 5'), (0x4e5447, 'shl', 'eax, 4'),
              (0x4e544c, 'sub', 'ebx, eax'), (0x4e545d, 'sar', 'eax, 0xa'),
              (0x4e5470, 'sar', 'eax, 0xa'),
              (0x4e6efe, 'imul', 'eax, dword ptr [edx + 0x24]'),
              (0x4e6f02, 'add', 'ecx, eax'),
              (0x4e6f2b, 'mov', 'ecx, dword ptr [ecx + 0x34]'),
              (0x4e6f2e, 'mov', 'dword ptr [eax + 8], ecx'),
              (0x4e6f33, 'mov', 'edx, dword ptr [edx + eax + 0x40]'),
              (0x4e6f37, 'mov', 'dword ptr [eax + 0x20], edx'),
              (0x4e5d54, 'lea', 'edi, [edi + 0x2c]'),
              (0x4e5d95, 'mov', 'dword ptr [edx + 0x30], ebx'),
              (0x4e5f29, 'mov', 'eax, dword ptr [ebx + 0x34]'),
              (0x4e5f43, 'mov', 'dword ptr [ebx + 0x40], eax'),
              (0x419c95, 'and', 'eax, 0x80000000'),
              (0x419c9a, 'or', 'eax, dword ptr [0x612c70]'),
              (0x419ca5, 'fsub', 'dword ptr [0x64d650]'),
              (0x419cab, 'fistp', 'dword ptr [0x64d650]'),
              (0x49c274, 'shl', 'eax, 6'), (0x49c27b, 'add', 'eax, 0x20'),
              (0x49c284, 'shl', 'eax, 6'), (0x49c28b, 'add', 'eax, 0x20'),
              (0x49c2a4, 'call', '0x419c80'), (0x49c2ac, 'sar', 'eax, 6'),
              (0x49c2b9, 'call', '0x419c80'), (0x49c2c5, 'sar', 'eax, 6'),
              (0x4c83a0, 'call', '0x4e6ec0'), (0x4c83d1, 'call', '0x49c2a0'),
              (0x4c83e7, 'mov', 'ecx, dword ptr [eax + 0x7c]'),
              (0x4c83eb, 'mov', 'ebx, dword ptr [eax + 0x78]'),
              (0x4c83f0, 'call', '0x4e5420'), (0x4c8404, 'sub', 'edi, eax'),
              (0x4c840e, 'sub', 'ebp, eax'),
              (0x4c841c, 'cmp', 'edx, dword ptr [eax + 8]'),
              (0x4c8445, 'mov', 'eax, dword ptr [eax + 0x20]'),
              (0x4c8466, 'call', '0x4e5420'), (0x4c847d, 'add', 'eax, edx'),
              (0x4c8484, 'add', 'edx, ecx'),
              (0x4c865c, 'cmp', 'edx, dword ptr [eax + 0xc]'),
              (0x4c7e75, 'cmp', 'eax, 0xfff0bdc0')]
    for a, mnemonic, operands in checks:
        i = by_address.get(a)
        require(i is not None and (i.mnemonic, i.op_str) == (mnemonic, operands), f'unexpected instruction {a:#x}')
    matrix = struct.unpack('<6i', read_va(exe, 0x626914, 24))
    bias, = struct.unpack('<f', read_va(exe, 0x612c70, 4))
    require(matrix == (32, 32, 16, 16, 1024, 1), 'unexpected default matrix')
    require(bias == 0.49999991059303284, 'unexpected float conversion bias')
    return dict(exe_sha256=EXE_SHA256, instruction_assertions=len(checks), default_matrix=matrix,
                conversion_bias=bias, limitations=['default matrix only', 'assumes nearest-even x87 mode',
                'static completed stage, axis1/2 zero preview; no gameplay validation'],
                instructions=[f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in ins])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    exe, output = args.exe.resolve(), args.output.resolve()
    require(not output.exists(), 'output already exists')
    require(not output.is_relative_to(exe.parent), 'output inside source game directory')
    before = exe.read_bytes()
    result = inspect(before)
    require(hashlib.sha256(exe.read_bytes()).hexdigest() == result['exe_sha256'], 'source changed')
    output.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(f"PASS {result['instruction_assertions']} instructions + matrix/bias; source hash unchanged")


if __name__ == '__main__':
    main()
