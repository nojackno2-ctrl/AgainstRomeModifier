#!/usr/bin/env python3
"""Verify ARCP/PUA metadata and decoded CRCs without extracting members.

XOR table is read from the caller-supplied, fingerprint-checked game EXE.
No bundled game data or key table; inputs are never modified.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
import zlib
from inventory_game_directory import pfil_prefix

EXE_SHA256 = "6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def xor_table(exe):
    require(hashlib.sha256(exe).hexdigest() == EXE_SHA256, "unsupported EXE fingerprint")
    nt = struct.unpack_from("<I", exe, 0x3c)[0]
    require(exe[:2] == b"MZ" and exe[nt:nt + 4] == b"PE\0\0", "not a PE executable")
    count = struct.unpack_from("<H", exe, nt + 6)[0]
    opt_size = struct.unpack_from("<H", exe, nt + 20)[0]
    opt = nt + 24
    require(struct.unpack_from("<H", exe, opt)[0] == 0x10b, "expected PE32")
    base = struct.unpack_from("<I", exe, opt + 28)[0]
    rva = 0x62f060 - base
    for index in range(count):
        section = opt + opt_size + index * 40
        va, raw_size, raw_offset = struct.unpack_from("<III", exe, section + 12)
        if va <= rva and rva + 64 <= va + raw_size:
            offset = raw_offset + rva - va
            require(offset + 64 <= len(exe), "XOR table outside source bytes")
            return exe[offset:offset + 64]
    raise ValueError("XOR table is not file-backed")


def inspect(data, key):
    require(len(data) >= 58, "short PUA header")
    header = struct.unpack_from("<14I2B", data)
    require(header[0] == 0x50435241, "not an ARCP archive")
    require(header[4] == 58 and header[6] == 28, "unsupported header or record layout")
    count, table, names_offset = header[8], header[4], header[10]
    require(count <= (len(data) - table) // 28, "entry table out of bounds")
    table_end = table + count * 28
    names_end = names_offset + header[11] + header[12]
    require(table_end <= names_offset <= names_end <= len(data), "name table out of bounds")
    checksum_header = bytearray(data[:58])
    checksum_header[20:24] = b"\0" * 4
    header_crc = zlib.crc32(data[table:table_end], zlib.crc32(checksum_header))
    require(header_crc == header[5], "header/entry-table CRC mismatch")
    names_blob = data[names_offset:names_end]
    require(zlib.crc32(names_blob) == header[13], "names CRC mismatch")
    strings = names_blob.split(b"\0")
    require(len(strings) == count + 3 and strings[-1] == b"", "name count/terminator mismatch")
    records = []
    for index, name in enumerate(strings[2:-1]):
        fields = struct.unpack_from("<6I2H", data, table + index * 28)
        offset, _, _, size, stored_size, crc, flags, extra_flags = fields
        require(size <= 64 * 1024 * 1024, "member exceeds analysis limit")
        require(names_end <= offset <= offset + stored_size <= len(data), "payload out of bounds")
        require(flags & ~3 == 0 and extra_flags == 0, "unsupported member flags")
        payload = data[offset:offset + stored_size]
        if flags & 2:
            wrapper = bytearray(64)
            wrapper[:5] = b"PFIL@"
            struct.pack_into("<I", wrapper, 16, size)
            _, payload = pfil_prefix(bytes(wrapper) + payload, limit=size)
        if flags & 1:
            payload = bytes(value ^ key[i & 63] for i, value in enumerate(payload))
        require(len(payload) == size, "decoded length mismatch")
        require(zlib.crc32(payload) == crc, "decoded CRC mismatch at entry " + str(index))
        records.append({"index": index, "path": name.decode("cp1251"),
                        "path_bytes_hex": name.hex(), "offset": offset,
                        "bytes": size, "stored_bytes": stored_size, "flags": flags,
                        "crc32": f"{crc:08x}", "sha256": hashlib.sha256(payload).hexdigest(),
                        "unknown_record_words": list(fields[1:3])})
    return {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data),
            "header_words_and_bytes": list(header), "header_crc_verified": True,
            "names_crc_verified": True, "decoded_member_crcs_verified": len(records),
            "extensions": dict(Counter(Path(r["path"]).suffix.lower() for r in records)),
            "flags": dict(Counter(r["flags"] for r in records)), "members": records,
            "limitations": ["cp1251 is a display convention; path bytes retained separately",
                            "unknown record words, version bytes and archive lookup/runtime semantics not inferred",
                            "no archive payloads extracted or executed"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pua", type=Path)
    parser.add_argument("exe", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    # Permit only a sibling external workspace, never write into either input tree.
    if output.exists() or any(output.is_relative_to(p.resolve().parent) for p in (args.pua, args.exe)):
        parser.error("output must be new and outside both input directories")
    try:
        result = inspect(args.pua.read_bytes(), xor_table(args.exe.read_bytes()))
        with output.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
        print(json.dumps({k: v for k, v in result.items() if k != "members"}, ensure_ascii=False))
        return 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
