#!/usr/bin/env python3
"""Read-only DirectDraw initialization instruction frontier; not runtime FPU evidence."""
import argparse
import hashlib
import json
from pathlib import Path
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require


def inspect(source):
    require(hashlib.sha256(source).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = []
    index = {}
    for address, length in [(0x5601f0, 159), (0x560290, 2477)]:
        instructions = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(source, address, length), address))
        require(sum(i.size for i in instructions) == length, 'incomplete region decoding')
        writers = [hex(i.address) for i in instructions if i.mnemonic in
                   ['fldcw', 'fninit', 'finit', 'frstor', 'fxrstor', 'xrstor']]
        require(not writers, 'direct FPU environment writer appeared in selected body')
        index.update({i.address: i for i in instructions})
        regions.append({'address': hex(address), 'bytes': length, 'instructions': len(instructions),
                        'direct_fpu_writers': writers})
    checks = [(0x560215, 'call', 'dword ptr cs:[0x5d8804]'),
              (0x560233, 'push', '0x604614'),
              (0x560239, 'call', 'dword ptr cs:[0x5d87d8]'),
              (0x560240, 'mov', 'dword ptr [0x29e9fc0], eax'),
              (0x5602d5, 'call', 'dword ptr [0x29e9fc0]'),
              (0x5602ea, 'push', '0x63eff4'),
              (0x5602f3, 'call', 'dword ptr [esi]')]
    for address, mnemonic, operands in checks:
        require((index[address].mnemonic, index[address].op_str) == (mnemonic, operands),
                f'instruction mismatch {address:#x}')
    require(read_va(source, 0x604614, 17) == b'DirectDrawCreate\0', 'symbol mismatch')
    return {'exe_sha256': EXE_SHA256, 'regions': regions, 'instruction_assertions': len(checks),
            'query_interface_iid_bytes': read_va(source, 0x63eff4, 16).hex(),
            'limitations': ['selected recognized bodies only; no transitive callees or external DLL code',
                            'absence of direct writers does not prove preservation across API calls',
                            'gameplay control word and exception state remain unobserved']}


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
    print(json.dumps(result))


if __name__ == '__main__':
    main()
