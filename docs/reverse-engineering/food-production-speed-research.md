# Food / Resource-Building Production Speed — Research Notes (Interrupted)

> Started 2026-07-02. This session was interrupted (AI-agent usage limit) before
> any code was written. No patch exists yet. This file exists so the next
> session/agent can resume without re-deriving the investigation from scratch.

## User goal

Player has increased troop counts (via existing unit/population features) and
now wants **food production from resource buildings to run faster**, to keep
up with the larger army's upkeep.

## Conclusion so far: no existing feature does this

Searched the whole repo (`ModifierForm.Patches.cs`, `Localization.cs`,
`objdef-fields.csv`, `ress-fields.csv`, all `docs/reverse-engineering/*.md`,
`data/game_schema.json`). Nothing in the current modifier changes the *rate*
at which staffed resource buildings produce food/wood/stone/gold/iron. The
closest existing features are NOT this:

| Existing feature | What it actually changes | Why it's not "production speed" |
|---|---|---|
| 10x Building Speed (`ModifierForm.Patches.cs:1474-1520`, `objdef.dau` cols 73/74 `buildt`/`upgrdt`) | Construction/upgrade/repair time | Speed of *building* the structure, not its ongoing output |
| Fast Civi Production (`ModifierForm.Patches.cs:1124-1236`, `cl_script.ini` `CiviDelay`) | Villager (population unit) spawn interval | Birth rate of villagers, not resource output |
| Free Production/Build/Upgrade costs (`ress.ini` `[objres]`/`[volkres]`) | Resource *cost* to build/train/upgrade | Cost, not production rate |
| Endless AI village economy patch (`ApplyEndlessAiVillageEconomyPatch`, `ModifierForm.Patches.cs:1927-1979`) | AI-only NPC economy scripts, and is forced to always restore to original (see below) | Explicitly rejected/disabled — see next section |

## The actual production logic: `SYSTEM/CLAK/SCRIPT/ak_produktion.bci`

This is the game's global production script (PFIL-compressed BCI0 bytecode).
It almost certainly contains the real per-tick production interval/amount
literals for staffed resource buildings.

**This script is currently only ever restored to its original bytes by the
modifier — never modified for real gameplay.** Relevant code:

- `ModifierForm.Patches.cs:1927-1979` — `ApplyEndlessAiVillageEconomyPatch()`.
  The call touching this file is at `1943-1954` (`ak_produktion.bci`), but
  the `enabled` flag passed in is hardcoded/derived such that the actual
  effect today is always "restore to original" — see the constants at
  `ModifierForm.Patches.cs:106-108`:
  `EndlessAiProductionGateOriginalOpcode = 117`,
  `EndlessAiProductionGateBypassOpcode = 112`,
  `EndlessAiProductionGateJump = 56`.
- `ModifierForm.Patches.cs:1899-1925` — `PatchEndlessAiEconomyScript()`, the
  generic BCI decompress → signature-match → patch-word → recompress helper.
  Reusable infrastructure for any future BCI edit.

### Why this path is dangerous — do NOT just flip the old gate patch back on

An older build changed the BCI word at decompressed offset `0x3710` in
`ak_produktion.bci` from opcode `117` (conditional jump / `jz`, i.e. the
resource-eligibility gate check) to `112` (unconditional jump, bypassing the
check). **Runtime testing proved this makes staffed player resource
buildings produce ZERO output, including in a brand-new game.** It was
permanently rejected. Do not re-enable it under any name. Documented in:

- `docs/reverse-engineering/endless-mode-ai.md:225-232` ("Rejected global
  village-production patch")
- `docs/AI_AGENT_HANDOFF.md:272-278`
- `docs/reverse-engineering/known-patches.md:139-142`
- `data/game_schema.json:214-235` (`villageEconomy`, status
  `disabled-runtime-regression`)

That old patch changed the *eligibility gate* (whether production fires at
all), not the *speed* (how often/how much it produces). Those are different
BCI sites. **Speeding up production requires finding a different literal** —
most likely a millisecond delay/interval constant or a per-tick output
amount, analogous to how `PatchEndlessLoopDelayLiterals`
(`ModifierForm.Patches.cs:1981-2019`) shortens AI polling-loop delay
literals in `ak_level.bci` without touching gate logic. That function is a
good template for the *shape* of a safe interval-only edit.

## What's needed to make progress

1. **Actual game files.** The public repo does not ship `Backup.zip` /
   original game data (see `docs/AI_AGENT_HANDOFF.md` §5.1 — backup source
   falls back to the user's own installation). To reverse-engineer
   `ak_produktion.bci`, the next session needs read access to a real
   `Against_Rome` install directory (specifically
   `SYSTEM/CLAK/SCRIPT/ak_produktion.bci`), or a `Backup.zip` containing it.
   Not present anywhere in this repo currently — confirmed via
   `find ... -iname "*.bci"` etc. returning nothing under `data/`/`tools/`.
2. **Decompress and disassemble it** using `GameLZSS.DecompressPfil` (see
   `src/Core/GameLZSS.cs`) and the opcode table in
   `docs/reverse-engineering/bci0-opcodes.md`. Look for:
   - `s_getTime()`-based deadline arithmetic near the production-gate site at
     `0x3710` (same neighborhood, since gate and interval logic are usually
     colocated in a production tick handler).
   - Any `pushlit <ms>` immediately compared/added against a stored
     timestamp variable, similar to the deadline pattern documented in
     `docs/reverse-engineering/endless-mode-ai.md:203-209` for
     `ak_level.bci`'s retreat deadlines.
   - Possibly a *food-specific* branch — `ak_produktion.bci` likely handles
     all resource types generically with a resource-type parameter/array
     index, so the interval site may be shared across all resources (wood,
     stone, gold, iron, food) rather than food-only. Need to check whether a
     food-only speed-up is even isolable, or whether "faster food" really
     means "faster all-resource production" in this script.
3. **Determine NPC-scope safety.** The old rejected patch broke because it
   wasn't safely scoped to NPC-only paths and hit player buildings too. Any
   new interval patch must be verified to affect the same staffed-building
   code path players use (that's actually desired here — player wants THEIR
   food production faster) but must be confirmed not to break the
   eligibility/gate check at `0x3710` as a side effect, and not to divide
   into an infinite-loop or divide-by-zero if the interval literal reaches 0
   (compare the `Math.Max(1, ...)` floor pattern used for build/upgrade time
   at `ModifierForm.Patches.cs:1477`, `1488`).
4. Cross-check any candidate value against the save-embedded copy warning in
   `docs/AI_AGENT_HANDOFF.md:280` (`CLAK\scr.dat` embeds a script copy; must
   compare live script, save-embedded script, and in-game behavior together).

## Suggested next steps (not started)

1. Ask the user for their game install path or an extracted
   `ak_produktion.bci` so it can actually be decompressed and read.
2. Disassemble it with the existing opcode table; locate literal(s)
   controlling production tick interval and/or per-tick yield amount,
   distinguishing them from the already-known `0x3710` gate site.
3. Design a minimal, NPC-scope-safe, floor-protected interval/yield patch
   (UI toggle + `PatchEndlessAiEconomyScript`-style write), following the
   existing 10x-build-speed pattern for value floors and round-trip safety.
4. Runtime-verify in a fresh game before treating it as stable — this file's
   history (§ "Why this path is dangerous") shows static/signature matching
   alone is not sufficient proof of safety for this specific script.

No files were modified as part of this research. This is documentation only.
