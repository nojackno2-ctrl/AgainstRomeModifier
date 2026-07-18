#!/usr/bin/env python3
"""bcitool - reader/disassembler for Against Rome `BCI0` script bytecode.

Recreates the throwaway helper referenced in
`docs/reverse-engineering/bci0-opcodes.md` (which noted the original tool was
never committed). Pure standard library; no third-party deps.

The game ships each script as a `PFIL@` LZSS-wrapped `BCI0` container under
`SYSTEM/CLAK/SCRIPT/*.bci` and `MAPS/**/SCRIPT/*.bci`. This tool decompresses
the wrapper, parses the container, and disassembles the code stream using the
opcode table documented in `docs/reverse-engineering/bci0-opcodes.md`.

Container layout (little-endian 32-bit words; see the doc for provenance):
  0x00 "BCI0"        0x08 codeSize      0x10 blobLen     0x14 symCount
  0x24 code stream (codeSize bytes)  -> all "decompressed offset" refs are
                                        relative to this base (byte 0 = 0x24)
  then "SYMBCONS" + blob(blobLen) + "CIDX" + symCount*int32 offsets
  then "VAR " ... "VIDX" ... (locals metadata, not decoded)

Subcommands:
  dec   <file> [out]          PFIL decompress to raw BCI0 bytes
  syms  <file>                dump external symbol table (index -> name)
  calls <file>               scan pushsym(128)+argc+call(86) sites, resolve names
  find  <file> <symbol>       list code offsets that pushsym a given symbol
  funcs <file>                list function prologue offsets (95 75 121 74 94)
  dis   <file> [start] [end]  linear disassembly (offsets are code-stream bytes)

Offsets printed by `dis`/`find`/`calls` are code-stream relative (add 0x24 to
get the byte offset inside the decompressed `.dec` container file).
"""
import sys, os, struct

# --- PFIL LZSS (same algorithm as GameLZSS.DecompressPfil) --------------------
def pfil_decompress(data: bytes) -> bytes:
    if data is None or len(data) < 64:
        return data or b""
    usize = int.from_bytes(data[16:20], "little", signed=True)
    out = bytearray(usize)
    ip, op = 64, 0
    ring = bytearray(4096)
    for x in range(4078):
        ring[x] = 0x20
    r = 4078
    n = len(data)
    while ip < n and op < usize:
        flags = data[ip]; ip += 1
        for _ in range(8):
            if ip >= n or op >= usize:
                break
            if flags & 1:
                b = data[ip]; ip += 1
                out[op] = b; op += 1
                ring[r] = b; r = (r + 1) & 4095
            else:
                if ip + 1 >= n:
                    break
                p1, p2 = data[ip], data[ip + 1]; ip += 2
                offset = p1 | ((p2 & 0xF0) << 4)
                length = (p2 & 0x0F) + 3
                for k in range(length):
                    b = ring[(offset + k) & 4095]
                    if op < usize:
                        out[op] = b; op += 1
                    ring[r] = b; r = (r + 1) & 4095
            flags >>= 1
    return bytes(out)

def pfil_store(payload: bytes, header_template: bytes) -> bytes:
    """Repack payload into a PFIL@ wrapper using store mode (all literals).

    Keeps the original 64-byte header, overwriting only the uncompressed size
    at 0x10. Store mode emits a 0xFF flag byte followed by 8 literal bytes, which
    the decompressor above reproduces byte-for-byte. NOTE: header fields at
    0x20+ are not fully understood; if the loader validates them this repack may
    be rejected -- test in-game before trusting.
    """
    hdr = bytearray(header_template[:64])
    struct.pack_into("<I", hdr, 0x10, len(payload))
    out = bytearray(hdr)
    i = 0
    while i < len(payload):
        chunk = payload[i:i + 8]
        out.append(0xFF)
        out.extend(chunk)
        i += 8
    return bytes(out)

# --- container parsing --------------------------------------------------------
class Bci:
    def __init__(self, raw: bytes):
        data = pfil_decompress(raw) if raw[:4] == b"PFIL" else raw
        assert data[:4] == b"BCI0", "not a BCI0 container"
        self.data = data
        self.codeSize = struct.unpack_from("<I", data, 0x08)[0]
        self.blobLen = struct.unpack_from("<I", data, 0x10)[0]
        self.symCount = struct.unpack_from("<I", data, 0x14)[0]
        self.code = data[0x24:0x24 + self.codeSize]
        p = 0x24 + self.codeSize
        assert data[p:p + 8] == b"SYMBCONS", data[p:p + 8]
        self.blob = data[p + 8:p + 8 + self.blobLen]
        q = p + 8 + self.blobLen
        assert data[q:q + 4] == b"CIDX", data[q:q + 4]
        self.cidx = list(struct.unpack_from("<%dI" % self.symCount, data, q + 4))
        self.tail = data[q + 4 + 4 * self.symCount:]  # VAR / VIDX blocks
        self.names = []
        for o in self.cidx:
            e = self.blob.find(b"\x00", o)
            self.names.append(self.blob[o:e].decode("latin1"))

    def words(self):
        return list(struct.unpack_from("<%di" % (len(self.code) // 4), self.code, 0))

    def sym_indices(self, name):
        return [i for i, n in enumerate(self.names) if n == name]

# --- opcode table (see bci0-opcodes.md) --------------------------------------
OP1 = {64, 65, 66, 68, 69, 70, 73, 76, 77, 78, 80, 81, 82, 83, 84, 90, 91, 92, 93,
       112, 113, 114, 115, 116, 117, 118, 120, 128, 129, 160}
OP2 = {67}
OPNAME = {32: "add", 33: "sub", 34: "mul", 35: "div", 37: "bitop", 40: "or", 41: "or2", 44: "and/comb",
          66: "pushlit", 71: "pop", 73: "argc", 74: "p74", 75: "p75",
          80: "pushsym80", 81: "pushvar", 82: "storevar", 86: "call",
          90: "arrbase", 91: "arrstore", 94: "p94", 95: "p95", 96: "cmp0",
          98: "lt", 99: "le", 100: "gt", 101: "ge", 102: "eq", 103: "ne",
          112: "jmp", 113: "jlt", 114: "jle", 115: "jgt", 116: "jge",
          117: "jz", 118: "jnz", 120: "callint", 121: "ret", 128: "pushsym",
          160: "arrcreate", 163: "arrrd", 164: "arrwr", 77: "pushglobref"}
PROLOGUE = [95, 75, 121, 74, 94]

def disasm(bci, start_word, end_word):
    w = bci.words()
    i = start_word
    rows = []
    while i < end_word:
        op = w[i] & 0xffffffff
        off = i * 4
        if op in OP2:
            rows.append((off, op, f"{OPNAME.get(op, 'op%d' % op)} {w[i+1]}, {w[i+2]}"))
            i += 3
        elif op in OP1:
            a = w[i + 1]
            if op == 128:
                nm = bci.names[a] if 0 <= a < len(bci.names) else "??"
                txt = f"pushsym #{a} <{nm}>"
            elif op in (112, 113, 114, 115, 116, 117, 118):
                txt = f"{OPNAME.get(op)} ->{off + 8 + a:#06x} (op {a})"
            elif op == 120:
                txt = f"callint ->{off + 8 + a:#06x} (op {a})"
            elif op == 66:
                txt = f"pushlit {a}"
            elif op == 81:
                txt = f"pushvar v{a}"
            elif op == 82:
                txt = f"storevar v{a}"
            elif op == 73:
                txt = f"argc {a}"
            else:
                txt = f"{OPNAME.get(op, 'op%d' % op)} {a}"
            rows.append((off, op, txt))
            i += 2
        else:
            rows.append((off, op, OPNAME.get(op, "op%d" % op)))
            i += 1
    return rows

def find_funcs(bci):
    w = bci.words()
    return [i * 4 for i in range(len(w) - 4)
            if w[i:i + 5] == PROLOGUE]

def scan_calls(bci):
    w = bci.words()
    i, out = 0, []
    while i < len(w):
        op = w[i]
        if op == 128:
            a = w[i + 1]
            nm = bci.names[a] if 0 <= a < len(bci.names) else "??"
            out.append((i * 4, a, nm))
            i += 2
        elif op in OP2:
            i += 3
        elif op in OP1:
            i += 2
        else:
            i += 1
    return out

# --- CLI ----------------------------------------------------------------------
def _read(path):
    return open(path, "rb").read()

def main(argv):
    if len(argv) < 3:
        print(__doc__); return 1
    cmd, path = argv[1], argv[2]
    if cmd == "dec":
        raw = _read(path)
        dec = pfil_decompress(raw) if raw[:4] == b"PFIL" else raw
        out = argv[3] if len(argv) > 3 else path + ".dec"
        open(out, "wb").write(dec)
        print(f"wrote {len(dec)} bytes -> {out}")
        return 0
    bci = Bci(_read(path))
    if cmd == "syms":
        for i, n in enumerate(bci.names):
            print(f"#{i:<4d} {n}")
    elif cmd == "funcs":
        offs = find_funcs(bci)
        print(f"{len(offs)} functions:", " ".join(hex(o) for o in offs))
    elif cmd == "calls":
        starts = find_funcs(bci)
        for off, idx, nm in scan_calls(bci):
            fn = max([s for s in starts if s <= off], default=None)
            fnx = f"{fn:#08x}" if fn is not None else "----"
            print(f"{off:#08x}  #{idx:<3d} {nm:28s} (func {fnx})")
    elif cmd == "find":
        want = argv[3]
        ids = set(bci.sym_indices(want))
        for off, idx, nm in scan_calls(bci):
            if idx in ids:
                print(f"{off:#08x}  {nm}")
    elif cmd == "dis":
        w = bci.words()
        s = int(argv[3], 0) // 4 if len(argv) > 3 else 0
        e = int(argv[4], 0) // 4 if len(argv) > 4 else len(w)
        for off, op, txt in disasm(bci, s, e):
            print(f"{off:#08x}: {txt}")
    else:
        print(f"unknown command: {cmd}"); return 1
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
