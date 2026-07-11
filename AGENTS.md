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

Never claim a fix, successful build, or working functionality without verification. Do not automatically commit, push, merge, rebase, reset, force-push, or delete branches unless explicitly authorized. Git and the repository are the source of truth. Keep collaboration files concise and current.

## Project rules

- Preserve uncommitted user changes and never access or modify the installed game directory.
- Keep behavior-preserving refactors separate from feature changes.
- Validate with the documented `dotnet build` and `dotnet test` commands.
- Communicate with the user in Traditional Chinese while preserving technical names, paths, APIs, and error messages.
