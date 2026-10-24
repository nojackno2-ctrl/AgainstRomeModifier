# 統一跨工具 Undo/Redo 實作計畫

## Status

- Steps 1–3：journal、穩定 token、各子系統收據、復原交換與 Scene redo 已完成。
- Step 4（4a／4b）：工具列、全域快捷鍵及面板導航已整合；面板可移動埋藏單筆，遇複合邊界則清除 journal 並提示。
- Step 5：成功 Safe Save 配合現有子系統清除歷史建立新 epoch；失敗不改 journal。載入、關閉、空白地圖、確認捨棄及還原地表清除 journal。當機復原固定依地形、材質、劇本各記一筆；起始配置、聚落及空白地形維持單筆，空白地形包含輔助旗標及浮雕快照。
- 限制：目前本地歷史在儲存時清除，因此無法跨儲存復原。水面控制值與戰役鏡頭仍無獨立 undo；當機復原不重建崩潰前時間順序。複合委派失敗採補償回復，底層若連補償都失敗，仍無交易式原子保證。真人 GUI／遊戲內驗收未執行。

2026-10-24，Codex（gpt-6.1-sol，唯讀原始碼審查，未經執行驗證）。行號為撰寫當時的 `apps/against-rome-editor/src/`，實作前請重新確認。

The journal approach is feasible for existing histories, but several adapters need stronger commit reporting. No files were changed; this is source inspection, without runtime validation. Paths below are relative to `apps/against-rome-editor/src/` unless prefixed `native/`.

Store `JournalEntry { epoch, children: Vec<SubsystemStep> }` in `EditorApp`, with subsystem identity, stable before/after tokens and diagnostic depths. Undo validates the newest entry, delegates, then transfers it to redo **only on success**; redo is symmetric. Never fall through to another subsystem after failure. Depth alone cannot identify replaced histories, branches or recovery swaps.

| Subsystem | Commit points and feasibility |
|---|---|
| Terrain: heights/collision/water bed | `terrain.rs:121` `commit_stroke()` reports changes; `:190` installs recovery. Existing `can_undo/can_redo/undo/redo` work; add tokens covering both core history and recovery swap. |
| Materials | `textures_ui.rs:955` `paint()` publishes a committed candidate at `:988`; `:1608` `end_material_stroke()` returns commit status; `:1709` `apply_blank_material()` commits at `:1720`; recovery commits at `:751`. Existing material availability/undo/redo wrappers suffice, but can implicitly finish strokes. |
| Placement DATA | `placement_ui.rs:607` `record_edit(entries)` groups committed operations; `:616` `apply_requests()` uses it. Depths and fallible undo/redo exist. `:637` `note_external_store_edit()` invalidates grouping rather than reporting a commit. |
| Nature | `app.rs:3115` `nature_end_stroke()` reports commits. Core direct edits converge on `native/against-rome-environment/src/nature/session.rs:264` `record()`. Existing availability/undo/redo work; remove `last_action` routing. |
| Script spawns | `spawn_placement_ui.rs:431` `mutate()` is the accepted-publication boundary after document synchronization; underlying commands enter `spawn_placement.rs:234` `execute()`. Availability, depths and bool undo/redo exist. Report only after synchronization succeeds. |
| Scenario events | `scenario_ui.rs:351` `commit_events()`, `:367` `add_events()`, `:390` duplicate, `:400` remove, `:411` replace, `:419` replace-all; `:325` synchronization can also create history. Availability/undo/redo exist; unify commit notifications. |
| SDL scene | `scene_ui.rs:192` `push_undo()` captures pre-edit snapshots; `:309` undo exists. **No redo**: add symmetric snapshots before journal integration. |
| Families / effects | `family_ui.rs:846` `run_copy()` edits the shared Placement store; `:840` `after_edit()` only flags change. Local undo/redo at `:831`/`:834` bypass grouping. Route through the DATA adapter. |
| Emboss / other settings | Emboss has no independent history; blank-terrain state changes at `app.rs:3575`. Attach auxiliary flags to its compound entry. Camera (`scenario_ui.rs:477`), ambience and other directly mutated settings cannot currently report undoable commits; require snapshots or explicit journal barriers. |

Every accepted new edit must clear global redo **and all participating subsystem redo stacks**, including recovery redo. Add redo-only clearing APIs; preserve undo. Finalize pending gestures before journal navigation; cancellations/no-ops emit nothing.

Safe Save 只在完整接受 baseline 後，依子系統實際清除歷史建立新 epoch；若未來所有子系統保留歷史，journal 也應保留。失敗儲存不改 journal；baseline 部分更新則由導航 token 檢查阻擋失同步。載入／關閉／捨棄清除 journal；面板單獨重新載入或重設仍以 token 檢查處理。

Recovery records one step per restored subsystem, in deterministic order—not invented pre-crash chronology. Scenario recovery includes spawns/camera, so resynchronize spawn history. Group Settlement (`app.rs:2366`), starter kit (`spawn_placement_ui.rs:1541`, including `pending_kit`) and blank terrain as atomic entries. Wave generation plus diplomacy already uses one event batch. Keep panel buttons, but route them globally; use the same dispatcher for shortcuts (`app.rs:9901`), respecting text focus.

1. **Scaffolding only:** journal types and fake adapters, disconnected from UI. Test ordering, token mismatch, failed delegation and compounds.
2. **Terrain/material pilot:** commit receipts and redo clearing; retain old routing. Test no-op/cancel, branching and recovery swaps.
3. **Remaining adapters:** DATA/Nature/spawns/events receipts; add scene redo. Test grouped edits, rejected publication and shared-store invalidation.
4. **Integration:** switch toolbar, panels and shortcuts together. Test interleaving across tools and focused text input.
5. **Lifecycle/compounds:** epochs, recovery and compound rollback. Test failed saves, discard/load and atomic undo/redo.

Target ~30 minutes per commit; step 3 is the sizing risk and may require narrower follow-up commits. Validate each with focused tests, then `cargo check`, `cargo test --workspace --locked`, and `cargo build`.
