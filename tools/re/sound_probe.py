#!/usr/bin/env python3
"""Read-only sound evidence probe. Inputs must be explicit TEMP/repository copies.
Never runs the executable. Does not write assets or modify its input directories.
Offsets: decoded PFIL payload for objdef/pools; BCI code relative (add 0x24);
PE virtual addresses for strings/disassembly. Requires only stdlib + bcitool;
--disassembly additionally requires pefile and capstone.
"""
import argparse
import hashlib
import re
import struct
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bcitool import Bci, pfil_decompress


def decoded(path):
    raw = path.read_bytes()
    return pfil_decompress(raw) if raw[:4] == b'PFIL' else raw


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compare', type=Path, required=True)
    parser.add_argument('--objdef', type=Path, required=True)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--disassembly', action='store_true')
    args = parser.parse_args()
    # Conservative guard against accidentally using the installed game.
    for path in (args.compare, args.objdef, args.exe):
        resolved = str(path.resolve()).replace('\\', '/').lower()
        if '/program files (x86)/against rome' in resolved:
            parser.error('Use authorized TEMP or repository copies only')
    wanted = ['sfxenv.dau', 'sfxobj.dau', 'sfxexp.dau', 'cl_sfx.ini', 'voice.ini']
    files = {p.name.lower() for p in args.compare.rglob('*') if p.is_file()}
    print('COMPARE SOUND TABLES', {name: name in files for name in wanted})
    for level in ('ENDL_000', 'ENDL_005'):
        root = args.compare / level
        print('DATA', level, sorted(p.name for p in (root / 'DATA').glob('*') if p.is_file()))
        script = root / 'SCRIPT/ak_level.bci'
        bci = Bci(script.read_bytes())
        print('BCI', level, sha(script), 'codeSize', bci.codeSize)
        words = bci.words()
        for index, name in enumerate(bci.names):
            if re.search(r'sound|voice|ambient|music', name, re.I):
                print('SYMBOL', index, name)
                for i in range(len(words) - 1):
                    if words[i:i+2] == [128, index]:
                        print('SITE', hex(i * 4), words[max(0, i-4):i+5])
        objects = decoded(root / 'DATA/objects.dat')
        version, count, name_width, id_width = struct.unpack_from('<4I', objects)
        assert (version, name_width, id_width) == (1, 30, 30)
        markers = []
        for slot in range(count):
            offset = 16 + slot * 79
            type_id = struct.unpack_from('<H', objects, offset+75)[0]
            if objects[offset] and 1753 <= type_id <= 1820:
                markers.append((slot, type_id, hex(offset)))
        print('ACTIVE SFXmark 1753..1820', markers)
        for name in ('action', 'flash', 'particle', 'light', 'engine'):
            path = root / 'DATA' / (name + '.dat')
            payload = decoded(path)
            print('POOL', level, name, sha(path), len(payload), payload[:16].hex(' '))
    text = decoded(args.objdef)
    print('OBJDEF', sha(args.objdef), 'decodedSize', len(text))
    lines = text.decode('latin1').splitlines()
    fields = [x.strip() for x in lines[1].lstrip(';').split(',')]
    for i, field in enumerate(fields):
        if re.search(r'sfx|sound|snd|wav', field, re.I):
            print('FIELD', i, field, hex(text.find(field.encode())))
    for row in lines[2:]:
        cells = [x.strip() for x in row.split(',')]
        if len(cells) > 159 and cells[0] in ('12', '184', '1753', '1764', '1773', '1778'):
            print('OBJ ROW', hex(text.find(row.encode())),
                  {fields[i]: cells[i] for i in (0, 52, 53, 54, 55, 68, 155, 159)})
    data = args.exe.read_bytes()
    print('EXE', sha(args.exe), len(data))
    peoff = struct.unpack_from('<I', data, 0x3c)[0]
    section_count = struct.unpack_from('<H', data, peoff+6)[0]
    opt_size = struct.unpack_from('<H', data, peoff+20)[0]
    base = struct.unpack_from('<I', data, peoff+52)[0]
    sections = [struct.unpack_from('<8sIIII', data, peoff+24+opt_size+i*40)
                for i in range(section_count)]
    def va(offset):
        for _, _, rva, size, raw in sections:
            if raw <= offset < raw+size:
                return base+rva+offset-raw
        return base+offset
    pattern = r'sfxenv|sfxobj|sfxexp|cl_sfx|SfxNames|SfxEnviroment|SfxObjects|SfxExplosion|Soundevent|ODOBJ_SOUNDEVENT|SFX_ACTION_PERMANENT|s_playVoice|s_sendVoice|s_setVoice|sound\.dat|SNDZ|Amb_Forest|Amb_River'
    for match in re.finditer(rb'[ -~]{4,}', data):
        value = match.group().decode('ascii')
        if re.search(pattern, value, re.I):
            print('STRING', hex(match.start()), hex(va(match.start())), value)
    if args.disassembly:
        import pefile
        from capstone import Cs, CS_ARCH_X86, CS_MODE_32
        pe = pefile.PE(data=data)
        decoder = Cs(CS_ARCH_X86, CS_MODE_32)
        # Start at verified function/instruction boundaries, not arbitrary byte offsets.
        for start, end in [(0x480e9e,0x480ea8),(0x480ec9,0x480ed3),
                           (0x480ef4,0x480efe),(0x4e1260,0x4e12bc),
                           (0x4e1470,0x4e15a4),(0x4df349,0x4df37a),
                           (0x521fb0,0x521fcf),(0x522000,0x52201a),
                           (0x520ed0,0x520ef7),(0x52228a,0x5222d9)]:
            offset = pe.get_offset_from_rva(start-base)
            print('RANGE', hex(start), hex(end))
            for ins in decoder.disasm(data[offset:offset+end-start], start):
                print(hex(ins.address), ins.bytes.hex(' '), ins.mnemonic, ins.op_str)


if __name__ == '__main__':
    main()
