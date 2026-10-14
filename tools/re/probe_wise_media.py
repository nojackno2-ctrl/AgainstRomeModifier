#!/usr/bin/env python3
"""Read-only audit of this known Wise split installer inside two ISO9660 images.

InstallFile candidates use the published SabreTools layout, not a full Wise VM
parser. Every accepted file must pass compressed bounds, EOF, length and CRC.
No installer actions are executed. Only explicitly selected files are extracted.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import zlib

ISO_HASHES = ('b48bcd4675bb20ae782c35480c40ffc53e3bb663414e2aa7a70517851d285484',
              'd66b47df8d62bc0ea2ee5af3309eafb6a828871ec085e9ef2e831b8c4b2e99d2')
BASE = 0x30fcf


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha_file(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


class IsoMember:
    def __init__(self, path, target):
        self.file = path.open('rb')
        self.file.seek(16 * 2048)
        pvd = self.file.read(2048)
        require(pvd[:7] == b'\x01CD001\x01', 'not primary ISO9660')
        root = pvd[156:190]
        extent, size = self.fields(root)
        self.file.seek(extent * 2048)
        directory = self.file.read(size)
        offset = 0
        while offset < len(directory):
            count = directory[offset]
            if not count:
                offset = (offset // 2048 + 1) * 2048
                continue
            record = directory[offset:offset + count]
            require(len(record) == count and count >= 34, 'invalid directory record')
            name = record[33:33 + record[32]].decode('ascii').split(';')[0]
            if name.casefold() == target.casefold():
                require(record[25] & 0x82 == 0, 'directory or multi-extent source not supported')
                extent, self.size = self.fields(record)
                self.offset = extent * 2048
                require(self.offset + self.size <= path.stat().st_size, 'ISO member exceeds image')
                return
            offset += count
        self.file.close()
        raise ValueError('missing ISO member: ' + target)

    @staticmethod
    def fields(record):
        extent, size = struct.unpack_from('<I', record, 2)[0], struct.unpack_from('<I', record, 10)[0]
        require(extent == struct.unpack_from('>I', record, 6)[0] and size == struct.unpack_from('>I', record, 14)[0],
                'ISO endian fields disagree')
        return extent, size

    def read(self, offset, count):
        require(0 <= offset <= self.size and 0 <= count <= self.size - offset, 'member read outside bounds')
        self.file.seek(self.offset + offset)
        result = self.file.read(count)
        require(len(result) == count, 'short source read')
        return result


class Composite:
    def __init__(self, members):
        self.members = members
        self.size = sum(member.size for member in members)

    def read(self, offset, count):
        require(0 <= offset <= self.size and 0 <= count <= self.size - offset, 'split read outside bounds')
        result = bytearray()
        for member in self.members:
            if offset >= member.size:
                offset -= member.size
                continue
            part = min(count, member.size - offset)
            result.extend(member.read(offset, part))
            count -= part
            offset = 0
            if not count:
                break
        require(count == 0, 'incomplete split read')
        return bytes(result)


def bootstrap(source):
    # Native entry4021af reads these blocks; 401edf strips the trailing CRC.
    header = source.read(0, 1048576)
    cursor = 0x3a9e
    sizes = (0x130c, 0x14ad0, 0x152b9, 0x2397, 0x105)
    blocks, script = [], None
    for index, packed in enumerate(sizes):
        part = header[cursor:cursor + packed]
        decoder = zlib.decompressobj(-15)
        data = decoder.decompress(part[:-4])
        require(decoder.eof and not decoder.unused_data, 'invalid bootstrap stream')
        require(zlib.crc32(data) == struct.unpack_from('<I', part, packed - 4)[0], 'bootstrap CRC mismatch')
        blocks.append(dict(offset=cursor, compressed_bytes=packed, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
        if index == 1:
            require(len(data) == 441016, 'unexpected script size')
            script = data
        cursor += packed
    require(cursor == BASE, 'unexpected file data base')
    return script, blocks


def candidates(script, source_size):
    rows = []
    for match in re.finditer(rb'%[A-Z_]+%\\[^\x00\r\n]+\x00', script):
        offset = match.start() - 43
        if offset < 0:
            continue
        op, flags, start, end, date, time, size = struct.unpack_from('<BHIIHHI', script, offset)
        if op != 0 or flags != 128 or not (0 <= start < end <= source_size - BASE) or not (0 < size <= 1073741824):
            continue
        path = match.group()[:-1].decode('cp1251')
        crc = struct.unpack_from('<I', script, offset + 39)[0]
        rows.append(dict(script_offset=offset, path=path, start=start, end=end, bytes=size, crc32=crc))
    require(rows, 'no InstallFile candidates found')
    return rows


def validate_file(source, row, destination=None):
    decoder = zlib.decompressobj(-15)
    digest, crc, length = hashlib.sha256(), 0, 0
    begin, finish = BASE + row['start'], BASE + row['end']
    require(finish - begin >= 4, 'file stream too short')
    output = destination.open('xb') if destination else None
    try:
        cursor = begin
        while cursor < finish - 4:
            part = source.read(cursor, min(262144, finish - 4 - cursor))
            cursor += len(part)
            # Limit each inflated chunk; avoid retaining large game archives in RAM.
            pending = part
            while pending:
                data = decoder.decompress(pending, 1048576)
                pending = decoder.unconsumed_tail
                length += len(data)
                require(length <= row['bytes'], 'inflated data exceeds declared size')
                crc = zlib.crc32(data, crc)
                digest.update(data)
                if output:
                    output.write(data)
            # Some media records (including ar.exe) have a zero pad before CRC.
            require(len(decoder.unused_data) <= 3 and not any(decoder.unused_data), 'nonzero/excess stream padding')
        trailer = struct.unpack('<I', source.read(finish - 4, 4))[0]
        require(decoder.eof and length == row['bytes'], 'file EOF/size mismatch')
        require(crc == trailer == row['crc32'], 'file CRC mismatch')
        row['padding_bytes'] = len(decoder.unused_data)
        return digest.hexdigest()
    finally:
        if output:
            output.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('disc1', type=Path)
    parser.add_argument('disc2', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--installed', type=Path)
    parser.add_argument('--extract', action='append', default=[], help='Exact MAINDIR-relative file to retain')
    args = parser.parse_args()
    discs = [args.disc1.resolve(), args.disc2.resolve()]
    output = args.output.resolve()
    require(not output.exists(), 'output already exists')
    require(all(not output.is_relative_to(disc.parent) for disc in discs), 'output in media directory')
    if args.installed:
        require(not output.is_relative_to(args.installed.resolve()), 'output in installed game')
    require(tuple(sha_file(disc) for disc in discs) == ISO_HASHES, 'unsupported media fingerprint')
    selected = {name.replace('/', '\\').casefold() for name in args.extract}
    for name in selected:
        require(not name.startswith('\\') and not any(part in ('', '.', '..') or ':' in part for part in name.split('\\')),
                'unsafe extraction path')
    output.mkdir()
    members = [IsoMember(discs[0], 'Setup.exe'), IsoMember(discs[1], 'Setup.W02')]
    try:
        source = Composite(members)
        script, blocks = bootstrap(source)
        (output / 'script.bin').write_bytes(script)
        rows = candidates(script, source.size)
        retained, compared = set(), 0
        for index, row in enumerate(rows):
            prefix = '%MAINDIR%\\'
            relative = row['path'][len(prefix):] if row['path'].startswith(prefix) else None
            safe = relative and all(part not in ('', '.', '..') and ':' not in part for part in relative.split('\\'))
            destination = None
            if safe and relative.casefold() in selected:
                destination = output / 'files' / Path(*relative.split('\\'))
                destination.parent.mkdir(parents=True, exist_ok=True)
                retained.add(relative.casefold())
            row['sha256'] = validate_file(source, row, destination)
            row['crosses_volume_boundary'] = BASE + row['start'] < members[0].size < BASE + row['end']
            if args.installed and safe:
                installed = args.installed.resolve() / Path(*relative.split('\\'))
                require(installed.resolve().is_relative_to(args.installed.resolve()), 'comparison path escaped source')
                if installed.is_file():
                    row['installed_sha256'] = sha_file(installed)
                    row['installed_identical'] = row['installed_sha256'] == row['sha256']
                    compared += 1
            if (index + 1) % 500 == 0:
                print(f'Validated {index + 1}/{len(rows)} files', flush=True)
        require(retained == selected, 'requested file not found')
        require(tuple(sha_file(disc) for disc in discs) == ISO_HASHES, 'source media changed')
        report = dict(iso_sha256=ISO_HASHES, file_data_base=BASE, bootstrap=blocks, candidate_files=len(rows),
                      compared=compared, installed_identical=sum(row.get('installed_identical', False) for row in rows),
                      limitations=['InstallFile candidate scan, not full Wise VM/action coverage',
                                   'all extracted streams validated; no installer actions executed'], files=rows)
        (output / 'report.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
        print(f"PASS {len(rows)} CRC/EOF/size checks; {compared} installed comparisons; sources unchanged")
    finally:
        for member in members:
            member.file.close()


if __name__ == '__main__':
    main()
