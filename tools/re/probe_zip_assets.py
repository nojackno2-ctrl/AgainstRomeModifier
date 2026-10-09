#!/usr/bin/env python3
"""Stream-check eight installed ZIP asset packages without extracting files."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import struct
import zipfile

PACKAGES = ('alr.dat', 'apt.dat', 'floortex.dat', 'gui.dat', 'mp.dat',
            'sfx.dat', 'shad.dat', 'voice.dat')


def signature(prefix):
    if prefix[:4] in (b'ALRA', b'APAT') and len(prefix) >= 8:
        return prefix[:4].decode('ascii') + ':word4=' + str(struct.unpack_from('<I', prefix, 4)[0])
    if prefix[:4] == b'RIFF' and len(prefix) >= 12:
        return 'RIFF:' + prefix[8:12].decode('ascii', errors='replace')
    if prefix[:2] == b'BM':
        return 'BMP'
    # TGA has no reliable fixed magic; do not infer its format from extension.
    return 'other'


def inspect(root):
    reports = []
    paths = defaultdict(list)
    total = 0
    for name in PACKAGES:
        source = root / name
        source_hash = hashlib.sha256()
        with source.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                source_hash.update(chunk)
        members = []
        with zipfile.ZipFile(source) as archive:
            records = archive.infolist()
            declared = sum(x.file_size for x in records)
            total += declared
            if total > 10 * 1024**3 or any(x.file_size > 512 * 1024**2 for x in records):
                raise ValueError('archive exceeds analysis size limit')
            for entry in records:
                digest = hashlib.sha256()
                length = 0
                prefix = bytearray()
                # ZipExtFile validates CRC when EOF is reached. Stream directories
                # too, so malformed directory entries do not escape verification.
                with archive.open(entry) as stream:
                    for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                        digest.update(chunk)
                        length += len(chunk)
                        prefix.extend(chunk[:max(0, 32 - len(prefix))])
                if length != entry.file_size:
                    raise ValueError('decoded length mismatch: ' + name + '/' + entry.filename)
                record = {'path': entry.filename, 'directory': entry.is_dir(),
                          'bytes': length, 'stored_bytes': entry.compress_size,
                          'method': entry.compress_type, 'crc32': f'{entry.CRC:08x}',
                          'sha256': digest.hexdigest(), 'signature': signature(prefix)}
                members.append(record)
                if not entry.is_dir():
                    key = entry.filename.replace('\\', '/').casefold()
                    paths[key].append({'package': name, 'path': entry.filename,
                                       'sha256': record['sha256']})
        reports.append({'package': name, 'sha256': source_hash.hexdigest(),
                        'bytes': source.stat().st_size, 'entries_crc_verified': len(members),
                        'decoded_bytes': declared,
                        'methods': dict(Counter(r['method'] for r in members)),
                        'signatures': dict(Counter(r['signature'] for r in members)),
                        'members': members})
    overlaps = [{'normalized_path': path, 'same_payload': len({m['sha256'] for m in matches}) == 1,
                 'members': matches} for path, matches in sorted(paths.items()) if len(matches) > 1]
    return {'packages': reports, 'entries_crc_verified': sum(p['entries_crc_verified'] for p in reports),
            'decoded_bytes': total, 'normalized_path_collisions': overlaps,
            'limitations': ['CRC and byte counts establish ZIP decoding, not full format validity or runtime use',
                            'casefold path normalization is a comparison convention, not native lookup emulation',
                            'no payload extraction or execution; TGA not inferred by extension']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(args.root.resolve()):
        parser.error('output must be new and outside source directory')
    try:
        result = inspect(args.root)
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, ensure_ascii=False, indent=2)
            stream.write('\n')
        print(json.dumps({k: v for k, v in result.items() if k not in ('packages', 'normalized_path_collisions')}))
        print(json.dumps({'packages': len(result['packages']),
                          'path_collisions': len(result['normalized_path_collisions'])}))
        return 0
    except (OSError, ValueError, zipfile.BadZipFile, RuntimeError, NotImplementedError) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
