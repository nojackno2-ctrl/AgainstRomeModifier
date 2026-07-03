# BCI0 Script Bytecode Reference

Decoded 2026-07-02 while tracing endless-mode AI defeat/respawn logic in
`MAPS/ENDL_*/SCRIPT/ak_level.bci` and `SYSTEM/CLAK/SCRIPT/ak_npc.bci`. This is
a partial instruction set covering everything needed to read the party state
machine and external-call sites; it is not a complete disassembler
specification. Treat opcode meanings below as evidence-backed for the
sequences observed, not as a verified exhaustive VM reference.

## Container Layout

A decompressed `BCI0` script has this layout (all integers are little-endian
32-bit words unless noted):

- Offset `0x00`: magic `"BCI0"`.
- Offset `0x04`: word, observed `1`.
- Offset `0x08`: `codeSize` (byte length of the `CODEJ` code section).
- Offset `0x10`: `blobLen` (byte length of the string blob used by `CIDX`).
- Offset `0x14`: `symCount` (number of external symbol names).
- Offset `0x18`: word, observed `0x3F` in `ak_npc.bci` (purpose unconfirmed).
- Offset `0x1C`: `"CODE"` tag start; full tag is `"CODEJ"` (5 bytes) then
  padding to the next 4-byte boundary at `0x24`.
- Offset `0x24`: start of the executable code stream, `codeSize` bytes long.
  This is the section addressed by all "decompressed offset" references
  elsewhere in the reverse-engineering docs.
- `0x24 + codeSize`: `"SYMBCONS"` tag (8 bytes), then the string blob
  (`blobLen` bytes, NUL-separated symbol names), then a 4-byte `"CIDX"` tag,
  then `symCount` 32-bit byte-offsets into the blob (one per symbol, in
  declaration order — this is the external-symbol table; index into it with
  the operand of a `pushsym` (opcode `128`) instruction).
- After `CIDX`: a `"VAR "` block and a `"VIDX"` block (local/global variable
  metadata; not decoded in detail — not needed for the party-state work).

`bcitool` (ad hoc helper written for this investigation, not checked into the
repo) implements this layout in its `ReadSymbols` helper; reimplement from
this spec if the tool is not available.

## VM Dispatcher (decoded 2026-07-03 — AUTHORITATIVE)

The runtime interpreter lives in the EXE at `005B1C62` (frame setup) with the
opcode fetch at `005B1C72`: it reads the 32-bit opcode word at PC, advances PC
by 4, computes `index = opcode - 1`, bounds-checks `index <= 0xB0`, and
dispatches through the jump table at `005B199C` (unknown opcodes go to the
default handler `005B972C`). VM context (EBX): `+0x08` PC (byte offset into
the code stream), `+0x28` code size, `+0x2C` code base, `+0x0C` stack index,
`+0x3C` stack base (cells grow by realloc), `+0x04` script object (its
`+0x38` is the global-variable slot table used by opcode 77).

Operand word counts below come from each handler's PC-advance guard, NOT from
pattern heuristics. Corrections to the earlier empirical table:

- Opcodes **76, 77, 78, 83, 84, 92, 93, 64, 65, 67, 68, 69, 70, 80, 129,
  160** all take an operand word (the old table treated 76/77/78 as 1-word,
  which misaligned every listing that contained them). 67 takes TWO operand
  words. 77 pushes a REFERENCE to global-variable slot `operand`
  (`table[operand] | 0xC0000000`) — used for script-API write-back args like
  `s_timeReached(interval, &deadline, now)`.
- The conditional-jump family is **113..118**, all sharing handler `005B8377`:
  pop one value, read the operand word, then branch via the sub-table at
  `005B1980`: 112 `jmp` (unconditional, no pop), 113 jump if `< 0`, 114 `<=
  0`, 115 `> 0`, 116 `>= 0`, 117 jump if `== 0` (jz), 118 jump if `!= 0`
  (jnz). The old table's "118 = AND-chain marker" was wrong.
- **Jump/call target arithmetic**: the branch handlers add the operand AFTER
  PC has passed both the opcode and operand words, so
  `target = opcode_addr + 8 + operand`. The old claim `target = off +
  operand` in this file was wrong (the memory note had it right); under the
  correct rule the `jmp 0` fillers land on the next instruction (separators),
  not on themselves.
- **120 = call internal**: pushes the return address (PC after operand) onto
  the VM stack, then `PC = opcode_addr + 8 + operand`. 121 is its return.
- **96..103 comparison family**: each handler is standalone (0 operands).
  Compiled code emits `push a; push b; op96; op102; jz` for `if (a == b)`
  and a bare `op102` after a call for `if (0 == result)`; 96/97 peek at
  stack depth -1/-2 and 98..103 produce the final boolean (lt/le/gt/ge/eq/ne
  order per the old table appears consistent with usage, but 96/97's exact
  role — normalize/duplicate before the predicate — is only partially
  decoded).
- Opcode 82 (storevar) DOES take an operand word (script-shape evidence;
  the automated guard scan misses it because its guard sits deeper in the
  handler).
- 1-word (no-operand) opcodes confirmed by the dispatcher table: 1-6, 16, 17,
  32-45, 48-53, 71, 72, 74, 75, 82(see above), 85-89, 94-103, 121-123, 130,
  131, 144, 145, 161-166, 176, 177.

## Instruction Word Shapes (historical empirical notes)

Almost every instruction is a 2-word (8-byte) `[opcode, operand]` pair. The
old heuristic "treat unknown opcodes as 1-word" produced misaligned listings
whenever opcodes 76/77/78 appeared — re-read any older disassembly notes with
the corrected table above.

| Opcode (dec) | Opcode (hex) | Meaning | Operand |
|---|---|---|---|
| 66 | 0x42 | push literal | signed 32-bit constant |
| 81 | 0x51 | push local/global variable | variable index (negative = function parameter, e.g. `-3` = first param below the frame) |
| 82 | 0x52 | store to local/global variable | variable index |
| 90 | 0x5A | push array-local base / frame-relative array ref | index/offset (frequently paired with an immediately following `163`/`164` array read/write) |
| 91 | 0x5B | store array-local element | array slot index |
| 163 | 0xA3 | array read (indexed by the value below top of stack) | — |
| 164 | 0xA4 | array write (indexed by the value below top of stack) | — |
| 73 | 0x49 | set pending call argument count | negative count (e.g. `-2` = 2 arguments) |
| 128 | 0x80 | push external symbol reference | symbol table index (see `CIDX`) |
| 86 | 0x56 | call external symbol (consumes the pending `pushsym`+`argc`) | — |
| 120 | 0x78 | call internal script function (unconfirmed target arithmetic; see "Jump Target Arithmetic" below) | relative displacement to a script-internal function, followed by an `argc`/`86`-style call sequence |
| 112 | 0x70 | unconditional jump | `target = off + operand` (see "Jump Target Arithmetic" below) |
| 117 | 0x75 | conditional jump if top-of-stack is zero (`jz`) | same target arithmetic as `112` |
| 71 | 0x47 | pop/discard (statement terminator) | — |
| 96 | 0x60 | begin comparison (pushes marker consumed by 98-103) | — |
| 98 | 0x62 | compare: `<` (less-than) | — |
| 99 | 0x63 | compare: `<=` | — |
| 100 | 0x64 | compare: `>` (greater-than) | — |
| 101 | 0x65 | compare: `>=` | — |
| 102 | 0x66 | compare: `==` | — |
| 103 | 0x67 | compare: `!=` | — |
| 118 | 0x76 | logical AND chain marker (precedes a `jz`) | — |
| 32 | 0x20 | add (`+`) | — |
| 44 | 0x2C | logical AND / conditional combine (seen chained with `164` for `deadline := getTime() + delay`) | — |
| 37 | 0x25 | subtract or bitwise op (context-dependent; seen in mask arithmetic) | — |
| 40, 41 | 0x28, 0x29 | bitwise OR / assignment-combine variants (seen in `v18 |= 1<<team`-style mask updates) | — |
| 83 (as `pushsym` target) | — | `s_getTime` symbol call, ubiquitous for deadline arithmetic | — |

Function boundaries in the code stream are marked by a recurring five-word
marker sequence observed as raw dwords `95, 75, 121, 74, 94` immediately
followed by an `argc <paramCount>` pair, and body locals are zero-initialized
with a `pushlit 0` / array-store pair per parameter before the real body
starts. This shape is what let party-handler function starts be located
quickly (search for the `95, 75, 121, 74, 94` word run); the individual
meaning of words `95`/`75`/`121`/`74`/`94` (call-frame setup markers) was not
decoded beyond "function prologue marker" in this session.

## Jump Target Arithmetic

For a 2-word instruction at byte offset `off` (the opcode word) with opcode
`112` (`jmp`) or `117` (`jz`), the operand at `off+4` is a **relative**
displacement measured from the instruction's own start:

```
target = off + operand
```

This was verified mechanically (the disassembler tool computed targets this
way) and cross-checked against dozens of `jmp 0 -> <self>` infinite-loop
terminators, which require `operand == 0` under this convention — consistent
with every sample observed. Do not assume `off + 8 + operand` (relative to
the instruction end) — that alternative was considered but not what the
verified tool output used.

Opcode `120` (seen immediately before an `argc`/`86` external-call-shaped
sequence, e.g. `pushvar ... ; op120 ; op<large-negative> ; argc N`) behaves
like an internal-function call whose second word is a call-target reference,
but its exact target arithmetic was **not** independently verified in this
session — the disassembler treated it as an unrecognized single-word opcode
and printed its operand word as a separate line rather than pairing them.
Treat `120`'s semantics as a plausible-but-unconfirmed internal-call opcode,
not a verified instruction shape.

## Party State Numbers (`ak_level.bci`, `s_writeToConfArray`/debug-name switch at `0x8C8C`)

| Value | Name |
|---|---|
| 0 | STOP |
| 1 | IDLE |
| 16 | INIT |
| 17 | INIT_CONTROL1 |
| 18 | INIT_CONTROL2 |
| 19 | INIT_LEADER |
| 20 | INIT_UNITSCIV |
| 21 | INIT_UNITSMIL |
| 22 | INIT_SEND |
| 23 | INIT_SENDSTART |
| 32 | WAIT_CIVDISSOLVE |
| 33 | CIVRECREATE |
| 34 | CIVRECREATE_WAIT |
| 35 | WAIT_HOMEREACHED |
| 48 | RETREAT |
| 49 | RETREAT_INIT |
| 50 | RETREAT_WAIT |
| 51 | RETREAT_WAITVILLAGE |
| 52 | RETREAT_WAITUNITS |
| 256 | DELETE_PARTY |
| 257 | DELETE_TEAM |
| other | `!UNKNOWNSTATE!` (formatted as `0x%02X`) |

## Key Script-Level Arrays (party-indexed, `ak_level.bci`)

| Variable | Meaning |
|---|---|
| `v47` | party type (party-type-family raider/settler/military scripts; `0` = free slot) |
| `v48` | party state (see table above) |
| `v49` | owning team id |
| `v61` | deadline timestamp (`s_getTime() + N`); consumed by RETREAT-chain exit checks |
| `v63[type]` | live-party count, indexed by party type; drives spawner probability |
| `v18` | net-game team-occupied bitmask (multiplayer) / defeat-recycle scratch |
| `v41` | team-occupied bitmask accumulator, single-player base value `1` (protects team 0) |
| `v68` | CPU-controlled team bitmask (`255` in single player, computed via `s_lgcGetTeamCpuCtrl` per team in net games) |

## Verified Function Entry Points (`ENDL_000` decompressed `ak_level.bci`)

| Decompressed offset | Role |
|---|---|
| `0x7B94` | party creation — allocate a free slot, set `v49/v47`, state := 16, `v63[type]++` |
| `0x81E8` | occupied-team mask builder: `base \| (1 << v49[i])` for every slot with `v47[i] != 0` |
| `0xC3DC` | `pickTeam` — selects among `~occupied & tribeMask & v68` |
| `0xF144` | settled-party handler A (types 1, covers INIT_UNITSCIV/IDLE loop, first `600000`-ms deadline site) |
| `0x10688` | dead-party consecutive-failure counter comparison (`20`, patched to `3`) |
| `0x144AC` | settled-party handler B (type 4, mirrors `0xF144`) |
| `0x18828` | Siedler (settlement) spawner — rolls 80/60/40/20% by `v63[1]` |
| `0x18E8C` | military-reinforcement spawner — requires an existing team with >= 2 buildings |
| `0x1A654` | single-player defeat/mask-update handler (per-team `v72/v74/v76/v79/v80` flags) |
| `0x1AA0C` | `s_netGame`-gated defeat handler — clears `v18/v41` bits, plays `ENDL_ALL_%02i.wav`; multiplayer human-slot recycler, NOT part of AI respawn |

## Verified Function Entry Points (`SYSTEM/CLAK/SCRIPT/ak_npc.bci`)

| Decompressed offset | Role |
|---|---|
| `0x2764` | `s_setNPCActive(team, 1)` call — reactivation, guarded by a healthy-village check |
| `0x30F4` | `s_setNPCActive(team, 1)` call — the actual site that fires when an inactive team's village becomes valid again (per earlier handoff doc's uncertainty; now confirmed by context at `0x2FA4`-`0x3110`, which reads `s_NPCActive` first and only calls `setNPCActive(team,1)` when currently inactive) |
| `0x3420` | `s_setNPCActive(team, 0)` call — deactivation when the village/team state is lost |
| `0x41BC` | `s_setNPCActive(team, 0)` call — deactivation, second site (build/message-context guarded) |

Symbol indices in `ak_npc.bci`'s own `SYMBCONS` table (not stable across
scripts — always resolve via each script's own `CIDX`/blob, never hardcode):
`#58 s_getODescHandle`, `#59 s_setNPCActive`, `#64 s_compareObj`,
`#69 s_NPCActive`, `#82 s_leaderExists`.

## Tooling Note

The investigation used a throwaway .NET console tool (not committed) with
four operations: `dec` (LZSS decompress via the same algorithm as
`GameLZSS.DecompressPfil`), `hex`, `words <file> <hexOffset> <count>`,
`findw <file> <w0> <w1> ... ('?' = wildcard)`, `syms` (dump the `CIDX` table
per the layout above), `calls` (scan for `pushsym`+`argc`+`call` triples and
print resolved symbol names), and `dis` (linear disassembler using the
opcode table above). Recreate it from this spec if further BCI reverse
engineering is needed; do not assume it still exists on disk.
