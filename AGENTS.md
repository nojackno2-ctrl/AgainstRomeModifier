# Cross-Agent Collaboration

This repository may be developed and debugged by multiple AI coding agents. Agents do not share conversation history or memory.

Before starting any task:

1. Read `AGENTS.md` and `AI_HANDOFF.md` if they exist.
2. Inspect the current Git branch, `git status`, and `git diff`.
3. Review relevant recent commits and previous attempts.
4. Understand existing uncommitted changes before modifying related files.

If either collaboration file is missing, create and maintain it automatically:

- `AGENTS.md` defines persistent collaboration rules.
- `AI_HANDOFF.md` records current state, objective, active problems, prior attempts, constraints, and actionable next steps.

Treat `AI_HANDOFF.md` as live shared project memory. Update it immediately after meaningful code changes, debugging attempts, failed fixes, important discoveries, build/test results, objective changes, new problems, or resolutions. Re-check it and the current Git diff after substantial changes.

Do not blindly overwrite another agent's information or repeat failed approaches without new evidence. Assume uncommitted changes may belong to the user or another agent; inspect them before editing and never discard them without explicit authorization.

Never claim a fix, successful build, or working functionality without verification. Automatic commit and push to GitHub are authorized once verified (see below); destructive operations (force-push, merge, rebase, reset, branch deletion) require explicit authorization. Git and the repository are the source of truth. Keep collaboration files concise and current.

## Project rules

- The project is Rust-only (user goal 2026-10-20: migrate the entire project to Rust; the C#/.NET, C++ and Python sources were removed after parity porting). Write all new code, probes, parsers and tools in Rust. Ghidra scripts under `tools/re/Ghidra*.java` are inputs to Ghidra's Java scripting host and are the only non-Rust sources kept.
- Preserve uncommitted user changes. The user authorized (2026-10-08) access to the installed game directory `C:\Program Files (x86)\Against Rome` for in-game verification: only write to custom test maps (e.g. `MAPS\ENDL_005`), back them up to `%TEMP%` first, and restore after testing; never touch original game files.
- Scope (user, 2026-10-14): the goal is a map editor for designing game maps, NOT a re-implementation of the game. Prioritize designer-facing features (placement, terrain, scenario/events, validation, save/load safety, in-game acceptance). Do not pursue pixel-exact native rendering fidelity (particle blend modes, >64 light selection, RNG/FPU replay) unless it blocks editing.
- Delegation (user, 2026-10-14): split work across multiple concurrent Codex and Agy subagents; both do coding AND reverse engineering, on disjoint topics/files.
- Keep behavior-preserving refactors separate from feature changes.
- Validate with `cargo test --workspace --locked` (or `tools/test-rust-rewrite.ps1 -IncludeDesktop`); release packaging is `tools/publish-rust.ps1`.
- Communicate with the user in Traditional Chinese while preserving technical names, paths, APIs, and error messages.

## Automatic commits and push to GitHub (user authorization, 2026-10-14)

- The user has authorized automatic local commits and automatic push to GitHub for all projects.
- After completing a task and appropriate verification, commit the task changes and push to GitHub (`origin`) without asking for confirmation again; do not create empty commits.
- Review the diff and preserve existing work. Include unrelated pre-existing changes only when the user explicitly requests committing them. Never commit secrets, credentials, or personal runtime data.
- This standing authorization covers local commits and pushing verified work to GitHub. Destructive operations (force-push, release, merge, rebase, reset, branch deletion) still require explicit authorization.
- A Git `post-commit` hook is configured in `.git/hooks/post-commit` to automatically push the current branch to `origin` upon commit.
- Record what was verified and any unverified behavior in `AI_HANDOFF.md`; never present a commit as proof that functionality works.
