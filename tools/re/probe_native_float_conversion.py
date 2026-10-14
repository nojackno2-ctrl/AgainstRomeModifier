#!/usr/bin/env python3
"""Read-only native conversion/startup contract and exact conditional x87 arithmetic model."""
import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from probe_engine_config import read_va
from probe_pua import EXE_SHA256, require


def integer_round(value, mode):
    lower = value.numerator // value.denominator
    remainder = value - lower
    if mode == 1: return lower
    if mode == 2: return lower + bool(remainder)
    if mode == 3: return lower + (value < 0 and bool(remainder))
    return lower + (remainder > Fraction(1, 2) or remainder == Fraction(1, 2) and lower % 2 != 0)


def operation_round(value, precision, mode):
    if not value: return value
    absolute = abs(value)
    exponent = absolute.numerator.bit_length() - absolute.denominator.bit_length()
    power = Fraction(2) ** exponent
    if absolute < power: exponent -= 1
    step = Fraction(2) ** (exponent - precision + 1)
    return integer_round(value / step, mode) * step


def conditional_conversion(bits, control):
    value = struct.unpack('<f', struct.pack('<I', bits))[0]
    require(bits & 0x7f800000 != 0x7f800000, 'model requires finite float32 input')
    precision = {0: 24, 2: 53, 3: 64}.get((control >> 8) & 3)
    require(precision is not None, 'reserved x87 precision control')
    mode = (control >> 10) & 3
    bias = Fraction(struct.unpack('<f', bytes.fromhex('fdffff3e'))[0])
    if bits & 0x80000000: bias = -bias
    adjusted = operation_round(Fraction(value) - bias, precision, mode)
    result = integer_round(adjusted, mode)
    return {'integer': result if -(1 << 31) <= result < 1 << 31 else None,
            'out_of_signed32_range': not -(1 << 31) <= result < 1 << 31,
            'precision_bits': precision, 'rounding_bits': mode}


def inspect(source):
    require(hashlib.sha256(source).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    regions = [(0x419c80, 55), (0x5cabf0, 68), (0x5d2180, 86), (0x5d2170, 16),
               (0x5ccce8, 87), (0x5d6050, 78), (0x5d5908, 141), (0x5c5af2, 29), (0x5cd8c0, 178)]
    instructions = [i for a, n in regions for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(read_va(source, a, n), a)]
    index = {i.address: i for i in instructions}
    checks = [(0x419c95, 'and', 'eax, 0x80000000'), (0x419c9a, 'or', 'eax, dword ptr [0x612c70]'),
              (0x419ca5, 'fsub', 'dword ptr [0x64d650]'), (0x419cab, 'fistp', 'dword ptr [0x64d650]'),
              (0x5cd901, 'call', '0x5ccce8'), (0x5ccd23, 'mov', 'eax, 0x21'),
              (0x5ccd28, 'call', '0x5d2180'), (0x5ccd33, 'mov', 'eax, 0xff'),
              (0x5ccd38, 'call', '0x5d2180'), (0x5d2186, 'mov', 'esi, 0x63cda4'),
              (0x5d218e, 'mov', 'eax, 0x63cd2c'), (0x5d21a4, 'cmp', 'dl, cl'),
              (0x5d21ac, 'add', 'eax, 6'), (0x5d21be, 'lea', 'eax, [ebx + 2]'),
              (0x5d21c2, 'call', '0x5d2170'), (0x5d217c, 'call', 'dword ptr [eax]'),
              (0x5cabf1, 'mov', 'ah, byte ptr [0x63c124]'), (0x5cac06, 'fninit', ''),
              (0x5c5afb, 'mov', 'byte ptr [esp + 1], 0x1f'), (0x5c5b00, 'fldcw', 'word ptr [esp]'),
              (0x5c5b03, 'frndint', ''), (0x5c5b05, 'fldcw', 'word ptr [esp + 4]'),
              (0x5d5956, 'push', '0x9f'), (0x5d595b, 'push', '0'), (0x5d595d, 'call', '0x5d6050')]
    for a, mnemonic, operands in checks:
        require(a in index and (index[a].mnemonic, index[a].op_str) == (mnemonic, operands), f'instruction mismatch {a:#x}')
    require(read_va(source, 0x612c70, 4).hex() == 'fdffff3e', 'bias mismatch')
    require(read_va(source, 0x63c124, 1) == b'\0', 'initializer guard not initially clear')
    table = [{'address': hex(a), 'state': state, 'priority': priority, 'function': hex(function)}
             for a in range(0x63cd2c, 0x63cda4, 6)
             for state, priority, function in [struct.unpack('<BBI', read_va(source, a, 6))]]
    require(next(item for item in table if item['address'] == '0x63cd6e') ==
            {'address': '0x63cd6e', 'state': 0, 'priority': 2, 'function': '0x5cabf0'}, 'startup callback mismatch')
    require(0x9f & 0x0f00 == 0, 'exception mask unexpectedly changes precision/rounding')
    return {'exe_sha256': EXE_SHA256, 'instruction_assertions': len(checks), 'startup_table': table,
            'bias': struct.unpack('<f', read_va(source, 0x612c70, 4))[0],
            'instructions': [f'{i.address:#x} {i.bytes.hex()} {i.mnemonic} {i.op_str}' for i in instructions]}


def model_cases():
    bits = [0, 0x80000000, 0x3f000000, 0xbf000000, 0x3f7ffffe, 0x3f7fffff, 0xbf7fffff,
            0x3f800000, 0xbf800000, 0x3fc00000, 0x3fffffff, 0x4effffff, 0x4f000000, 0xcf000000]
    controls = [0x037f, 0x007f, 0x0f7f, 0x077f, 0x0b7f]
    cases = [{'float_bits': f'{value:08x}', 'float32': struct.unpack('<f', struct.pack('<I', value))[0],
              'controls': {f'{cw:04x}': conditional_conversion(value, cw) for cw in controls}} for value in bits]
    require(conditional_conversion(0x3f7fffff, 0x037f)['integer'] == 1, 'extended-nearest boundary mismatch')
    require(conditional_conversion(0x3f7fffff, 0x007f)['integer'] == 0, 'single-nearest boundary mismatch')
    require(conditional_conversion(0xbf7fffff, 0x037f)['integer'] == -1, 'negative boundary mismatch')
    require(conditional_conversion(0, 0x077f)['integer'] == -1, 'signed zero/down boundary mismatch')
    require(conditional_conversion(0x80000000, 0x0b7f)['integer'] == 1, 'signed zero/up boundary mismatch')
    require(conditional_conversion(0x4f000000, 0x037f)['out_of_signed32_range'], 'overflow model mismatch')
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path); parser.add_argument('output', type=Path)
    args = parser.parse_args(); source_path = args.exe.resolve(); output = args.output.resolve()
    require(not output.exists() and not output.is_relative_to(source_path.parent), 'output must be new and outside source directory')
    source = source_path.read_bytes(); result = inspect(source); result['conditional_models'] = model_cases()
    require(source_path.read_bytes() == source, 'source changed during probe')
    result['limitations'] = ['static startup reachability and conditional rational model, not game execution',
        'external APIs and gameplay/thread control words are unobserved',
        'x87 exception masks/traps and out-of-range indefinite results are not emulated',
        'startup FNINIT is evidence for initial extended-nearest mode, not perpetual gameplay mode']
    with output.open('x', encoding='utf-8') as stream: json.dump(result, stream, indent=2); stream.write('\n')
    print(json.dumps({'instruction_assertions': result['instruction_assertions'], 'startup_records': len(result['startup_table']),
                      'conditional_cases': len(result['conditional_models']) * 5, 'gameplay_fpu_verified': False}))


if __name__ == '__main__': main()
