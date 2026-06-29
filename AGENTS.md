# AGENTS.md — transcribe-project

This file is the shared entry point for **every** coding agent working in this repo —
Claude Code, GitHub Copilot, OpenCode, or any other tool. Read this before anything
tool-specific (`CLAUDE.md`, `.github/copilot-instructions.md`, `.opencode/` rules).

The problem this file solves: the same repo is worked on by multiple agent tools
interchangeably, in separate sessions, with no shared memory by default. This file plus
`__handovers__/CURRENT.md` and `_memory/` are how state actually crosses that boundary.

## Project

Desktop app (Tauri + SvelteKit/TypeScript frontend, Rust shell) that transcribes audio
from any URL — YouTube, Instagram, TikTok, Twitter/X, 1000+ sites via yt-dlp — using a
local Python sidecar (`faster-whisper`) bundled as a PyInstaller binary. All processing
is on-device; no server in the loop for transcription itself.

> Note: `.rna/rna-schema.json` and `_memory/rna-method/*` still describe an earlier
> "SvelteKit + Python serverless, deployed to Vercel" shape of this project. The project
> has since moved to local-first Tauri + bundled sidecar (see `src-tauri/`, `README.md`,
> `transcribe-sidecar.spec`). Trust this file and `README.md` over those stale fields
> until someone regenerates the RNA schema.

Stack: SvelteKit (TS, Svelte 4) UI · Tauri 2 (Rust) desktop shell · Python sidecar
(`api/sidecar.py`, `api/transcribe_core.py`) built with PyInstaller · yt-dlp + faster-whisper.

Commands:
```bash
npm run tauri:dev        # run the desktop app in dev
npm run sidecar:build     # rebuild the PyInstaller sidecar — required after any
                          # change to api/sidecar.py or api/transcribe_core.py
npm run check             # svelte-check + tsc
npm run dev:all           # web-only dev (vite + flask api), no Tauri shell
```

## Read this first, every session

1. **`__handovers__/CURRENT.md`** — what the last session (any tool) did, what's in
   progress, what to do next, and what NOT to redo. This is the single most
   important file for cross-tool continuity. If it doesn't exist or looks stale
   (>1 week old, or describes a state that doesn't match the working tree), say so.
2. **`_memory/rna-method/timeline.json`** — `currentPhase`, `recentDecisions`,
   `openQuestions`. Tracked in git — this is shared state, not a local scratchpad.
3. **`__specs__/INDEX.md`** — if the task is a known feature/fix, check whether a
   spec already exists before designing from scratch.
4. **`openspec/changes/`** — `openspec list` (or read the directory) to see if a
   formal change is already in flight that touches the area you're about to edit.
5. **`.learnings/index.md`** — if the task touches yt-dlp invocation, sidecar/binary
   packaging, YouTube/Instagram bot-detection workarounds, local SQLite/queue state, or
   anything resembling a problem a similar app has already shipped a fix for, check this
   first. It indexes reference codebases (currently: `neodlp-main`, a shipped Tauri +
   yt-dlp desktop downloader) kept specifically to mine for implementation patterns and
   error-handling knowledge — read for ideas, never copy code wholesale.

## Which planning system to use

Three systems coexist on purpose — they're different grain sizes, not redundant:

| System | Grain | Use for | Lives in |
|---|---|---|---|
| **OpenSpec** | Formal, multi-file change | New capabilities, anything needing a design doc + spec deltas + reviewable task breakdown | `openspec/changes/<name>/` |
| **`__specs__/`** | Lightweight single-file spec | One concrete fix or small feature with a clear before/after and verification steps — not worth a full OpenSpec change | `__specs__/F##-*.md`, `__specs__/FIX-##-*.md` |
| **`_memory/rna-method/`** | Live session/task state | What's currently being worked on, decisions made, signals/blockers between agent roles | `_memory/rna-method/*.json` |

Rule of thumb: if it needs a design tradeoff or touches multiple files/specs →
OpenSpec. If it's a scoped, well-understood fix → `__specs__/`. Either way, log the
outcome in `_memory/rna-method/timeline.json` and `__handovers__/CURRENT.md` so the
next session (possibly a different tool) doesn't rediscover it.

Do not invent a fourth system. Do not duplicate a spec that already exists in one
system into another.

## Session protocol (applies regardless of which tool you are)

**At session start:**
1. Read `__handovers__/CURRENT.md` and `_memory/rna-method/timeline.json`.
2. If picking up in-progress work, state explicitly what you're resuming and from
   where — don't silently restart analysis already done.

**At session end** (always, even for small tasks):
1. Update `_memory/rna-method/timeline.json` — append to `recentDecisions[]` with
   `{ date, agent, decision, rationale }`; update `openQuestions[]` if any opened
   or closed.
2. Overwrite `__handovers__/CURRENT.md` using the template in
   `__handovers__/README.md`. This is the part another tool will actually read —
   keep it honest and specific, not a status-report formality.
3. If the session used an OpenSpec change or `__specs__` file, leave it in a state
   where its own status reflects reality (tasks checked off, INDEX.md updated).

## Engineering discipline (all agents, all tools)

These are enforced for Copilot via `.github/instructions/*.instructions.md` — the
same rules apply here regardless of tool:

- **Test-first.** Write a failing test before implementation code. See
  `.github/instructions/tdd-workflow.instructions.md`.
- **Root cause before fix.** Reproduce, isolate, confirm a hypothesis before
  changing code. See `.github/instructions/systematic-debugging.instructions.md`.
- **Evidence before "done."** Run the test/build/check and show the output before
  claiming something passes or is fixed. See
  `.github/instructions/verification-before-completion.instructions.md`.
- Minimal diffs, no `console.log`/`debugger`/`print()` debug statements left in,
  no hardcoded secrets, Zod-equivalent validation on the Python API's external
  inputs.

## Tool-specific files (load after this one)

- **Claude Code** → `CLAUDE.md` (repo root, if present) + `.claude/skills/`,
  `.claude/commands/opsx/`.
- **GitHub Copilot** → `.github/copilot-instructions.md`, `.github/agents/*.agent.md`,
  `.github/instructions/*.instructions.md`, `.github/prompts/opsx-*.prompt.md`.
- **OpenCode** → `.opencode/skills/`, `.opencode/commands/`.

All three already have OpenSpec (`opsx:*`) wired in independently — that part is
already in sync. What was missing was a place for cross-tool session continuity
and engineering discipline that isn't Copilot-specific; this file and
`__handovers__/` are that place.
