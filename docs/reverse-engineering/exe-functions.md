# Executable Functions And Offsets

Addresses are from the currently analyzed `Against_Rome.exe`. Function names are
provisional unless manually named in a Ghidra project.

## Full Local Inventory

The current local Ghidra project identifies 7381 functions in `Against_Rome.exe`.
Generated lookup artifacts are under ignored local workspace files:

- `re_workspace/ghidra_inventory/against_rome_function_index.csv`
- `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

See `decompilation-workflow.md` for the exact headless command. The generated
pseudocode is local reverse-engineering material, not original source, and must
not be treated as proof of a function's gameplay meaning without call-path or
runtime evidence.

## BCI Virtual Machine Dispatcher (decoded 2026-07-03, no Ghidra)

The Temp Ghidra install (`C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\ghidra`)
lost its `Framework/Utility` module partway through this session and
`analyzeHeadless.bat` now fails with `Failed to find the 'Utility' module!`
before loading the existing project. This session's disassembly work used
`pip install capstone pefile` (into the harness's isolated user Python) instead
— a linear x86 disassembler plus direct EXE byte scanning is sufficient for
opcode-table and call-site work; only a fresh Ghidra project/reinstall would
be needed for broader whole-program analysis again.

- Interpreter entry: `005B1C62` (frame setup, `EBX` = VM context pointer).
  Opcode fetch at `005B1C72`: read 32-bit opcode word at `[EBX+8]` (PC,
  byte offset into the code stream), advance PC by 4, `index = opcode - 1`,
  bounds-check `index <= 0xB0`, dispatch via jump table `005B199C[index]`
  (unrecognized opcodes fall through to the default handler `005B972C`).
- VM context layout (offsets from `EBX`): `+0x04` script object, `+0x08` PC,
  `+0x0C` stack index, `+0x28` code size, `+0x2C` code base, `+0x38` stack
  capacity, `+0x3C` stack base (grown via `realloc`-style call through
  `[0x637ed4]`).
- Every opcode's operand-word count was read directly from its handler's
  PC-advance guard (`lea eax,[esi+N]; cmp eax,[ebx+0x28]` — `N` is the byte
  count consumed), not inferred from patterns. This superseded and corrected
  the pre-existing empirical BCI opcode table — see
  `docs/reverse-engineering/bci0-opcodes.md` for the full corrected table and
  the specific errors it fixes (opcodes 76/77/78/82/etc. were wrongly treated
  as zero-operand, which misaligned any listing containing them).
- Conditional-jump family `113..118` (jlt/jle/jgt/jge/jz/jnz) share one
  handler (`005B8377`) with a small 7-entry sub-table at `005B1980`;
  unconditional `jmp` is opcode `112`. Verified target arithmetic:
  `target = opcode_word_address + 8 + operand` (NOT `+4`, and NOT relative to
  the instruction start alone — the PC has already advanced past both the
  opcode and operand words by the time the branch handler adds the operand).
- `120` = call internal script function: pushes the return address, then
  jumps using the same `+8+operand` arithmetic; `121` = return.
- Regenerable via `tools/re/` scripts is NOT how this was done this session
  (Ghidra was broken); the throwaway capstone scripts (`trace_lpinc.py`,
  `dump_keytable.py`, `vm_table.py`, `disfun.py`, `bcidis2.py`) are not
  checked into the repo — recreate from this spec (jump table at `005B199C`,
  per-handler PC-advance guard scan) if this needs to be redone.

## Idle HP Regeneration / `s_addLP` Chain (decoded 2026-07-03)

Traced while implementing the idle-regeneration modifier feature; see
`known-patches.md`'s "10x Idle HP Regeneration" section for the
patch itself and the exact heal gate conditions.

- `s_addLP` script callback trampoline `0051A090` forwards to `FUN_005129c0`
  (bounds-checks the object, `-1 < id < 0x3714`) `-> FUN_00512a10` (branches
  on `FUN_00513f00`: unit-array object vs. plain object)
  `-> FUN_00525ac0` (iterates every member of a unit/formation object,
  `iVar3 < *(&DAT_0259e44c + id + 1)>>0x18` member count) or directly
  `-> FUN_00512aa0` (single object) `-> FUN_004ad190 -> FUN_004ad1e0`, the
  actual LP-delta applicator. `FUN_004ad1e0` has NO village-bounds test and
  NO resource-store read/write anywhere in its body — confirmed by full
  pseudocode read, not just symbol absence.
- `[TribeData]` key table at `0061BB40` (EXE data, not a script string blob):
  `LPIncIdle` = key `15`. Parser `0041c600` (`[TribeData]` section callback)
  stores each key via `FUN_00540dc0`, which switches on the key and clamps:
  key 15 (`LPIncIdle`) into `DAT_029c50e8[tribe]`, clamp `500..100000000`,
  default `10000` (the runtime default differs from the `cl_script.ini`
  shipped default of `15000` — the INI always overrides it at load).
  Getter `FUN_00540fe0` mirrors the same switch; script-exposed as
  `s_getTribeValue` via trampoline `00542250`.
- 18 `SYSTEM/CLAK/SCRIPT/*.bci` scripts call `s_addLP`; 12 are unit AI
  (healing target of the modifier feature), 7 share an identical byte-for-
  byte template for `Bau*` building self-repair (`ak_haupthaus`, `ak_lager`,
  `ak_produktion`, `ak_wohnhaus` all resolve `s_addLP` to symbol index 51 at
  the identical decompressed offset `0x2890` — strong evidence of a shared
  compiled template), and 3 unit scripts (`ak_geisterreiter`,
  `ak_kundschafterwolf`, `ak_verbandswolf`) additionally call `s_addLP` with
  literal `-1` at a second site (an LP-decay tick, unrelated to healing).

## Ress Parser

- `0046c1c0`: loads `SYSTEM/ress.ini`.
- `0042a230`: parser used by `0046c1c0` for named sections.
- `0046bd00`: `[objres]` callback.
- `0046b200`: `[volkres]` callback.
- `0073d36c`: referenced as volkres row/table storage.
- `0073d070`, `0073d078`: referenced as formation-related tables.

Known strings:

- `SYSTEM/ress.ini` at `005eedb9`
- `[maxskills]` at `005eedc9`
- `[objres]` at `005eeddb`
- `[volkres]` at `005eede6`
- `resv_res%ld_bau` at `005ee9b9`
- `resv_res%ld_upg` at `005ee9d9`

## Team Data And Banner Version

- `00469370`: loads/parses map `DATA/team.dat` sections.
- `00468fcc`: `[teamdata]` callback candidate. The parser call passes 6
  columns and 8 rows for `[teamdata]`.
- `0073ca88`: team structure base. Rows are 0x84 bytes each.
- `team + 0x00`: faction id parsed from `[teamdata]` column 1.
- `team + 0x04`: banner version (`bver`) parsed from `[teamdata]` column 5.
- `team + 0x80`: population limit parsed from `[teamdata]` column 4.
- `0046a850`: setter candidate for `team + 0x04`; validates
  `0 <= bver < 10`.
- `0046a890`: getter candidate for `team + 0x04`.
- `00469ab0`: loads `SYSTEM/banner.ini`.
- `0073b3f0`: banner lookup table base used by faction plus `bver`.

Known strings:

- `SYSTEM/banner.ini` at `005ee661`
- `[volk%02ld_vicon_bver%02ld]` at `005ee673`
- `[volk%02ld_obdef_bver%02ld]` at `005ee699`
- `SYSTEM/CLMK/DLG/BANNER/%s` at `005ee6ba`
- `[teamdata]` at `005ee4c1` and `005ee4ce`

## Unit Creation And UI Limits

- `00529f90`: script callback `s_createUnit`
- `0052a020`: script callback `s_createUnitAndMems`
- `0052a110`: script callback `s_createBattleUnitsMax`
- `0052a140`: script callback `s_createCiviUnitsMax`
- `0052a170`: script callback `s_unitMemsWeaponMax`
- `0052a3e0`: script callback `s_getNotHorseUnitMems`
- `005249d0`: create battle units max implementation candidate
- `00524d70`: create civilian units max implementation candidate
- `005251d0`: unit members weapon max implementation candidate
- `00527110`: get non-horse unit members implementation candidate
- `00523a00`: unit creation from gathered members candidate
- `00538320`: member gather/filter candidate

## Endless Mode Script

Findings are from decompressed `MAPS/ENDL_000/SCRIPT/ak_level.bci`.

- `0x08DEC`: BCI reference to state name `INIT_UNITSCIV`.
- `0x08E1C`: BCI reference to state name `INIT_UNITSMIL`.
- `0x0E584`: BCI external call to `s_setVillageTemplate`.
- `0x0A5D8`, `0x0A784`, `0x1A554`: BCI references to
  `s_createUnitAndMems`.
- `0x17B60`: BCI external call to `s_addNPCJob_createUnit` with 9 arguments.
- Observed external-call bytecode pattern:
  `0x80 <symbol-id> 0x49 <negative-argument-count> 0x56`.
- `0054aa80`: EXE script callback for `s_addNPCJob_createUnit`; forwards
  9 integer arguments to `00547f50`.
- `00547f50`: implementation for NPC create-unit jobs. Argument 1 is team
  `0..7`; argument 2 is restricted to `0`, `1`, or `3`; argument 3 is
  restricted to `0..9`; arguments 6 and 7 become the clamped unit-count range.
  If argument 2 is `0`, max count is 4; otherwise max count is 20.

## NPC Active-State Storage And Endless Defeat Recovery

Findings from decoding the `ak_level.bci`/`ak_npc.bci` party-defeat-recovery
chain (2026-07-02); see `docs/reverse-engineering/endless-mode-ai.md` and
`bci0-opcodes.md` for the full script-side trace.

- `DAT_029e6000`: 8-byte array, one entry per team (`0..7`), holding the
  `npcActive` flag consumed by `s_NPCActive`/set by `s_setNPCActive`.
- `0054ac80` (`s_setNPCActive` script callback trampoline) forwards to
  `FUN_00548ce0(team, activeFlag)`: validates `team` in `0..7`, writes
  `DAT_029e6000[team] = (activeFlag != 0)`.
- `0054acb0` (`s_NPCActive` script callback trampoline) forwards to
  `FUN_00548d20(team)`: validates `team` in `0..7`, returns
  `DAT_029e6000[team] != 0`.
- `0054a070`: level-init sweep that zeroes `DAT_029e6000[0..7]` (and various
  other per-team arrays) once per level load. This and the two callbacks
  above are the *only* writers of `DAT_029e6000` found; the defeat-to-active
  transition is entirely script-driven (`ak_npc.bci` calls
  `s_setNPCActive(team, 1)` once a team's village is healthy again), so no
  EXE patch is needed for AI Ultimate's respawn behavior — only the
  `ak_level.bci` party retreat/cleanup deadlines gate how quickly that script
  logic gets a chance to run.

## Village Bounds And Rejected Red-Frame Candidate

- `0054af80`: script callback `s_setVillageTemplate`.
- `00549500`: village-template implementation; resolves template name and loads
  village building/palisade data.
- `005363e0`: returns current team village position through `FUN_0050f750`.
- `00536450`: validates the team and deltas, calls `004c0900` to write the X/Z
  values into the village object's type-definition rectangle, then stores the
  same values in per-object village state. `00539700` reaches this setter while
  initializing a pending village from its computed/template record.
- `00536580`: returns the per-object copy of those half-size candidates.
- `00536630`: calculates logical village bounds as
  `center +/- (halfSize * 0x40 + 0x20)`.
- `00536820`: tests whether a world coordinate is inside the village bounds,
  with an optional `param_4 * 0x40` margin.
- `005368c0`: clamps or projects a coordinate to the village bounds.
- `0053c140 -> 00537d60`: `s_setShowTeamVillageAera` stores the per-team
  visibility flag.
- `00535060`: when the village center object is displayed, the visibility flag
  adds object display bit `0x1d`.
- `004c0970`: reads the same type-definition words written by `004c0900` and
  tests whether a point falls inside the object-centered rectangle. It has ten
  UI callers, so globally changing this generic function would also change
  non-village object hit testing.
- `004d7160`: consumes display bit `0x1d`, resolves the displayed object's
  object-definition ID, reads the type-definition rectangle, and draws four
  dashed sides through `00495360`.
- `00536820` is directly called by `005367c0` and `00544fd0`; the latter uses it
  only when its restrict-to-village argument is enabled while searching for a
  candidate coordinate. Player previews `0044f4b0` and `0044f7b0` do not use
  this test, so it is not the general player construction-range gate.
- `00451650` and overlay type `0x28` are unrelated; their callers are the
  `igm_but_kampf_beserk` and `igm_but_kampf_normal` combat-mode buttons.

Rejected runtime candidate: changing the two logical shifts and the two
`004d7160` shifts from `6` to `7` produced no observed change to either the
buildable area or the visible red dashed frame. The modifier therefore never
applies these bytes and retains them only for detection and restoration. See
`known-patches.md` for exact file offsets and the failed-test record.

## Implemented EXE Patch

## Resource Table Runtime Consumers

- `0046bd00`: `[objres]` row loader. It loads five independent groups:
  columns `1-6` (`bau`), `7-12` (`upg`), `13-18` (`aus`), `19-24` (`auf`),
  and `25-28` (`spruch`).
- `0046df80`, `0046dff0`, `0046e070`, `0046e170`, `0046e1e0`: accessors for
  those five groups in the same order.
- `00449560` and `0044c4d0`: consume the six `aus` values when calculating
  producible unit counts and accumulated training resources.
- `00450020` and related construction paths consume the six `bau` values;
  `0044fa00` consumes the six `upg` values.
- `0044a010`: consumes all four `spruch` values for priest spell availability.
  It also enforces the hardcoded per-spell altar-count requirements (1/2/3/4
  `OD_BAUOPF` buildings, counted via `00453730 -> 00421dc0 -> 0052c7c0 ->
  00538740`); the twelve `cmp esi, imm8` patch sites are documented in
  `spell-altar-requirements.md`.
- `0046c6d0` and `0046ce60`: expose/load the `[volkres]` groups by their
  runtime names. Columns `264-295` are four arrays of eight values:
  `befehl`, `motivieren`, `angriff`, and `verteidigung`.

## Spell Parameter Table (see `priest-spells.md`)

- `0041bce0`: loads `SYSTEM/cl_script.ini` then `SYSTEM/CLAK/cl_scint.ini`;
  both register the `[Spells]`/`[SpecialAbilities]` line callback `0041cba0`.
- `0041cba0`: `[Spells]` line parser. Key table at file offset `0x21bc10`
  (MainExpl..SpellODef2 → field indexes 0-9); tribe table `0x61ba40`; spell
  table `0x61bbd0`. Fields 8/9 (`SpellODef`/`SpellODef2`) take an ODef NAME
  resolved via `0052ea90` (stricmp loop over the 500-entry name table at
  `0x265be70`) + `0052ea20` (name index → handle); fields 0-7 parse `%ld`.
- `00541440`: spell-parameter setter `(tribe<4, spell<8, field<10, value)` —
  single caller is the parser above.
- `00541660` region: field getter behind `s_getSpecialEffectValue`.
  `Duration` is 32-bit at `0x29c5508`; all other fields are 16-bit signed
  words (`sar eax, 0x10` on read → effective max 32767).
- `00541dc0`: defaults initializer (MainExpl/SubExpl/SpellODef/SpellODef2 =
  -1, Radius = 500, others 0) — INI records overwrite these.
- `00547650` → `005465e0` → `005245d0`: `s_specialEffektCreateUnit` handler
  chain (summon/resurrect spawn loop, attaches `DEFSCRIPT`).

Focus-loss pause patch:

- File offset: `0x161a88`
- Original bytes: `89 15 C4 7D 9E 02`
- Patched bytes: `90 90 90 90 90 90`

The modifier verifies the expected original bytes before writing.
