# CLAUDE.md — transcribe-project

@AGENTS.md

Claude Code reads this file natively but does not natively read `AGENTS.md` — this
import line is how it gets pulled in. Everything that matters (project context,
session start/end protocol, which planning system to use, engineering discipline) lives
in `AGENTS.md` so it stays identical across Claude Code, Copilot, and OpenCode. Don't
duplicate content here; add Claude-Code-only specifics below if any arise.
