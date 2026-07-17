# Modifier Feature Decompilation Verification Matrix

> Reviewed 2026-07-17 against all 44 entries in `FeatureRegistry.All`. This is
> a static reverse-engineering audit, not a claim that every combination has
> been exercised in a live game. Runtime evidence is called out separately.

## Verdict

- Every registered feature has an identified apply target and a corresponding
  detection/restore path, or is explicitly an external installation feature
  for which original-game decompilation is not applicable.
- 42 feature IDs have an original-game EXE, data-loader, BCI, or decoded-script
  evidence chain. `DgVoodoo` and `ToEnglish` are external deployment features.
  `NativeWidescreen1920x1080` retains a statically decoded native EXE experiment,
  but that experiment was runtime-rejected; the active implementation restores
  the EXE and uses dgVoodoo presentation instead.
- `EndlessAi.M2`, `EndlessAi.M3`, and `EndlessAi.M4` are compatibility aliases.
  `PatchProfile.NormalizeCompositeValues` intentionally normalizes all three to
  `EndlessAi.Core`; they are not independent byte patches.
- Static proof establishes that the game reads the patched values and how they
  flow. It does not by itself prove every user-visible balance or gameplay
  outcome. The last column therefore never promotes static evidence to runtime
  evidence.

## Stats Features

| Feature ID | Actual write target | Decompilation / decoded-script evidence | Static verdict | Recorded runtime evidence |
|---|---|---|---|---|
| `FastCiviProduction` | `cl_script.ini` `CiviDelay` -> 500 ms | `[TribeData]` key parser, tribe storage, and runtime getter chain; `GhidraScriptIniAnalysis.java` | verified | not separately recorded |
| `InfiniteMorale` | four `MoralsDec*` / `MoralsIncIdle` records | `cl_script.ini` key parser and tribe-value consumers | verified | not separately recorded |
| `Balance` | composed `objdef.dau` HP, damage, reload, VW, AW, sight, and range values | objdef field-name table, loader callbacks, weapon and movement consumers; see `objdef-fields.csv` | verified as a composition of verified fields | preset as a whole not separately recorded |
| `FreeProduction` | `ress.ini` build/training/siege cost fields; pack horse exception preserved | `[objres]` parser and construction/training consumers in `GhidraRessAnalysis.java` and `exe-functions.md` | verified | mixed mounted-civilian quota side effect documented |
| `FreeUpgrade` | `ress.ini` building and `[volkres]` upgrade groups | parser names the four eight-level groups and consumers | verified | not separately recorded |
| `NoSpellCost` | `ress.ini` `spruch` columns 25-28 | spell-button cost consumer at `0x44a010`; distinct from spell effect values | verified | not separately recorded |
| `HousingCapacity20x` | `objdef.dau` column 156 `wohnwer` | objdef field table and housing-capacity consumer | verified | not separately recorded |
| `StorageCapacity10x` | `objdef.dau` column 42 on `Bau*Hau*` / `Bau*Lag*` | objdef field table and storage-capacity consumer | verified | not separately recorded |
| `HqHp10x` | `objdef.dau` column 19 on main buildings | objdef HP loader and runtime object-HP chain | verified | not separately recorded |
| `FastBuildUpgradeRepair` | `objdef.dau` columns 73/74 divided by 10 | build/upgrade time fields and repair linkage decoded from building runtime | verified | not separately recorded |
| `FoodHealing10x` | 12 unit BCI scripts: `s_addLP(obj, 1)` -> `10` | BCI opcode/symbol decode plus EXE `s_addLP` and `LPIncIdle` chain | verified | exact amount behavior not separately recorded |
| `MaxPopulation` | `MAPS/**/team.dat` `[teamdata]` column 4 -> 1600 | team loader stores column 4 as the per-team limit; `maxteamobjgenerell` is ignored | verified | not separately recorded |
| `CiviProduce20` | EXE file `0x4FC00`, production literal 1 -> 20 | player residential-tent click handler and housing clamp | verified | verified in game 2026-07-15 |
| `UnitRecruit20` | EXE file `0x4C7DD`, selected conversion count -> 20 | equipment/conversion click handler with the original cap retained | verified | verified in game 2026-07-15 |
| `IdleSelect999` | EXE `0x451DC0` handler rewritten in place | UI dispatch, 1000-entry scratch list, 999-entry master selection cap, and all callers decoded | verified | verified in game 2026-07-16 |
| `RomanEndless` | ENDL `team.dat` team 0 faction + EXE `dlg_volk` setter | team loader plus faction-selector call/data flow | verified | verified in game 2026-07-13 |
| `SpellDamage5x` | six `cl_script.ini [Spells] Value` records | spell parser -> runtime table -> `s_getSpecialEffectValue` -> damage consumers | verified | verified in game 2026-07-13 |
| `SpellHealing10x` | `KEL, Spell1, Value` 65 -> 650 | same runtime table; heal effect consumes field 5 | verified | not separately recorded |
| `SpellResurrection` | `KEL, Spell3, Value/Value2` -> 100/100 | decoded `ak_priester.bci` passes both fields to `s_specialEffektCreateUnit` | verified | not separately recorded |
| `GeneralSkills` | all shipped `[SpecialAbilities] Value*` records x5 | shared spell/ability parser plus decoded BCI ability setters and consumers | verified | not separately recorded |
| `LeaderGlory` | leader objdef columns 148, 149, 150, 161 x5 | per-glory-level attack, defense, damage, and motivation-aura consumers | verified | not separately recorded |
| `RangedRange3x` | ranged `w*_rad2` x3, synchronized `Sirad`, `w*_drad` x2, and moving variance -> 0 | weapon range, target acquisition, launch scatter, and impact-radius chains | verified | gameplay targeting and Legionary coverage follow-ups recorded 2026-07-13/14 |
| `UnitMovementSpeed2x` | objdef `moves`, `movsf`, `bmovs` x2 | movement functions and all three field consumers in `GhidraUnitRangeSpeedAnalysis.java` | verified | not separately recorded |
| `VillagerMovementSpeed5x` | same three movement fields on civilian/pack-horse rows | same movement data flow | verified | not separately recorded |
| `SpellEntireMap` | priest objdef `Sirad` -> 30000 | priest target acquisition/casting-distance gate; distinct from effect `Radius` | verified | not separately recorded |
| `SpellRange3x` | `cl_script.ini [Spells] Radius` x3 | parser field 4 -> `s_getSpecialEffectValue` -> area searches/effects | verified | not separately recorded |
| `ProjectileArcHeight` | projectile `w*_emit` x2 and seven `partgeo.dau ysub` values x2 | launch vertical velocity and particle-gravity integrator traced end to end | verified | mechanism verified with 10x experiment; shipped multiplier is 2x |
| `AllUnitsEntireMapVision` | supported unit/civilian objdef `Sirad` -> 30000 | shared sight/target-acquisition radius consumer | verified | not separately recorded |
| `CustomUnitStats` | composed objdef HP, damage, reload, VW, AW, sight, and ranged-distance fields | same verified objdef field and consumer chains as `Balance` | verified as a specialized composition | arbitrary combinations are user-defined and not globally runtime-certifiable |

## Compatibility And Installation Features

| Feature ID | Actual write target | Decompilation / decoded-script evidence | Static verdict | Recorded runtime evidence |
|---|---|---|---|---|
| `FocusLoss` | EXE file `0x161A88`, six-byte pause-state store -> NOP | focus-loss handler and state store decoded | verified | not separately recorded |
| `VillageBuildRange` | EXE hook `0x1364C1` + cave `0x16258F`, both bounds -> 30000 | shared setter updates type and per-object village bounds | verified | verified in game; build area and red frame synchronized |
| `NoSpellAltar` | 12 EXE altar-count comparisons -> 0 | three faction spell-button branches and their hard-coded altar thresholds | verified | not separately recorded |
| `GameSpeed` | EXE doubles at `0x204214` and `0x20424C` | QPC and `timeGetTime` clock paths each have a single scale-constant xref | verified | not separately recorded |
| `DgVoodoo` | managed dgVoodoo DLL/EXE/config deployment | external wrapper; no original-game semantic patch | not applicable to original-game decompilation | install/detect/restore covered by tests |
| `NativeWidescreen1920x1080` | active path restores six native EXE sites and updates dgVoodoo centered presentation | native resolution identify/create/getter/dialog chains were decoded | native experiment statically verified but runtime-rejected | legacy viewport persisted; dgVoodoo fallback is the retained behavior |
| `CameraZoomOut1` | three persistent setter calls redirected through EXE cave, minimum zoom step 1.0 | startup, save-load, mission-script setters and native `0x498A30` clamp decoded | verified | first runtime-effective native step recorded |
| `EndlessAi.M1` | BCI P1/P10/P12 reinforcement and conversion-size literals | decoded `ak_level.bci` state machine, native BCI callbacks, and exact signatures | verified | multiple reinforcement follow-ups recorded; see `endless-mode-ai.md` |
| `EndlessAi.Core` | integrated lifecycle/respawn patches formerly exposed as M2/M3/M4, plus mandatory safety repairs | decoded BCI states, scheduler, deletion handshake, settle-place native checks, and save evidence | verified; some latest sub-fixes remain runtime-pending as documented | respawn recovery verified; deadline repair remains explicitly pending |
| `EndlessAi.M2` | compatibility alias normalized to `EndlessAi.Core` | no independent target after normalization | verified alias, not an independent patch | inherits Core evidence |
| `EndlessAi.M3` | compatibility alias normalized to `EndlessAi.Core` | no independent target after normalization | verified alias, not an independent patch | inherits Core evidence |
| `EndlessAi.M4` | compatibility alias normalized to `EndlessAi.Core` | no independent target after normalization | verified alias, not an independent patch | inherits Core evidence |
| `EndlessAi.M5` | settlement SDL `resv` 0s -> `614,300,372,250,460,288` | decoded settlement template resource fields and exact PFIL rewrite | verified | not separately recorded |
| `EndlessAi.M6` | BCI reinforcement threshold, retreat quota, and type-filter control points | decoded state-49/50 donation flow plus native `s_getUnitType` callback semantics | verified | final `jnz 92` behavior verified in a new endless game 2026-07-17 |
| `ToEnglish` | managed language overlay files | file deployment/rollback only; no original-game semantic patch | not applicable to original-game decompilation | install/detect/restore covered by tests |

## Evidence Sources And Limits

Primary static evidence is the local Ghidra function inventory, focused logs
under `re_workspace/`, reusable scripts under `tools/re/`, the BCI decoder notes,
and the canonical documents linked from `README.md`. Generated full-program
pseudocode and the original EXE remain local-only and must not be committed.

The initial live headless rerun on 2026-07-17 exposed an incomplete/mixed local
Ghidra tree. It was replaced non-destructively with the SHA-256-verified official
12.1.2 distribution. Two fresh one-shot imports then completed successfully:
`GhidraUnitRangeSpeedAnalysis.java` reconfirmed movement and weapon-distance
xrefs, while `GhidraScriptIniAnalysis.java` and `GhidraRessAnalysis.java`
reconfirmed their parser and storage chains. This refreshes those high-coverage
evidence families; the generated full inventory and remaining focused logs stay
valid local evidence unless a future rerun finds a concrete contradiction.
