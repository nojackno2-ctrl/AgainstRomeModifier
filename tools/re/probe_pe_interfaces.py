#!/usr/bin/env python3
"""Read PE32 import/export metadata without loading or executing any module."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from probe_engine_config import read_va
from probe_pua import require


def inspect(data):
    require(len(data) >= 64 and data[:2] == b'MZ', 'not MZ')
    nt = struct.unpack_from('<I', data, 60)[0]
    require(nt + 24 <= len(data) and data[nt:nt + 4] == b'PE\0\0', 'not PE')
    machine, count = struct.unpack_from('<HH', data, nt + 4)
    opt_size, flags = struct.unpack_from('<HH', data, nt + 20)
    opt = nt + 24
    require(opt_size >= 224 and opt + opt_size + count * 40 <= len(data), 'truncated PE32 headers')
    require(struct.unpack_from('<H', data, opt)[0] == 0x10b, 'expected PE32')
    base = struct.unpack_from('<I', data, opt + 28)[0]
    def rd(rva, size):
        return read_va(data, base + rva, size)
    def string(rva):
        result = bytearray()
        for offset in range(1024):
            byte = rd(rva + offset, 1)[0]
            if byte == 0:
                return result.decode('ascii')
            result.append(byte)
        raise ValueError('unterminated interface string')
    sections = []
    raw_end = opt + opt_size + count * 40
    for index in range(count):
        offset = opt + opt_size + index * 40
        name, virtual_size, rva, raw_size, raw_offset = struct.unpack_from('<8sIIII', data, offset)
        require(not raw_size or raw_offset + raw_size <= len(data), 'section exceeds source bytes')
        raw_end = max(raw_end, raw_offset + raw_size)
        sections.append({'name': name.rstrip(b'\0').decode('ascii'), 'rva': rva,
                         'virtual_bytes': virtual_size, 'raw_bytes': raw_size,
                         'raw_offset': raw_offset})
    imports = []
    rva, size = struct.unpack_from('<II', data, opt + 104)
    if rva:
        for index in range(min(size // 20, 4096)):
            lookup, stamp, chain, name, iat = struct.unpack('<5I', rd(rva + index * 20, 20))
            if not any((lookup, stamp, chain, name, iat)):
                break
            members = []
            for slot in range(65536):
                value = struct.unpack('<I', rd((lookup or iat) + slot * 4, 4))[0]
                if not value:
                    break
                members.append({'iat_va': base + iat + slot * 4,
                                'ordinal': value & 65535} if value & 0x80000000 else
                               {'iat_va': base + iat + slot * 4, 'name': string(value + 2)})
            else:
                raise ValueError('unterminated import table')
            imports.append({'library': string(name), 'members': members})
        else:
            raise ValueError('unterminated import directory')
    exports = []
    rva, size = struct.unpack_from('<II', data, opt + 96)
    if rva:
        header = struct.unpack('<IIHH7I', rd(rva, 40))
        ordinal_base, functions, names, eat, name_table, ordinals = header[5:]
        require(names <= functions <= 65536, 'export count exceeds analysis limit')
        labels = {}
        for index in range(names):
            name = struct.unpack('<I', rd(name_table + index * 4, 4))[0]
            slot = struct.unpack('<H', rd(ordinals + index * 2, 2))[0]
            require(slot < functions, 'export ordinal outside EAT')
            labels.setdefault(slot, []).append(string(name))
        for slot in range(functions):
            address = struct.unpack('<I', rd(eat + slot * 4, 4))[0]
            if address:
                record = {'ordinal': ordinal_base + slot, 'names': labels.get(slot, []),
                          'va': base + address}
                if rva <= address < rva + size:
                    record['forwarder'] = string(address)
                exports.append(record)
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data),
            'machine': machine, 'dll': bool(flags & 0x2000), 'image_base': base,
            'entry_va': base + struct.unpack_from('<I', data, opt + 16)[0],
            'sections': sections, 'imports': imports, 'exports': exports,
            'unmapped_tail_bytes': len(data) - raw_end,
            'limitations': ['PE32 metadata only; dynamic loads and runtime interfaces require separate tracing',
                            'unmapped tail may contain certificates or other data, not an inferred payload']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(args.source.resolve().parent):
        parser.error('output must be new and outside source directory')
    try:
        result = inspect(args.source.read_bytes())
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({key: value for key, value in result.items()
                          if key not in ('sections', 'imports', 'exports', 'limitations')}))
        return 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
