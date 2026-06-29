# Copilot Instructions — transcribe-project

> Auto-generated from RNA schema v1.0.0. Edit `.rna/rna-schema.json` and re-run the adapter to update.

> **Cross-tool note:** `AGENTS.md` at the repo root is read alongside this file and is
> the shared source of truth across Claude Code, Copilot, and OpenCode — session
> start/end protocol, `__handovers__/CURRENT.md`, and which planning system
> (OpenSpec / `__specs__` / `_memory`) to use for what. The Project Context table below
> is stale in places (still says Vercel serverless; the project moved to Tauri desktop +
> bundled sidecar) — `AGENTS.md` has the corrected description.

## Project Context

| Field | Value |
|-------|-------|
| Project | transcribe-project |
| Description | Web app that transcribes audio from URLs using yt-dlp + faster-whisper (Python Flask API) with a SvelteKit/TypeScript frontend, self-hosted on a Vultr VPS. |
| Domain | web-app |
| Stack | Python (Flask + Gunicorn) · SvelteKit · TypeScript · faster-whisper · yt-dlp |
| Deployment | Vultr VPS (Terraform + Nginx + systemd) |

All agents should use this project context when making decisions about code style, tooling, and architecture.

## Development Standards

Write simple, readable code. Use early returns. Minimal diffs — change only what the task requires. DRY principle. Prefix event handlers with 'handle'. Document public functions with JSDoc.

## Context Router

Before responding, check if the request matches an existing rule, skill, or agent. Suggest the match to the user. Never mention the router when no match is found.

## Agent Collective

| Agent | Role | Invoke |
|-------|------|--------|
| Developer | Full-Stack Developer | `/dev <task>` |
| Reviewer | Code Reviewer & Quality Gate | `/review <task>` |
| Architect | System Architect & Optimization Lead | `/arch <task>` |
| Director | Routing & Orchestration | `/director <task>` |
| Designer | UI/UX & Design System | `/designer <task>` |
| Ops | Operator / Automation Specialist | `/ops <task>` |
| Researcher | Explorer / Researcher | `/researcher <task>` |

## Available Skills

| Skill | Owner Agent | Trigger Keywords |
|-------|-------------|------------------|
| Smart Dev Agent | developer | implement, build, fix, debug, optimize, refactor |
| Design Quality | developer | audit UI, normalize, polish, critique, distill, harden, design quality |

## Engineering Discipline

These apply to every agent, regardless of which one is active:

| Rule | When it applies |
|------|------------------|
| `tdd-workflow.instructions.md` | Before writing any implementation code — write a failing test first |
| `systematic-debugging.instructions.md` | Before fixing any bug — reproduce, isolate, and confirm root cause first |
| `verification-before-completion.instructions.md` | Before claiming anything is done, fixed, or passing — run it and show evidence |

## Slash Commands

| Command | Agent | Description |
|---------|-------|-------------|
| `/dev` | developer | Invoke Developer agent |
| `/review` | reviewer | Invoke Reviewer agent |
| `/arch` | architect | Invoke Architect agent |
| `/director` | director | Invoke Director (routing/orchestration) |
| `/designer` | designer | Invoke Designer agent |
| `/ops` | ops | Invoke Ops agent |
| `/researcher` | researcher | Invoke Researcher agent |

## Session Protocol

**At the start of every session, the active agent must:**
1. Read `_memory/rna-method/timeline.json` — note the current phase, last decisions, open questions.
2. Read `_memory/rna-method/receptors.json` — identify active signal routes for this agent.
3. Announce: "I am [Agent Name]. I see [N] active signals. [Summary or 'queue is clear.']"

**At the end of every session:**
1. Archive key decisions to `_memory/agents/<id>/YYYY-MM-DD_<task-slug>_session.md`.
2. Update `knownDecisions[]` and `openQuestions[]` in `timeline.json`.
3. Record the exact stopping point if work is incomplete.
