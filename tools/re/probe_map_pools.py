#!/usr/bin/env python3
"""Read-only structural probe of native action/animation/gfxtype map pools."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from inventory_game_directory import pfil_prefix
from probe_pua import require


def inspect(data, family):
    source_hash = hashlib.sha256(data).hexdigest()
    wrapped = data.startswith(b'PFIL@')
    if wrapped:
        require(len(data) >= 64, 'short PFIL header')
        size = struct.unpack_from('<i', data, 16)[0]
        require(0 <= size <= 4 * 1024 * 1024, 'decoded pool exceeds analysis limit')
        _, data = pfil_prefix(data, limit=size)
    require(len(data) >= 8, 'short pool header')
    version, count = struct.unpack_from('<II', data)
    require(version == 1, 'unsupported pool version')
    # The native readers do not visibly enforce this capacity; the probe does.
    require(count <= 14000, 'count exceeds native writer capacity')
    if family == 'action':
        require(len(data) >= 12, 'short action header')
        words = struct.unpack_from('<I', data, 8)[0]
        require(words == 6, 'unsupported action word count')
        require(len(data) == 12 + count * 25, 'action length does not match native layout')
        offset, stride = 12, 25
    elif family == 'anim':
        require(len(data) == 8 + count * 25, 'anim length does not match native layout')
        # 21-byte records, then two independent count * uint16 arrays.
        offset, stride = 8, 21
    elif family == 'gfxtype':
        require(len(data) == 8 + count * 17, 'gfxtype length does not match native layout')
        # 15-byte records (byte + seven uint16), then one count * uint16 array.
        offset, stride = 8, 15
    else:
        raise ValueError('unsupported family')
    leading = Counter(data[offset + i * stride] for i in range(count))
    return {'family': family, 'source_sha256': source_hash, 'pfil_wrapped': wrapped,
            'decoded_sha256': hashlib.sha256(data).hexdigest(), 'decoded_bytes': len(data),
            'version': version, 'slots': count, 'record_bytes': stride,
            'leading_byte_histogram': dict(sorted(leading.items())),
            'trailing_array_bytes': count * (4 if family == 'anim' else 2 if family == 'gfxtype' else 0)}


def scan(root):
    reports, errors = [], []
    maps = root / 'MAPS'
    require(maps.is_dir(), 'missing MAPS directory')
    for directory in sorted(maps.iterdir()):
        if not directory.is_dir() or directory.is_symlink():
            continue
        for family in ('action', 'anim', 'gfxtype'):
            path = directory / 'DATA' / (family + '.dat')
            if not path.is_file():
                continue
            require(path.resolve().is_relative_to(root.resolve()), 'pool path escapes source')
            try:
                report = inspect(path.read_bytes(), family)
                reports.append({'path': path.relative_to(root).as_posix(), **report})
            except (OSError, ValueError, struct.error) as exc:
                errors.append({'path': path.relative_to(root).as_posix(), 'error': str(exc)})
    return {'files': reports, 'errors': errors,
            'families': dict(Counter(r['family'] for r in reports)),
            'limitations': ['native writer version 1 layout only; stricter capacity and length checks than native readers',
                            'PFIL decoded to declared size; compressed trailing bytes and wrapper integrity not established',
                            'no field semantics, pool cross-references, runtime load success or missing-file completeness inferred']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(args.root.resolve()):
        parser.error('output must be new and outside source directory')
    try:
        result = scan(args.root)
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'families': result['families'], 'errors': len(result['errors'])}))
        return 1 if result['errors'] else 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
