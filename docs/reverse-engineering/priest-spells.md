# Priest Spell System — Effect Values, Unit Types, Resurrection

> Status reviewed 2026-07-11. `Radius` is the spell effect area; priest casting distance is separately gated by objdef `Sirad` (column 24).

> Decoded 2026-07-04. Status: **static-verified** (parser, storage, getter, and
> BCI consumption all traced end-to-end against the installed game and EXE).
> The modifier's characterization tests verify the patch values against these
> same `Value`/`Value2` fields. `Radius` remains a separate effect-area field;
> it is deliberately not used for casting-distance features.

## Summary

All twelve priest spells (3 tribes × Spell0..Spell3) are data-driven. Their
numeric strength (damage / heal / morale per affected object), durations,
summon counts, and — for summon/resurrect spells — the created unit types are
plain INI records in two PFIL-compressed files:

- `SYSTEM/cl_script.ini` `[Spells]` — numeric parameters per tribe+spell.
- `SYSTEM/CLAK/cl_scint.ini` `[Spells]` — `SpellODef`/`SpellODef2` unit-type
  assignments (this file also owns the `[ObjDefName]` alias table the values
  reference).

Both files parse into the same runtime spell-parameter table; the priest AI
script `SYSTEM/CLAK/SCRIPT/ak_priester.bci` reads that table through
`s_getSpecialEffectValue` at cast time. Nothing about spell strength is
hardcoded in the BCI script or the EXE.

## Field Table (EXE key table at file offset 0x21bc10)

The `[Spells]`/`[SpecialAbilities]` line parser callback lives at VA
`0x41cba0` (registered from `FUN_0041bce0`, the `cl_script.ini` +
`cl_scint.ini` loader). It resolves the key name through a `{name, index}`
table at file offset `0x21bc10` (10 entries):

| Field index | Key | Value format |
|---|---|---|
| 0 | `MainExpl` | int, explosion id at target point, -1 = none |
| 1 | `Duration` | int ms (stored 32-bit) |
| 2 | `SubExpl` | int, explosion id per affected object |
| 3 | `SubInterval` | int ms |
| 4 | `Radius` | int [vP] |
| 5 | `Value` | int (damage / heal / morale per object) |
| 6 | `Value2` | int (second value, e.g. resurrect morale %) |
| 7 | `NumObjects` | int (summon count) |
| 8 | `SpellODef` | **ODef name string**, resolved case-insensitively |
| 9 | `SpellODef2` | **ODef name string** (second unit type) |

Line syntax: `Key =TRIBE, SpellN, value` — tribe token resolved via the table
at `0x61ba40` (GER/HUN/KEL/ROM), spell token via `0x61bbd0`
(Spell0..3, SAbility0..3).

- Fields 0–7 parse with `%ld`; fields 8–9 take the value token as a name and
  resolve it through the runtime ODef name table at `0x265be70`
  (500 × 16-byte entries, `stricmp` loop in `FUN_0052ea90`, name → handle via
  `FUN_0052ea20`).
- Storage setter: `FUN_00541440(tribe<4, spell<8, field<10, value)`.
  Getter (used by the script API): field dispatch at `0x541660`; `Duration`
  is a 32-bit slot at `0x29c5508`, **all other fields are stored as 16-bit
  signed words** (`sar eax, 0x10` on read) — practical maximum 32767 for
  `Value`/`Value2`/`Radius`/`NumObjects` etc.
- Defaults initializer (`0x541dc0..`): `MainExpl`/`SubExpl`/`SpellODef`/
  `SpellODef2` default to -1, `Radius` defaults to 500 (0x1f4), the rest to 0
  — then both INI files overwrite whatever they define.

## Shipped Values

`SYSTEM/cl_script.ini` `[Spells]` (numbers as shipped):

| Tribe | Spell | Name | Value fields |
|---|---|---|---|
| GER | Spell0 | Kundschafter (scout) | Duration 60000 |
| GER | Spell1 | Nebel (fog) | Duration 30000, SubInterval 1000 |
| GER | Spell2 | Blitz und Donner (lightning) | **Value 70 damage** |
| GER | Spell3 | Wolfsrudel (wolf pack) | NumObjects 10, Duration 120000 |
| HUN | Spell0 | Feuer (fire) | **Value 30 damage**, Radius 1250 |
| HUN | Spell1 | Erdbeben (earthquake) | **Value 350 damage**, Radius 1250 |
| HUN | Spell2 | Giftwolke (poison cloud) | **Value 80 damage**, Radius 1250 |
| HUN | Spell3 | Geisterreiter (ghost riders) | NumObjects 10, Duration 90000 |
| KEL | Spell0 | Zwietracht (discord) | **Value 60 morale**, Radius 1250 |
| KEL | Spell1 | Heilen (heal) | **Value 65 heal**, Radius 1250 |
| KEL | Spell2 | Telekinese | **Value 60 damage**, Radius 1250 |
| KEL | Spell3 | Tote erwecken (raise dead) | **Value 50 %HP, Value2 50 %morale**, Radius 1250 |

GER spells define no `Radius` records in the original file (known limitation
already documented for the spell-radius feature).

`SYSTEM/CLAK/cl_scint.ini` `[Spells]` (unit types for summon/resurrect):

```ini
SpellODef =GER, Spell0, GER_WOL00      ; scout      -> FigGerWol00_Wolf
SpellODef =GER, Spell3, GER_WOL01      ; wolf pack  -> FigGerWol01_Wolf
SpellODef =HUN, Spell3, HUN_KAVINF02   ; ghost riders -> FigHunKav03_Geisterreiter
SpellODef =KEL, Spell3, KEL_INF00      ; male corpses   -> FigKelInf00_Schwert
SpellODef2=KEL, Spell3, KEL_SCH00      ; female corpses -> FigKelSch00_Bogen
```

The aliases resolve in the same file: `[ObjDefName]` (~56 `Fig*` unit aliases,
e.g. `KEL_INF01=FigKelInf00_Lanze`, `ALL_BAE00=FigTieBae00_Baer`) and
`[ObjDefScript]` (alias → `ak_*` AI script, e.g. `KEL_INF00=ak_krieger`).
To retarget a summon/resurrect spell, point `SpellODef` at another **existing
alias**; do not rewrite the alias definitions themselves — `KEL_INF00` etc.
are also consumed by `FigType` recruit mappings.

## Resurrection Logic (KEL Spell3), decoded from ak_priester.bci

`ak_priester.bci` (BCI0, 47064 code bytes, 145 symbols) — resurrection branch
at decompressed code offsets `0xA4DC..0xAB14`:

1. `s_getODescHandle(OD_MENSCH)` — class handle for "human".
2. `s_searchDeadObjsPos(arr, team=-1, OD_MENSCH, 1, x, y, Radius)` — collects
   **dead humans of any team** inside the spell radius (animals/artillery
   corpses excluded by class).
3. Two passes over the corpse list with `s_copyObjArrBySex(dst, src, sex)`
   (sex 1 then 2); each pass reads the unit type from spell field 8
   (`SpellODef`, males) or 9 (`SpellODef2`, females).
4. Count is clamped by `s_getTeamFreeObjs(team)` via `s_minN2L` — free
   population slots cap the resurrection.
5. Each corpse is removed via `s_sendNetMsg(team, obj, 101, 19)`; the script
   polls `s_objExists` with a 10-second deadline before spawning.
6. `s_specialEffektCreateUnit(..., odefHandle, count, x, y, Value, Value2)`
   creates the caster-team unit with `Value` %HP and `Value2` %morale.

Damage spells instead call `s_enemyAeraSpecialEffect(x, y, effectId, value,
...)` with per-spell effect ids (8 fire, 9 earthquake, 10 poison, 16 discord,
18 telekinesis observed as `pushlit` literals; lightning uses `s_createExpl`
plus its own path). The value argument always originates from
`s_getSpecialEffectValue(effect, fieldIndex)` — i.e. the INI table above.

EXE side: `s_specialEffektCreateUnit` handler thunk `0x547650` →
`FUN_005465e0` → `FUN_005245d0` (spawn loop, attaches `DEFSCRIPT`, registers
created ids). Registration block for all `s_specialEffekt*` script APIs is in
the `FUN_005b1410` sequence around decompiled-inventory lines 186446-186448.

## Not the spell system (disambiguation)

- `SYSTEM/cl_epara.ini` holds combat-STATE factors (Berserker, Schutzschild,
  Donnerschlag, Schuetzengeschick multipliers) — special abilities of regular
  units, unrelated to priest spells.
- `ress.ini` `spruch` columns 25-28 are spell **costs** (consumed by
  `0044a010`), not effects — see `exe-functions.md`.
- Priest idle HP regen is the separate `s_addLP` patch — see
  `known-patches.md` "10x Idle HP Regeneration".

## Patch Design Notes (Implemented 2026-07-04)

- Both files are PFIL/LZSS; the existing `GameLZSS` + line-rewrite machinery
  used for the spell-radius feature is applied. `cl_script.ini` is managed 
  by parsing `Value` and `Value2` using `RegexSpellValuePatch`.
- `cl_scint.ini` is integrated into the modifier's required backup list and 
  managed keys. Modifying `SpellODef`/`SpellODef2` changes Celt Spell3 
  resurrected unit types.
- Respect the 16-bit signed storage limit (≤ 32767) for all non-Duration
  fields. All multiplied values are verified to be safe from overflow.

## Experimental feature verification

The static call/data chain and the modifier test suite verify these targets:

| Feature | Decompilation target | Patched value |
|---|---|---:|
| Healing 10x | `KEL, Spell1, Value` consumed by `s_getSpecialEffectValue` | 65 -> 650 |
| Enhanced resurrection | `KEL, Spell3, Value/Value2` passed to `s_specialEffektCreateUnit` | 50/50 -> 100/100 |
| Ranged unit range 3x | `objdef` `w*_rad1/w*_rad2` (80/81, +8 per weapon) | original x 3 |
| Unit movement speed 2x | `objdef` `Moves/Movsf/Bmovs` (4/23/191) | original x 2 |
| Priest casting distance, entire map | priest `objdef` `Sirad` (24, sight radius) | max distance 30000 |
| Spell effect radius 3x | `cl_script.ini` `[Spells] Radius` | original x 3 |

The priest `Radius` value (field index 4 in the EXE spell table) is the
effect/search area used by healing and resurrection, not the caster-to-target
distance. `SpellEntireMap` uses the priest `Sirad` sight-radius field in
`objdef`; `SpellRange3x` uses these `Radius` records in `cl_script.ini`.
