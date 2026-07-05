# Glory Upgrade & Combat Skills — Leader Ranking and Active Abilities

> Decoded 2026-07-04. Status: **static-verified** (objdef columns, `cl_epara.ini`
> ability factors, `cl_script.ini` glory rules, and the BCI ability-setter /
> glory function names all traced against the installed game and EXE). No
> runtime edit test has been performed yet, and no modifier patch exists for
> these fields yet.

## Summary

"Glory upgrades that strengthen combat units" is really **two separate
systems** in Against Rome:

1. **Glory-level stat scaling (leaders only).** A unit accumulates glory
   (`Ruhm`) from kills and reaching glory milestones grants permanent per-level
   bonuses to attack, defense, and damage. Verified against the whole objdef:
   only the four tribal leaders (`FigRomAnf00`, `FigGerAnf00`, `FigKelAnf00`,
   `FigHunAnf00`) have `maxruhm > 0`, so this progression is hero/leader-only.
2. **Active combat skills (any unit).** Toggle-able battle states — Berserker,
   marksmanship, protective shield, thunder strike, charge — applied to an
   object through `s_setObj*` BCI calls, with their effect magnitudes stored as
   plain factors in `SYSTEM/cl_epara.ini`.

The UI treats these as the "Kampf" (combat) branch of a glory improvement tree,
paralleled by a "Zivil" (civilian) branch — see [Upgrade Icons](#upgrade-icons).

## System 1 — Glory-Level Stat Scaling (objdef)

Per-level bonus columns in `SYSTEM/DATA_MP/DEFAULTS/objdef.dau` (1-indexed;
209 columns total). EXE field names in parentheses (`oe_*_plusstufe`).

| Col | Header | EXE name | Meaning |
|---|---|---|---|
| 143 | `aw` | `oe_aw` | Base attack value (Angriffswert) |
| 147 | `vw` | `oe_vw` | Base defense value (Verteidigungswert) |
| 148 | `aw_stuf` | `oe_aw_plusstufe` | Attack bonus **per glory level** |
| 149 | `vw_stuf` | `oe_vw_plusstufe` | Defense bonus **per glory level** |
| 150 | `dam_stu` | `oe_dam_plusstufe` | Damage bonus **per glory level** |
| 153 | `maxruhm` | `oe_maxruhm` | Max glory (glory needed to reach top level) |
| 158 | `AWsturm` | — | Charge (Sturmangriff) attack value |
| 159 | `AWunsic` | — | Attack uncertainty |
| 161 | `mobonus` | `oe_moralbonusstufe` | Morale/motivation bonus **per glory level** (leader aura) |
| 162 | `mobotim` | `oe_moralbonusstufetime` | Duration of that morale bonus, ms |

The morale-bonus columns (161/162) are the leader's **Motivation** aura, applied
in-battle through the `s_setObjMotivate` / `s_setObjMotivationLevel` BCI calls —
so glory-level scaling grants leaders **four** growing stats, not three (attack,
defense, damage, *and* morale aura). Other decompiled string: `oe_explonzstufe`.

### Verified leader values

| Unit | aw | vw | aw_stuf | vw_stuf | dam_stu | mobonus | mobotim | maxruhm |
|---|---|---|---|---|---|---|---|---|
| `FigRomAnf00_Anfuehrer` | 80 | 100 | 0.20 | 0.25 | 0.10 | 25 | 60000 | 100 |
| `FigGerAnf00_Anfuehrer` | 30 | 100 | 0.10 | 0.25 | 0.10 | 20 | 60000 | 100 |
| `FigKelAnf00_Anfuehrer` | 60 | 100 | 0.10 | 0.25 | 0.10 | 20 | 60000 | 100 |
| `FigHunAnf00_Anfuehrer` | 50 | 100 | 0.10 | 0.25 | 0.10 | 20 | 60000 | 100 |

A full-objdef scan confirms **no other unit** has `aw_stuf`, `vw_stuf`,
`dam_stu`, `mobonus`, or `maxruhm` non-zero — glory-level combat growth is
leader-only.

### Glory earn / loss rules (`SYSTEM/cl_script.ini` `[GlobalData]`)

| Key | Value | Meaning |
|---|---|---|
| `GloryIncKillGrp0` | 0 | per kill: civilians / palisades |
| `GloryIncKillGrp1` | 1 | per kill: warriors |
| `GloryIncKillGrp2` | 3 | per kill: buildings (except main house) |
| `GloryIncKillGrp3` | 5 | per kill: priests / artillery |
| `GloryIncKillGrp4` | 25 | per kill: main house / leader |
| `GloryIncAttacks` | 200 | +1 glory per 200 attacks |
| `GloryDecFlee` | 100 | glory lost per flee (leader only), per tribe |
| `GloryDecLost` | 1 | glory lost per lost battle |

(`GloryKillCntGrp0..4` set the kill-count divisor per group; all shipped as 1.)

### Relevant BCI / EXE functions

Object glory accessors registered by the script engine:
`s_addObjGlory`, `s_getObjGlory`, `s_setObjGloryRel`, `s_getObjGloryRel`,
`s_addToGloryCnt`, `s_getGloryCnt`, `s_initGloryCnt`, `s_gloryCntKilledObjs`,
`s_getTribeGloryPerMotivation`. EXE also references `obje_aktruhm` /
`obje_maxruhm` / `oe_maxruhm` (per-object current/max glory storage).

## System 2 — Active Combat Skills

Battle states applied to a combat object through BCI setters; magnitudes are
factors in `SYSTEM/cl_epara.ini`.

| Skill (DE) | BCI setter | Effect (cl_epara factor) |
|---|---|---|
| Berserker (狂戰士) | `s_setObjBerserker` | attack ×2.0 (`BerserkerAWfaktor`), damage ×2.0 (`BerserkerDAMfaktor`), defense ×0.0 (`BerserkerVWfaktor`) |
| Schuetzengeschick (射擊技巧) | `s_setObjSchuetzengeschick` | ranged weapon 1+2 shot radius ×1.2 (`SchuetzengeschickRADfaktor`) |
| Schutzschild (護盾) | `s_setObjSchutzschild` | incoming damage ×0.8 (`SchutzschildDAMfaktor`) |
| Donnerschlag (雷擊) | `s_setObjDonnerschlag` | weapon 0 damage ×1.5 (`DonnerschlagDAMfaktor`) |
| Sturmangriff (衝鋒) | `s_setObjSturmangriff` | uses objdef `AWsturm` charge attack value |
| Motivation (激勵, leader) | `s_setObjMotivate` / `s_setObjMotivationLevel` | morale aura from objdef `mobonus`/`mobotim` (cols 161/162), scales with the leader's glory level |

Related setters: `s_setObjCCombat` / `s_setObjFCombat` (close / ranged combat
mode), `s_objBerserker` / `s_setBerserker` (query / global variants),
`s_setObjRallyPnt` (leader rally point).

### Passive tribe special abilities (`cl_script.ini` `[SpecialAbilities]`)

| Tribe | Ability | Effect |
|---|---|---|
| Germanen | Kampfeslust | `Value=1` morale gain per affected object |
| Hunnen | Schrecken | `Value=5` morale loss to affected enemies (`SubExpl=1`) |
| Hunnen | Kannibalen | `Value=5` food per affected object on kill (`SubExpl=1`) |
| Kelten | (none shipped) | — |

## Upgrade Icons

`SYSTEM/banner.ini` defines the glory improvement-tree icons. Per tribe there
are **10 combat** and **10 civilian** upgrade icons:

- `Ver{Ger,Hun,Kel,Rom}KamIco00-09_Kampf_Icon` — combat branch (Ver =
  Verbesserung, Kam = Kampf).
- `Ver{Ger,Hun,Kel,Rom}ZivIco00-09_Zivil_Icon` — civilian branch.
- `Ver{...}Sta00-09_Bauwerkstandarte` — building standards.

## Glory Is Lost on Leader Death/Respawn (original-game behavior)

Player-reported symptom: after maxing the leader's glory (model grows, morale
aura at full `mobonus`), the moment the leader dies he respawns reset to base
size and base stats. This is confirmed by the scripts, not a display glitch.

**Why.** Glory lives on the individual object as `obje_aktruhm`; the model
scale (`alrfigfr`, objdef col 154, adjacent to `maxruhm`) and all four per-level
bonuses are derived live from the object's current glory. On death the object is
destroyed and the main house spawns a **fresh** leader object with
`obje_aktruhm = 0`; nothing carries the old value over.

**Respawn flow (decompressed scripts under `SYSTEM/CLAK/SCRIPT/`):**

| Stage | Script | Key symbols |
|---|---|---|
| Death / final tally / respawn timer | `ak_anfuehrer.bci` | `s_dead`, `s_gloryCntKilledObjs`, `s_addObjGlory`, `s_setDeadTime`, `s_destroyObj` |
| Detect missing leader, request respawn | `ak_haupthaus.bci` | `s_leaderExists`, `s_setLeaderRequest`, `s_leaderRequested` |
| Recreate leader object (glory 0) | `ak_haupthaus.bci` | `s_conCreateObjAtIOPnt`, `s_createBattleUnitsMax` |
| New leader init (counter zeroed) | `ak_anfuehrer.bci` | `s_initGloryCnt` |

Neither script reads the dying leader's glory value or restores it. The dead
branch even adds a final glory tally with `s_addObjGlory` immediately before the
object is destroyed, so that glory is computed and then thrown away.

### Injection points (`ak_anfuehrer.bci`, main per-tick handler @ code `0x724c`)

Offsets are code-stream relative (add `0x24` for the byte offset inside the
decompressed `.dec`). Disassembled with `tools/bcitool.py`.

INIT — glory counter is zeroed here; restore would go right after:
```
0x00791c  pushsym s_initGloryCnt ; s_initGloryCnt(v11)  -> counter reset
0x007930  pop
0x007934  <-- INIT injection (v10,v98 object id in scope; s_addObjGlory imported as #57)
```
DEATH — final tally then respawn timer; save would go right before setDeadTime:
```
0x008dd8  s_gloryCntKilledObjs(v10,v98)
0x008e9c  s_addObjGlory(1,v10,v98)     ; final tally added to the dying object
0x008f80  s_addObjGlory(v2,v10,v98)
0x008fc0  <-- DEATH injection (capture glory here)
0x008fe0  pushsym s_setDeadTime        ; s_setDeadTime(v10,v98) -> respawn timer
```

### Fix feasibility (as of this investigation)

- Reading the value is possible: `s_getObjGlory(team,obj)` exists in the EXE
  (signature `i_ii`, returns current glory) — but it is **not** in this
  script's 128-entry `SYMBCONS` import table, so it must be added to patch.
- `s_addObjGlory` (import `#57`) is already present, so the **restore** side is
  straightforward.
- **There is no `s_setGlobalValue`** — `s_getGlobalValue` is read-only map
  config. The only script-writable persistent primitive is `s_setScriptVarL` /
  `s_getScriptVarL` (read/write pair, sig `i_ii`).

**`ScriptVarL` scope — CONFIRMED global (EXE-verified).** The handlers at
`0x5220d0`/`0x522110` marshal args and delegate to `0x520fb0`/`0x520ff0` →
`FUN_004285f0` (store) / `FUN_00428630` (load). Both stringify the value with
`"%ld"` and read/write a **single named dictionary through the global pointer
`DAT_0065e074`**, keyed by the integer index (`FUN_004285c0`/`FUN_00428590` →
`FUN_0058a500`/`FUN_0058a710`). `DAT_0065e074` is assigned **exactly once**, in
`FUN_00428510` (create/reset), and is never swapped per script execution — so it
is a process/level-global store, not a per-instance table. A value written by
the dying leader's script instance is therefore readable by the respawned
leader's new instance. Carryover via `ScriptVarL` is sound.

Caveat — the index space is global and shipped scripts use it
(`Dorfverteidigung`, `Tributgeher`, `ak_level`, ...), so a patch must pick a
non-colliding index. The candidate keys per team as `700000 + team` to stay
clear of the small indices those scripts use.

Two implementation routes:

1. **BCI script patch** — extend `ak_anfuehrer.bci`'s `SYMBCONS` table with
   `s_getObjGlory` + `s_setScriptVarL` + `s_getScriptVarL`, then trampoline-inject
   a save at `0x008fc0` and a restore at `0x007934`. A candidate build lives under
   `re_workspace/glory_persist_patch/`. All statically-verifiable risks are now
   cleared (ScriptVarL scope = global; symbol arities match; PFIL header has no
   checksum so store-mode repack is safe; VAR/VIDX references only preserved
   code-stream offsets). Residual unknowns need the running game: whether the
   loader binds the three appended imports by name, whether the VM accepts the
   longer code section, and the actual in-game effect. See the patch README.
2. **EXE patch** — hook leader-ODef object create/destroy to stash/restore
   `obje_aktruhm` in a per-tribe slot. Avoids the script symbol-table and
   persistence-scope unknowns but is an assembly-level code patch.

### Modifier support withdrawn after runtime crash (2026-07-05)

In-game testing confirmed that completely disabling the leader-death glory
retention behavior causes the game to crash. The feature is therefore treated
as unsafe to modify.

The modifier no longer exposes this option in the UI, embeds or applies the
patched BCI, detects its state, or restores `ak_anfuehrer.bci` as part of Apply
All or either restore workflow. Existing installed-file state is deliberately
left untouched. This withdrawal removes modifier support; it does not attempt
to disable the behavior by writing another script state.

The crash cause has not yet been isolated. Static build or script validation is
not sufficient evidence that a future implementation is safe; any replacement
must pass in-game validation before this feature can be offered again.

## Modding Notes

- To let ordinary warriors gain glory-based combat growth, give their objdef row
  a non-zero `maxruhm` plus `aw_stuf` / `vw_stuf` / `dam_stu` values.
- To rebalance existing leaders, edit columns 148–150 (`aw_stuf`, `vw_stuf`,
  `dam_stu`), 161–162 (`mobonus`/`mobotim` morale aura), and 153 (`maxruhm`).
- To tune active skill strength, edit the factor lines in `cl_epara.ini`
  (e.g. `BerserkerDAMfaktor`, `SchuetzengeschickRADfaktor`).
- All objdef edits carry the usual caveat that some columns are ignored at
  runtime; verify against the installed game before shipping a patch.

## Sources

- Decompressed configs: `re_workspace/archive/scratch-2026-07-04/`
  (`objdef_decompressed.txt`, `cl_epara.decomp.ini`, `cl_script.decomp.ini`,
  `banner.decomp.ini`).
- BCI/EXE function names:
  `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`.
- Leader scripts: `SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci`, `ak_haupthaus.bci`
  (PFIL-wrapped BCI0; decompile with `tools/bcitool.py`).
