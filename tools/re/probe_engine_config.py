#!/usr/bin/env python3
"""Read the fingerprinted game's encoded engine section table; never execute it."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from probe_pua import EXE_SHA256, require


def read_va(exe, address, size):
    nt = struct.unpack_from('<I', exe, 0x3c)[0]
    require(exe[:2] == b'MZ' and exe[nt:nt + 4] == b'PE\0\0', 'not PE')
    count, opt_size = struct.unpack_from('<H', exe, nt + 6)[0], struct.unpack_from('<H', exe, nt + 20)[0]
    opt = nt + 24
    require(struct.unpack_from('<H', exe, opt)[0] == 0x10b, 'expected PE32')
    rva = address - struct.unpack_from('<I', exe, opt + 28)[0]
    for index in range(count):
        section = opt + opt_size + index * 40
        va, raw_size, raw_offset = struct.unpack_from('<III', exe, section + 12)
        if va <= rva and rva + size <= va + raw_size:
            offset = raw_offset + rva - va
            require(offset + size <= len(exe), 'read exceeds source')
            return exe[offset:offset + size]
    raise ValueError('address is not file-backed')


def inspect(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, 'unsupported EXE fingerprint')
    table = read_va(exe, 0x632b98, 36 * 22)
    sections = []
    for index in range(36):
        record = table[index * 22:(index + 1) * 22]
        require(0x39 in record, 'unterminated encoded section')
        encoded = record[:record.index(0x39)]
        decoded = bytes((((c ^ 0x39) >> 3) | ((c ^ 0x39) << 5)) & 255 for c in encoded)
        require(bytes((((c << 3) | (c >> 5)) & 255) ^ 0x39 for c in decoded) == encoded,
                'section roundtrip failed')
        sections.append({'index': index, 'prefix': decoded.decode('ascii')})
    # Extract only header metadata from the embedded defaults, never full game data.
    default = read_va(exe, 0x615e40, 4096).split(b'\0', 1)[0].decode('ascii')
    headers = [line.strip().lower() for line in default.splitlines() if line.strip().startswith('[')]
    recognized = []
    for header in headers:
        matches = [s['index'] for s in sections if header.startswith(s['prefix'])]
        require(matches and matches[0] != 35, 'unrecognized default header')
        recognized.append({'header': header, 'section_index': matches[0]})
    return {'exe_sha256': EXE_SHA256, 'table_va': '0x632b98', 'record_stride': 22,
            'sections': sections, 'embedded_default_headers': recognized,
            'limitations': ['static metadata only; no current process or effective configuration observed',
                            'index 35 is the generic opening-bracket fallback; prefixes are not corrected',
                            'not a full configuration parser or a runtime emulator']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(args.exe.resolve().parent):
        parser.error('output must be new and outside input directory')
    try:
        result = inspect(args.exe.read_bytes())
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'sections': len(result['sections']),
                          'default_headers': len(result['embedded_default_headers'])}))
        return 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
