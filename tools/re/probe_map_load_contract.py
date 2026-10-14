#!/usr/bin/env python3
"""Extract the fingerprinted game's DATA loader call sites without executing it."""
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
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    # Ghidra's function body ends at 0x48f3d1 inclusive; exclude adjacent code.
    instructions = list(md.disasm(read_va(exe, 0x48e960, 0xa72), 0x48e960))
    opens = [n for n, i in enumerate(instructions)
             if i.mnemonic == 'call' and i.op_str == '0x415400']
    require(len(opens) == 24, 'expected exactly 24 loader open calls')
    pools = []
    for ordinal, n in enumerate(opens):
        start = opens[ordinal - 1] + 1 if ordinal else 0
        preceding = instructions[start:n]
        names = []
        for i in preceding:
            if i.mnemonic == 'push' and i.op_str.startswith('0x'):
                address = int(i.op_str, 16)
            elif i.mnemonic == 'mov' and i.op_str.startswith('eax, 0x'):
                address = int(i.op_str.split(', ')[1], 16)
            else:
                continue
            if not 0x5f4500 <= address <= 0x5f4b00:
                continue
            value = read_va(exe, address, 64).split(b'\0', 1)[0]
            if value.endswith(b'.dat'):
                names.append(value.decode('ascii'))
        require(names, 'missing native filename before open')
        # The closest filename belongs to this snprintf/open pair, rather than
        # the diagnostic text following the previous reader.
        name = names[-1]
        end = opens[ordinal + 1] if ordinal + 1 < len(opens) else len(instructions)
        following = instructions[n + 1:end]
        test_index = next((j for j, i in enumerate(following)
                           if i.mnemonic == 'test' and i.op_str == 'eax, eax'), None)
        require(test_index is not None, 'missing open-result test')
        branch = following[test_index + 1]
        expected = ('jne', '0x48ea20') if ordinal == 0 else ('je', '0x48ea00')
        require((branch.mnemonic, branch.op_str) == expected, 'unexpected open failure branch')
        reader_index = next((j for j, i in enumerate(following)
                             if i.mnemonic == 'call' and 0x48ab70 <= int(i.op_str, 16) < 0x48e960), None)
        require(reader_index is not None, 'missing pool reader')
        reader = following[reader_index]
        args = following[max(0, reader_index - 3):reader_index]
        destinations = []
        for i in args:
            if i.mnemonic == 'push' and i.op_str.startswith('0x'):
                destinations.append(int(i.op_str, 16))
            elif i.mnemonic == 'mov' and i.op_str.startswith('eax, 0x'):
                destinations.append(int(i.op_str.split(', ')[1], 16))
        require(len(destinations) == 1 and destinations[0] >= 0x700000,
                'unexpected reader destination argument')
        pools.append({'file': name, 'open_call_va': hex(instructions[n].address),
                      'failure_branch_va': hex(branch.address), 'reader_call_va': hex(reader.address),
                      'reader_va': reader.op_str, 'destination_va': hex(destinations[0]),
                      'reader_call_bytes': reader.bytes.hex()})
    require(len({p['file'] for p in pools}) == 24, 'duplicate native filenames')
    failure = list(md.disasm(read_va(exe, 0x48ea00, 0x20), 0x48ea00))
    require(failure[0].op_str == 'esi, 0x3e8' and failure[-1].mnemonic == 'ret',
            'unexpected common open failure epilogue')
    caller = list(md.disasm(read_va(exe, 0x48742f, 0x4b), 0x48742f))
    require(caller[0].op_str == '0x48e960', 'missing loader call')
    require(not any(i.mnemonic.startswith('j') for i in caller), 'unexpected caller branch')
    require([i.op_str for i in caller if i.mnemonic == 'call'][-3:]
            == ['0x4acfe0', '0x4ad110', '0x4adf40'], 'unexpected CHECK sequence')
    return {'exe_sha256': EXE_SHA256, 'loader_va': '0x48e960', 'pools': pools,
            'open_failure_epilogue': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in failure],
            'caller_to_checks': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in caller],
            'limitations': ['static direct call sites only; no effective virtual-file lookup or runtime observed',
                            'destinations are global addresses, not evidence of uninitialized pointers',
                            'missing-file load failure does not establish a particular hang or crash root cause',
                            'reader return values are not validated by this probe; field semantics are out of scope']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    game = Path(r'C:\Program Files (x86)\Against Rome').resolve()
    if output.exists() or output.is_relative_to(args.exe.resolve().parent) or output.is_relative_to(game):
        parser.error('output must be new and outside input and installed-game directories')
    try:
        result = inspect(args.exe.read_bytes())
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'pools': len(result['pools']), 'exe_sha256': result['exe_sha256']}))
        return 0
    except (OSError, ValueError, struct.error, StopIteration) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
