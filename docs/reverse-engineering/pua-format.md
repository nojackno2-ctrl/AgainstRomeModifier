# PUA / ARCP archive format

Status: native-consumer and full installed-sample CRC verified, 2026-10-09.
Source `cl.pua`: 451,789 bytes, SHA-256
`40c782d98d6861953b8e2da8db4412ccb933204db8b5ea91f5fe969d5e2e5fbb`.
This is distinct from ZIP and PFIL, though compressed members reuse the
LZSS ring model. No archive members were extracted to disk or executed.

## Header and record layout

The decoder at `0x5642e0` reads fourteen little-endian uint32 words followed by
two bytes, for a serialized header length of 58. Do not read the trailing two
bytes plus the first record as a fifteenth header word.
The native loader at `0x5645f0` checks magic `ARCP` (`0x50435241`).

| Header offset | Observed use | Installed value |
| --- | --- | ---: |
| 0x00 | ARCP magic | 0x50435241 |
| 0x10 | Record-table start / serialized header length | 58 |
| 0x14 | CRC of header with this word zeroed, continued through records | 0x9205e197 |
| 0x18 | Serialized entry stride | 28 |
| 0x20 | Entry count | 169 |
| 0x24 | Matches actual archive byte length; further semantics unproven | 451789 |
| 0x28 | Name-blob offset | 4790 |
| 0x2c, 0x30 | Two name-blob size portions (combined before reading) | 31, 4630 |
| 0x34 | Combined name-blob CRC | 0xfc0a685f |
| other words, 0x38/39 bytes | Kept uninterpreted | See probe manifest |

The name blob contains two leading NUL-terminated strings, then 169 member
names in record order. The leading strings in this sample are an archive name
and a comment; their general semantics are not required by the verifier.

Each serialized record is six little-endian uint32 words and two uint16 words:

| Entry offset | Native-consumer meaning |
| --- | --- |
| 0x00 | Stored payload file offset |
| 0x04, 0x08 | Unknown words; do not label them timestamps without evidence |
| 0x0c | Decoded byte length |
| 0x10 | Stored byte length |
| 0x14 | Decoded payload CRC32 |
| 0x18 | Flags: bit 1 (`2`) LZSS; bit 0 (`1`) XOR transform |
| 0x1a | Extra flags, zero in all inspected records; other values unsupported |

`0x564b20` copies these fields into member handles. `0x564d10` reads raw or
LZSS bytes, applies XOR if bit 0 is set, and accumulates CRC over decoded bytes.
XOR at `0x564fa0` cycles a 64-byte table at EXE VA `0x62f060`, indexed by
decoded stream offset modulo 64. The tool reads it from a fingerprint-checked
caller-supplied EXE rather than bundling game bytes.

Native CRC function `0x585080` agrees with standard `zlib.crc32` for both
metadata and all decoded members in this sample. LZSS initialization at
`0x565c00` uses a 4,096-byte ring, first 4,078 bytes spaces, cursor 0xfee.

## Verification and content families

The new standard-library `probe_pua.py` verifies metadata CRCs, table/payload
bounds, lengths, supported flags, and every decoded payload CRC without
writing payload data. All 169 passed: 166 LZSS-only (`flags=2`) and 3 XOR-only
(`flags=1`). The XOR members are `SYSTEM/CLMK/DLG/LIT/pal01.tga`,
`SYSTEM/version.ini`, and `USER/edit.cfg`.
Their raw CRC mismatches before the transform were not archive corruption.

Central-name inventory: 68 `.dlg`, 47 `.put`, 24 `.tga`, 10 `.ini`, 8 `.pur`,
4 `.kor`, 3 each `.tab`/`.cfg`, 1 each `.pos`/`.lst`.
Names alone do not establish how all these resources are consumed.

```powershell
python tools/re/probe_pua.py `
  'C:\Program Files (x86)\Against Rome\cl.pua' `
  'C:\Program Files (x86)\Against Rome\Against_Rome.exe' `
  re_workspace/pua-new.json
```

Output must be new and outside both input directories. Local manifest:
`re_workspace/pua-20261009.json`. Additional checks: independent header/name
and decoded CRC agreement; in-memory mutations to header, name table, LZSS
payload and XOR payload are rejected. Existing-output refusal and Python
compilation passed. This does not verify a writable repacker, runtime lookup,
other PUA variants, or unrelated game files.

Native evidence (REA bundle `virtual-file-final-20261009-evidence.json`):
loader `ev_7c290aa139c0ab7bf424c2f46d8821233a11dab1ae32653bc230d83b6d4d49c0`;
member read `ev_4ed1b4e92f83b4d135733c1f0bac3571a4c70d17465bc4dd178adedc77b14e5a`;
XOR `ev_d719c873132235c5a364e907feb334b3de30a1491a4c0b23d8353a5c19c1ea2b`.
