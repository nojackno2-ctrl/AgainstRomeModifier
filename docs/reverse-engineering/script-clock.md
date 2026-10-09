# Script clock capture and rebase

Status: static-verified data flow; rollback cause unresolved, 2026-10-09.
REA Ghidra 12.1.4 analyzed the EXE identity recorded in
`installed-game-directory.md`, which matches the installed game.
Full inline evidence is local-only in
`re_workspace/clock-20261009-evidence-final.json` (25 records).

## Observed arithmetic

| Address | Observed behavior | REA evidence |
| --- | --- | --- |
| 0x5098d0 | Returns `FUN_00566b90() - [0x2143f7c]` | ev_3b211f1673cfe29371c255a94f64c2204c71a39b50986b524aee2193083b2d25 |
| 0x5098f0 | Writes `[0x2143f7c] = FUN_00566b90() - argument` | ev_ddacb8420d8541858b1cb5f409e02c67560e431746474bb6521a6818ea7d2d09 |
| 0x52f860 | Captures the first getter into `[0x2662648]` | ev_a8210304119cdac5cd26139579a0283155e07b463786010845e2924b1a68aefd |
| 0x52f870 | Passes `[0x2662648]` to the rebase setter | ev_73071cb02c73184399bc384ea3995398d5920ba6ddd41f21bbce2029c3feecdb |
| 0x52fd70 | Version-0x14 reader restores this global as its third scalar | ev_dc172b9050d3f344a72b43ef4bc1ca7c3e259c8415be184e77617f7ac8c47667 |
| 0x452100 | Argument 1 captures via thunk 0x420f20; other arguments rebase via thunk 0x420f30 | ev_38960802d048847b68a981c3d48a6882ebeeb023959946be9b268b0a7ea28416 |

The setter intentionally changes an epoch; the stored global is not itself
the continuously advancing getter. To establish the script time after loading,
the capture/rebase ordering must therefore be observed, not inferred solely
from the stored scalar. The version reader does not by itself prove the
previously documented absolute `scr.dat` offset 0x80 for all saves.

The lower getter at 0x566b90 subtracts `[0x29fa2ac]` from 0x566b50.
0x566b50 converts `FUN_0055e530() * 1e-6` to an integer.
0x55e530 selects a QPC-derived path (0x55def0) or
`(timeGetTime() - [0x29e89dc]) * [double 0x60424c]`.
Read bytes establish original constants: double 0x604214 = 1e9,
double 0x60424c = 1e6, double 0x606733 = 1e-6.
This current EXE has original clock scale constants, rather than the 10x
constants described in the historical paired-save incident.

0x55ab50 also adjusts `[0x29fa2ac]` by elapsed lower-clock ticks across an
activation-state transition and contains the diagnostic `Puse is %s active`.
This supports an activation/pause-time compensation interpretation, but does
not establish a save/load defect. Evidence:
`ev_01d1442bb2234b61d77028894bc644f83bc31ed5a35529754438901a8266e372`.

## Limits and next probes

- The global 0x2662648 has five exact analyzed references: 0x52f865,
  0x52f870, 0x52fae3, 0x52fcd8 and 0x52fdd4. 0x52fae0 clears it.
  Ghidra did not identify a containing procedure for 0x52fcd8; raw byte reads
  show the reference, but its writer/serializer role is not yet established.
- Resolve callers of 0x452100 and the unidentified serialization region, then
  locate capture, file write/read, and rebase order at save/load boundaries.
- Compare getter, stored scalar, epoch and pending BCI deadlines during a
  controlled original-speed versus 10x save/load run. No runtime probe or
  original-game write was performed in this investigation.
- Existing `endless-mode-ai.md` paired-save evidence and P6 repair remain
  historical evidence. These observations do not prove the rollback cause or
  validate the repaired script in a new runtime session.
