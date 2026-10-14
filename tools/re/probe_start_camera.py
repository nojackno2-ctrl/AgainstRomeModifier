import os, glob, sys, struct
sys.path.insert(0, 'tools')
import bcitool

game_dir = r'C:\Program Files (x86)\Against Rome\MAPS'
target_maps = ['ENDL_000', 'ENDL_005', 'KAMP_002_i', 'KAMP_024', 'MP_001']

for m in target_maps:
    path = os.path.join(game_dir, m, 'SCRIPT', 'ak_level.bci')
    if not os.path.exists(path):
        path = os.path.join(game_dir, m, 'script', 'ak_level.bci')
    if not os.path.exists(path):
        continue
    raw = open(path, 'rb').read()
    b = bcitool.Bci(raw)
    idxs = b.sym_indices('s_centerToPos')
    print(f'=== {m} ak_level.bci ===')
    dtor = struct.unpack_from('<I', b.data, len(b.data)-8)[0]
    main = struct.unpack_from('<I', b.data, len(b.data)-4)[0]
    print(f'dtor: {hex(dtor)}, main: {hex(main)}')
    for idx in idxs:
        for o, op, txt in bcitool.disasm(b, 0, len(b.code)//4):
            if op == 128 and f'<{b.names[idx]}>' in txt:
                print(f'Call at offset {hex(o)}')
                start = max(0, o//4 - 25)
                end = min(len(b.code)//4, o//4 + 10)
                for sub_o, sub_op, sub_txt in bcitool.disasm(b, start, end):
                    print(f'  {hex(sub_o)}: {sub_txt}')
