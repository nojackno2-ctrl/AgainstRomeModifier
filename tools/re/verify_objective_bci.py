#!/usr/bin/env python3
"""Verify a raw/PFIL objective injection using the repository bcitool decoder.
Usage: python tools/re/verify_objective_bci.py ORIGINAL INJECTED
Read-only. Neither input is rewritten. This is static validation, not gameplay.
"""
import hashlib
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bcitool


def verify(original_path, injected_path):
    original = bcitool.Bci(Path(original_path).read_bytes())
    injected = bcitool.Bci(Path(injected_path).read_bytes())
    rows = bcitool.disasm(injected, 0, len(injected.words()))
    boundaries = {offset for offset, _, _ in rows}
    words = injected.words()
    for offset, op, _ in rows:
        if op in range(112, 119) or op == 120:
            target = offset + 8 + words[offset // 4 + 1]
            assert target in boundaries, f"unaligned/invalid branch {offset:#x} -> {target:#x}"
    # Original wait push becomes the only changed eight-byte trampoline.
    changed = [i for i, (a, b) in enumerate(zip(original.code, injected.code)) if a != b]
    assert changed, "no original wait hook replaced"
    hook = min(changed) // 4 * 4
    assert struct.unpack_from("<i", injected.code, hook)[0] == 112
    assert injected.code[:hook] == original.code[:hook]
    assert injected.code[hook + 8:len(original.code)] == original.code[hook + 8:]
    assert len(injected.code) > len(original.code)
    assert injected.tail[:-4] == original.tail[:-4], "variables or destructor changed"
    main = struct.unpack_from("<i", injected.tail, len(injected.tail) - 4)[0]
    assert main >= len(original.code) and main in boundaries
    assert injected.names[:len(original.names)] == original.names
    calls = bcitool.scan_calls(injected)
    added = [(off, name) for off, _, name in calls if off >= len(original.code)]
    for native in ("s_getTime", "s_getObjPos", "s_setScriptVarL", "s_quitGame"):
        assert any(name == native for _, name in added), f"missing {native}"
    assert "GLOBAL_MISSION_RESULT" in injected.names
    assert not any(name == "s_lgcSetMissionResult" for _, name in added)
    print(f"PASS: {len(rows)} instructions; all branches valid; hook {hook:#x}; original code/metadata preserved")
    for off, native in added:
        if native in ("s_getObjPos", "s_quitGame"):
            print(f"{off:#08x}: {native}")
    print("injected SHA256:", hashlib.sha256(Path(injected_path).read_bytes()).hexdigest())


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    verify(sys.argv[1], sys.argv[2])
