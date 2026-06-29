# __handovers__/

Cross-tool session continuity. The problem this solves: you (Abhishek) switch between
Claude Code, GitHub Copilot, and OpenCode on the same repo, and none of them know what
the others just did. `CURRENT.md` is the one file every tool reads at session start and
overwrites at session end — see `AGENTS.md` at the repo root for the full protocol.

## Rules

1. **One active file: `CURRENT.md`.** Not one file per session. A pile of dated files
   nobody reads is worse than no handoff file — the whole point is there's exactly one
   place to look, and it's always current.
2. **Overwrite, don't append.** When a session ends, replace `CURRENT.md` entirely. Move
   the previous version to `archive/YYYY-MM-DD-<slug>.md` first if it has anything worth
   keeping (otherwise just let it be overwritten — git history already has it).
3. **Typed sections, not free prose.** Use the template below as-is. An agent skimming
   this in 10 seconds should know: what's the status, what's the very next action, and
   what NOT to redo.
4. **State what NOT to do.** If a session tried an approach and abandoned it, say so
   explicitly. Without this, the next tool re-discovers the same dead end.
5. **Date and tool-stamp every handoff.** Cross-tool means you need to know which agent
   wrote it — behavior and blind spots differ between tools.
6. **Treat it as load-bearing, not a formality.** If `CURRENT.md` says something that
   doesn't match the working tree (e.g. claims a test passes that's now failing), that's
   a signal the handoff is stale or wrong — flag it, don't silently trust it.

## Template

```markdown
# Handover — <YYYY-MM-DD HH:MM> — <tool: claude-code | copilot | opencode>

## Status
<one line: what state is the work in — done / in-progress / blocked>

## What changed this session
- <file or area> — <what, briefly>

## Next action
<the single next concrete thing to do — not a list of options, the one thing>

## Do NOT
- <approach tried and abandoned, and why — saves the next session from repeating it>

## Open questions / blockers
- <anything unresolved that needs a human decision>

## Related
- OpenSpec change: <name, or "none">
- __specs__ file: <path, or "none">
- _memory/rna-method/timeline.json updated: <yes/no>
```

## Archive

`archive/` holds superseded handoffs by date, kept only for history — never read at
session start. If you need to know what happened three sessions ago, check
`_memory/rna-method/timeline.json` (`recentDecisions[]`) or git log, not the archive.
