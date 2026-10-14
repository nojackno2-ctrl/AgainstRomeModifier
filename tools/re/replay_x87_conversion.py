#!/usr/bin/env python3
"""Controlled reconstructed x87 replay in a disposable Windows x64 Python process.

Does not execute the game or establish its runtime control word. Use only as a
standalone process: x87 exception status is not preserved for an embedding host.
"""
import argparse
import ctypes
import json
import struct
import sys
from pathlib import Path
from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from probe_native_float_conversion import model_cases
from probe_pua import require


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    require(sys.platform == 'win32' and struct.calcsize('P') == 8, 'requires Windows x64 Python')
    require(not args.output.exists(), 'output must be new')
    code = bytes.fromhex('4883ec20 d93c24 6689542404 d96c2404 894c2408 89ca '
                         '81e200000080 81cafdffff3e 8954240c d9442408 d864240c '
                         'db5c2410 d96c2400 8b442410 4883c420 c3')
    instructions = list(Cs(CS_ARCH_X86, CS_MODE_64).disasm(code, 0))
    require(sum(i.size for i in instructions) == len(code), 'incomplete replay decoding')
    require(any(i.mnemonic == 'fldcw' and i.op_str == 'word ptr [rsp]' for i in instructions),
            'missing control word restoration')
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.VirtualAlloc.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_ulong, ctypes.c_ulong]
    kernel.VirtualAlloc.restype = ctypes.c_void_p
    kernel.VirtualProtect.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_ulong,
                                     ctypes.POINTER(ctypes.c_ulong)]
    kernel.VirtualFree.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_ulong]
    address = kernel.VirtualAlloc(None, len(code), 0x3000, 4)
    require(bool(address), 'VirtualAlloc failed')
    rows = []
    try:
        ctypes.memmove(address, code, len(code))
        old = ctypes.c_ulong()
        require(bool(kernel.VirtualProtect(address, len(code), 0x20, ctypes.byref(old))),
                'VirtualProtect failed')
        call = ctypes.WINFUNCTYPE(ctypes.c_int32, ctypes.c_uint32, ctypes.c_uint32)(address)
        for row in model_cases():
            for control, model in row['controls'].items():
                actual = call(int(row['float_bits'], 16), int(control, 16))
                expected = model['integer'] if model['integer'] is not None else -(1 << 31)
                require(actual == expected, f"hardware mismatch {row['float_bits']}/{control}")
                rows.append({'bits': row['float_bits'], 'control': control,
                             'actual': actual, 'model': expected})
    finally:
        require(bool(kernel.VirtualFree(address, 0, 0x8000)), 'VirtualFree failed')
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump({'scope': 'reconstructed x64 x87 replay, not original game ABI or runtime CW',
                   'machine_code': code.hex(),
                   'disassembly': [i.mnemonic + ' ' + i.op_str for i in instructions],
                   'cases': rows}, stream, indent=2)
        stream.write('\n')
    print(f'{len(rows)} hardware cases match conditional model')


if __name__ == '__main__':
    main()
