#!/usr/bin/env python3
"""Read-only installed-game inventory. Output must be new and outside the game.

Uses only the standard library; never executes binaries or extracts archives.
SAVE, screenshots, backups and runtime logs are excluded from content analysis.
PFIL inspection decodes only a bounded prefix, not a full integrity check.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
import zipfile


def pfil_prefix(data, limit=32):
    if len(data) < 64 or data[:5] != b"PFIL@":
        raise ValueError("missing PFIL@ header")
    size = struct.unpack_from("<i", data, 16)[0]
    if size < 0:
        raise ValueError("negative PFIL length")
    wanted = min(size, limit)
    ring = bytearray(b" " * 4078 + b"\0" * 18)
    pos, cursor, out = 64, 4078, bytearray()
    while len(out) < wanted:
        if pos >= len(data):
            raise ValueError("truncated PFIL prefix")
        flags = data[pos]
        pos += 1
        for bit in range(8):
            if len(out) >= wanted:
                break
            count = 1 if flags & (1 << bit) else 2
            if pos + count > len(data):
                raise ValueError("truncated PFIL token")
            if count == 1:
                values = [data[pos]]
            else:
                lo, hi = data[pos:pos + 2]
                offset, length = lo | ((hi & 240) << 4), (hi & 15) + 3
                values = None
            pos += count
            for k in range(1 if values is not None else length):
                value = values[0] if values is not None else ring[(offset + k) & 4095]
                out.append(value)
                ring[cursor] = value
                cursor = (cursor + 1) & 4095
                if len(out) >= wanted:
                    break
    return size, bytes(out)


def signature(data):
    for prefix, name in [(b"PFIL@", "PFIL@"), (b"PK\x03\x04", "ZIP-local-header"),
                         (b"MZ", "DOS-MZ"), (b"BCI0", "BCI0"), (b"ALRA", "ALRA"),
                         (b"APAT", "APAT"), (b"ARCP", "ARCP"), (b"RIFF", "RIFF"), (b"BM", "BMP")]:
        if data.startswith(prefix):
            return name
    return "other"


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    root, output = args.game.resolve(), args.output.resolve()
    if not root.is_dir():
        parser.error("game directory does not exist")
    if output.is_relative_to(root) or output.exists():
        parser.error("output must be new and outside the game directory")
    records, excluded, errors = [], Counter(), []
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(root)
        if path.is_symlink() or path.resolve().is_relative_to(root) is False:
            excluded["links"] += 1
            continue
        if (rel.parts[0].upper() in {"SAVE", "SCRNSHOT", "TOENG", ".AGAINST-ROME-MODIFIER-LANGUAGE-BACKUP"}
                or path.suffix.lower() in {".bak", ".log", ".dmp", ".arm_original"}
                or path.suffix.lower().startswith(".dm")):
            excluded["save/screenshot/backup/runtime"] += 1
            continue
        try:
            with path.open("rb") as stream:
                prefix = stream.read(4096)
            record = {"path": rel.as_posix(), "bytes": path.stat().st_size,
                      "extension": path.suffix.lower(), "signature": signature(prefix)}
            if prefix.startswith(b"PFIL@"):
                size, payload = pfil_prefix(prefix)
                record.update(uncompressed_bytes=size, payload_signature=signature(payload),
                              payload_prefix_hex=payload[:16].hex())
            if path.suffix.lower() in {".exe", ".dll"} or len(rel.parts) == 1 and path.suffix.lower() == ".dat":
                record["sha256"] = sha256(path)
            if len(rel.parts) == 1 and prefix.startswith(b"PK\x03\x04"):
                with zipfile.ZipFile(path) as archive:
                    entries = archive.infolist()
                    samples = {}
                    for entry in entries:
                        ext = Path(entry.filename).suffix.lower()
                        group = samples.setdefault(ext, [])
                        if len(group) < 3 and not entry.is_dir():
                            with archive.open(entry) as stream:
                                head = stream.read(64)
                            group.append({"path": entry.filename, "signature": signature(head),
                                          "prefix_hex": head[:16].hex()})
                    record["archive"] = {"entries": len(entries),
                        "extensions": dict(Counter(Path(e.filename).suffix.lower() for e in entries)),
                        "uncompressed_bytes": sum(e.file_size for e in entries),
                        "encrypted_entries": sum(bool(e.flag_bits & 1) for e in entries),
                        "samples": samples, "crc_verified": False}
            records.append(record)
        except (OSError, ValueError, zipfile.BadZipFile, RuntimeError) as exc:
            errors.append({"path": rel.as_posix(), "error": str(exc)})
    report = {"root": str(root), "scope": "read-only shipped data plus custom maps; saves and backups excluded",
              "limitations": ["PFIL prefix only, not full decompression/integrity",
                              "ZIP central directory and at most 3 prefix samples per extension; no full CRC pass",
                              "Signatures and filenames do not establish runtime semantics"],
              "files": records, "excluded": dict(excluded), "errors": errors,
              "summary": {"files": len(records), "bytes": sum(r["bytes"] for r in records),
                          "extensions": dict(Counter(r["extension"] for r in records)),
                          "signatures": dict(Counter(r["signature"] for r in records)),
                          "payload_signatures": dict(Counter(r["payload_signature"] for r in records if "payload_signature" in r))}}
    with output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({"output": str(output), "summary": report["summary"], "errors": len(errors)}, ensure_ascii=False))
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
