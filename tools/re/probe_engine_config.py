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
    options = []
    for index in range(40):
        case, name, alias = struct.unpack('<III', read_va(exe, 0x61a444 + index * 12, 12))
        def cstring(address):
            data = read_va(exe, address, 100)
            require(b'\0' in data, 'unterminated option text')
            return data.split(b'\0', 1)[0].decode('ascii')
        options.append({'index': index, 'parser_case': case,
                        'long_name': cstring(name), 'short_alias': cstring(alias)})
    return {'exe_sha256': EXE_SHA256, 'table_va': '0x632b98', 'record_stride': 22,
            'sections': sections, 'embedded_default_headers': recognized,
            'game_option_table_va': '0x61a444', 'game_option_record_stride': 12,
            'game_options': options, 'registry_selector_resources': string_resources(exe, (9998, 9999)),
            'limitations': ['static metadata only; no current process or effective configuration observed',
                            'index 35 is the generic opening-bracket fallback; prefixes are not corrected',
                            'not a full configuration parser or a runtime emulator']}


def string_resources(exe, ids):
    """Return specified RT_STRING entries, including empty entries and languages."""
    nt = struct.unpack_from('<I', exe, 0x3c)[0]
    opt = nt + 24
    base = struct.unpack_from('<I', exe, opt + 28)[0]
    rva, size = struct.unpack_from('<II', exe, opt + 112)
    root = read_va(exe, base + rva, size)

    def entries(offset):
        require(0 <= offset <= len(root) - 16, 'resource directory out of bounds')
        named, numbered = struct.unpack_from('<HH', root, offset + 12)
        count = named + numbered
        require(offset + 16 + count * 8 <= len(root), 'resource entries out of bounds')
        return [struct.unpack_from('<II', root, offset + 16 + i * 8) for i in range(count)]

    types = [value for key, value in entries(0) if key == 6]
    require(len(types) == 1 and types[0] & 0x80000000, 'missing RT_STRING directory')
    blocks = entries(types[0] & 0x7fffffff)
    result = []
    for resource_id in ids:
        matches = [value for key, value in blocks if key == resource_id // 16 + 1]
        require(len(matches) == 1 and matches[0] & 0x80000000, 'missing string block')
        for language, leaf in entries(matches[0] & 0x7fffffff):
            require(not leaf & 0x80000000 and leaf + 16 <= len(root), 'invalid resource leaf')
            data_rva, length = struct.unpack_from('<II', root, leaf)
            data = read_va(exe, base + data_rva, length)
            offset = 0
            for index in range(16):
                require(offset + 2 <= len(data), 'truncated string length')
                chars = struct.unpack_from('<H', data, offset)[0]
                offset += 2
                require(offset + chars * 2 <= len(data), 'truncated string data')
                if index == resource_id % 16:
                    result.append({'id': resource_id, 'language': language,
                                   'text': data[offset:offset + chars * 2].decode('utf-16le')})
                offset += chars * 2
    return result


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
                          'default_headers': len(result['embedded_default_headers']),
                          'game_options': len(result['game_options']),
                          'registry_selector_entries': len(result['registry_selector_resources'])}))
        return 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
