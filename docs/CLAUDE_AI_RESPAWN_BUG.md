# AI Ultimate Respawn Bug — RESOLVED 2026-07-02 (static analysis; runtime acceptance pending)

## Original problem

With AI Ultimate Mode enabled on endless maps, defeated computer players never
returned. An `ENDL_002` save at game time 27:44 showed all participating teams
(1, 2, 3, 5, 6) at `npcActive=0`, every NPC-job slot empty, and no returns —
even though the then-current patch set (military delay 5000, alleged "village
delay" `0x17F38` = 5000, counts 20..20, recycle flag 1, active-party 8) was
fully applied. That save disproved the old interpretation of `0x17F38`.

## Root cause (decoded from the ak_level.bci party state machine)

`ak_level.bci` manages endless opponents as up to 16 "parties" (arrays:
`v47` type, `v48` state, `v49` team, `v61` deadline, `v63[type]` live count).
Defeat recovery is DESIGNED-IN but extremely slow in vanilla:

1. A settled party (types 1/4, handlers `0xF144`/`0x144AC`, covering both
   INIT_UNITSCIV and INIT_UNITSMIL arrivals) sits in state 1 (IDLE) and checks
   `s_getVillageCenterObj` every tick.
2. Village + leader + civilians all gone → a consecutive-failure counter
   (literal `20` at decompressed `0x1068C`) must fill before the party enters
   the RETREAT chain (states 48..52).
3. The RETREAT chain's only exit for a wiped team (no units left to walk home)
   is the deadline `v61[party] := s_getTime() + 600000` — **10 minutes** —
   set at six per-handler sites (value offsets `0x10700`, `0x119C0`,
   `0x12FFC`, `0x13FE8`, `0x160EC`, `0x17F38`).
4. Only after DELETE_PARTY (state 256) frees the slot (`v47[party] := 0`,
   `v63[type]--`) does the team leave the occupied mask (`fn 0x81E8`) and
   become eligible in `pickTeam` (`~occupied & tribeMask & cpuMask`), letting
   the spawners (Siedler probability 80/60/40/20 % by live count; plus the
   military-reinforcement and raider spawners) send a new arrival.
5. The new arrival settles; `ak_npc.bci` detects the healthy village for an
   inactive team and calls `s_setNPCActive(team, 1)` (call site `0x30F4`) —
   reactivation is native and needs no patch.

The old patch only shortened ONE of the six deadlines (`0x17F38`, actually the
type-5 handler's RETREAT_INIT deadline — not a "village respawn timer"), so
dead teams' parties stalled ~10+ minutes per phase and the user never saw a
return.

## Fix implemented (all in ak_level.bci; saves are never touched)

- All six retreat/cleanup deadlines: `600000 -> 5000` ms. BCI word signature
  `[81,61, 90,-3, 128,83, 86, 66, <ms>, 32, 44, 164]`; two same-shaped sites
  are structurally excluded: the initial-arrival timeout at `0x7F24` (no `44`
  word — shortening it would retreat parties before they can settle) and the
  military reinforcement wait `0x178E0` (followed by `pushlit 34` =
  CIVRECREATE_WAIT; patched separately as before).
- Dead-party confirmation counter `0x1068C`: `20 -> 3` ticks (signature
  `[90,24, 66,<n>, 96, 101, 117, 16, 66, 1, 91, 17]`, unique hit).
- Existing count/recycle/military-delay/polling/active-limit patches retained.
- Legacy enabled states (only `0x17F38` shortened, or none, counter 20) are
  detected as legacy-enabled and migrated on the next apply; disable restores
  every literal to the exact original values.
- Code: `FindEndlessRetreatDeadlineLiterals` / `FindEndlessDeadPartyDebounceLiteral`
  in `src/UI/ModifierForm.Patches.cs`; the misnamed
  `FindEndlessVillageRespawnDelayLiteral` and
  `EndlessAiOriginalVillageRespawnDelayMs` are removed.
- Docs/schema/localization corrected: `known-patches.md`,
  `endless-mode-ai.md` (new "Party Lifecycle And Defeat Recovery" section),
  `TechDoc.md`, `TechDoc_EN.md`, `README.md`, `game_schema.json`
  (`VillageDefeatRespawn` replaced by `PartyRetreatDeadlines`,
  `DeadPartyDebounceTicks`, `DefeatRecoveryChain`), `Localization.cs`.

Supporting EXE facts (Ghidra): `npcActive` lives at `DAT_029e6000[8]`; the
only writers are the `s_setNPCActive` callback implementation `FUN_00548ce0`
and the level-init reset `FUN_0054a070` — the defeat transition is script-only
(`ak_npc.bci` sites `0x3420`/`0x41BC` deactivate; `0x30F4` reactivates), so
script patching is sufficient and no EXE or save edit is needed. The
`s_netGame`-gated handler at `0x1AA0C` (clears `v18`/`v41` bits, voice
announcements) recycles defeated HUMAN slots in multiplayer and is unrelated.

## Hard prohibitions (unchanged)

- Never rewrite save payloads (`npc.dat`, `scr.dat`, ...): a previous attempt
  produced "Savegame corrupt or version not compatible!" despite a clean
  decompress/recompress round-trip. Self-round-trip is NOT proof of
  game-compatible serialization.
- Never shorten the initial-arrival timeout at `0x7F24`.

## Acceptance tests (still to run in the real game)

1. Fresh `ENDL_002` with AI Ultimate enabled.
2. Defeat each participating AI team (both settle-style and military-style).
3. Every defeated team's slot frees within seconds and a replacement arrival
   appears after the spawner's probability roll (expect well under a minute,
   plus march/settle travel time); the team becomes `npcActive=1` again after
   resettling.
4. Multiple defeat cycles; no team stops returning.
5. Long run to catch NPC-job leakage/stall regressions.
6. Save while teams are defeated, reload, confirm recovery still works.
7. Disabled team slots (4, 7) never activate.
8. Player resource production and manual unit conversion unaffected.
9. Disable/restore returns original bytes exactly.
10. Validate all five `ENDL_000..004` and at least one runtime defeat/recovery
    test per map variant.
