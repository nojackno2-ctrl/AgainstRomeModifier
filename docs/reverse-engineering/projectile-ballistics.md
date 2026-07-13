# Projectile Ballistics And Hit Determination

Research notes (2026-07-10; status reviewed 2026-07-11). Static analysis of `Against_Rome.exe` (Ghidra
pseudocode inventory + capstone disassembly) plus decompressed config tables.
Not yet runtime-verified; no product code changed.

## Data Chain (weapon → flying arrow)

For a ranged weapon slot `wN` in `objdef.dau`:

```
wN_expl ──> expldef.dau row (e.g. 15 = ED_RomSch01_Pfeil)
              └─ objd00 ──> objdef row (e.g. 830 = ParRomSch01_Pfeil, the flying entity)
                              ├─ papar (col 64) ──> partpar.dau  "Pfeil00" (idx 22): timer=15000ms, floorc=1
                              ├─ pageo (col 65) ──> partgeo.dau  "Pfeil00" (idx 23): heigh=15, ysub=89  (16.16 fixed)
                              └─ pagfx (col 66) ──> partgfx.dau  "Pfeil00" (idx 24): obdef=227 arrow mesh
```

The three `part*.dau` tables have independent numbering; rows correspond by
name, objdef references them by each table's own index.

## Launch (FUN_004bb770, VA 0x4bb770)

Weapon-array bases inside the objdef in-memory row (stride 0x2a4, row base +
field-array base, weapon index `w` scales ×4 or ×2):

| array | base | type |
|---|---|---|
| w_akti | 0xc6497c | int16[8] |
| w_dam  | 0xc6498c | float[8] |
| w_rad1 | 0xc649ac | float[8] |
| w_rad2 | 0xc649cc | float[8] |
| w_angl | 0xc649ec | float[8] |
| w_expl | 0xc64a0c | int32[8] |
| w_relt | 0xc64a2c | int32[8] |
| w_emit | 0xc64a4c | int32[8] (16.16 fixed) |
| w_drad | 0xc64a6c | int32[8] |
| w_dtim | 0xc64a8c | int32[8] |
| w_dtyp | 0xc64aac | int16[8] |

Velocity computed at 0x4bbc9d–0x4bbd80 with aim deltas `d = aim − shooter`:

- horizontal: `vx = d.x * 16384.0 * ProjectileInitSpeedFactor` (same for z;
  16384/65536 = 0.25 in 16.16) → **flight speed scales with distance; the
  horizontal arrival time is constant ≈ 65536/(16384·ISF) = 4/ISF game
  seconds** (2.67 s at the shipped ISF 1.5).
- vertical: `vy = w_emit + round(d.y * 16384.0 * ISF)` — **`w_emit` is the
  vertical launch speed** (misleading column name), plus a height-difference
  correction. Shipped values: all bows/spears 7208960 (=110.0), Speerschleuder
  6356992 (=97), GerArt Katapult 10158080 (=155), RomArt 9306112 (=142).
- spawn position: shooter pos, y+1.0; the partgeo `heigh` (15 for Pfeil00)
  adds visual start height.
- damage computed by FUN_004e0a70: `dam' = w_dam * (1 + gloryLevel*factor)`,
  ×DonnerschlagDAMfaktor / ×BerserkerDAMfaktor when those states are active
  (weapon 0 only).
- projectile instance created by FUN_004e1a80 (13 args: expl idx, team, pos,
  velocity, damage, target info, dtyp); FUN_004e1c10 processes the ExplDef
  slots and calls FUN_004a9740 to create the flying object with the launch
  velocity.

## Flight And Impact (FUN_004dbe60, particle integrator)

Per tick (instance arrays at 0x01235630/730/830 = x/y/z, 0x01235930/a30/b30 =
vx/vy/vz):

- `pos += vel*dt`; `vy -= ysub*dt` (ysub from partgeo table 0x819fcc, stride
  0x58, ysub at +0x24; the only consumer of that field).
- impact when `y <= floorHeight(x,z)` (FUN_0049bd50 heightfield — this is what
  obstacles/terrain "block" with) or lifetime (partpar timer) expires.
- on impact, FUN_004e2130 creates a damage zone at the landing point with
  radius `w_drad`, duration `w_dtim`, the launch-computed damage and `w_dtyp`.
- FUN_004e2200 ticks damage zones: applies damage to every object whose
  distance (FUN_004be360 mode 3) < radius, team ≠ shooter team, not dead, and
  passing the dtyp check FUN_004c1220. **No to-hit roll exists — hits are
  purely geometric.**

Ballistic closure: time to return to launch height is `2*emit/ysub`
(bows: 2·110/89 ≈ 2.47 s) which is deliberately tuned just under the
horizontal arrival time 4/ISF ≈ 2.67 s; the ~0.2 s difference is absorbed by
the extra fall from spawn height (heigh 15 + 1). Arc apex ≈ `emit²/(2·ysub)`
≈ 68 height units for bows, independent of shot distance.

## Aiming: lead prediction and scatter (in FUN_004bb770)

Only applied when the target is moving (has a move destination ≠ current pos):

- lead offset = direction-to-destination × `3.0 × 64.0 × moveSpeed`
  (constants 0x5f5fa2/0x5f5fa6), i.e. the epara-documented "3×MoveSpeed".
- lead is DISABLED if the angle between current and predicted position exceeds
  `ProjectileVarianceMaximumAngle` (45°) or predicted/current distance ratio
  is outside `1 ± ProjectileVarianceDistanceRange` (0.4).
- random scatter: up to `max(|lead.x|,|lead.z|) × min(1, moveSpeed/1280) ×
  ProjectileVarianceOnMove` added to the aim point (each axis independently).

Stationary targets get no scatter; long-range misses on them come from the
~2.5 s constant flight time (target micro-movement) combined with the small
`w_drad` (arrows 45, spear throwers/artillery 80 — under one pattern cell of
64) and the residual land-short margin that grows linearly with distance.

## cl_epara.ini knobs (loader at pseudocode line ~72400)

| key | global | shipped |
|---|---|---|
| ProjectileInitSpeedFactor | 0x771ca0 | 1.5 |
| ProjectileVarianceOnMove | 0x771ca4 | 0.5 |
| ProjectileVarianceMaximumAngle | 0x771cac | 45 |
| ProjectileVarianceDistanceRange | 0x771cb0 | 0.4 |

The ini comment "correct trajectory length by multiplying Ysub by 1.5²"
matches the model: horizontal time ∝ 1/ISF, so gravity must scale ∝ ISF² (and
emit ∝ ISF) to keep the same landing point.

## Modification Recipes (derived, not yet runtime-tested)

Let `k` = arc-raise factor, `s` = speed-up factor:

- **Raise arc only (same landing point, same flight time):** `w_emit ×k` in
  objdef AND `ysub ×k` in partgeo for the matching projectile (Pfeil00,
  Wurfspeer00, Wurfaxt00, Katapultstein00/01…). Apex scales ×k.
  E.g. k=1.5: bows emit 7208960→10813440, Pfeil00 ysub 5832704→8749056.
- **Faster flight (less dodge, same arc & landing):** ISF ×s in cl_epara,
  `w_emit ×s`, `ysub ×s²`.
- **Combined:** ISF ×s, `w_emit ×s·k`, `ysub ×s²·k`.
- **Forgiving hits:** raise `w_drad` (objdef cols 167/169 for w2/w3; arrows
  45→~96) — turns near-misses into hits; and/or ProjectileVarianceOnMove→0.0
  for perfectly-led shots at movers.

Do not scale `w_emit` when scaling ranges — it is not a range field; scaling
it alone shifts every landing point long (overshoot).

## Range 3× interaction: arrows land short (fixed 2026-07-13)

The land-short margin is a **fixed fraction of shot distance**, not an absolute
value: on flat ground the arrow returns to launch height at `T_fall = 2·emit/ysub`
(bows ≈ 2.47 s) while the horizontal arrival time is the distance-independent
constant `T_arrival = 4/ISF` (≈ 2.67 s at ISF 1.5), so every shot lands at
`T_fall/T_arrival ≈ 92.5 %` of the target distance — a ~7.5 %-of-distance short
fall. At vanilla range that ~7.5 % is roughly within `w_drad` (arrows 45) so
stationary targets are hit; with **`RangedRange3x`** the shortfall triples to far
beyond `w_drad`, so arrows physically land in front of the enemy ("根本射不到").

Fix (`EparaPatcher.InitSpeedReachMultiplier`, tied to `RangedRange3x`): raise ISF
so `T_arrival = 4/ISF` drops to the bow flight time `2·emit/ysub ≈ 2.47 s`, which
needs ISF ≈ 1.618; shipped multiplier is 1.08 (1.5 → 1.62). This drives the
land-short fraction to ≈ 0 at **every** distance, so projectiles reach the tripled
range. It also shortens flight time, reducing dodge on movers. The exact factor is
static-derived from the estimates above (like `ArcEmitMultiplier`) and still needs
in-game calibration — the hit window at 3× range is tight (`|1 − T_fall·ISF/4| ·
range_3x < w_drad`, i.e. ISF within roughly ±0.05 of 1.62 for bows).
