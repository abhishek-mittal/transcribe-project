# §base — Shared Agent Protocol

> All agents in this collective inherit these standard sections.
> Only overrides and unique content go in individual agent files.

---

## §Step1 — Intake Protocol

On every invocation, before doing anything else:

1. **HANDOFF check**: If invoked with `[HANDOFF from @<agent>]`:
   - Read [[agent-context|_memory/rna-method/agent-context.json]] → find matching `joinId`
   - Load every file in `artifacts[]` — this is your full context
   - Proceed directly with the assigned task

2. **RESUME check**: If invoked with `[RESUME: <task-slug>]`:
   - Read [[agent-context|_memory/rna-method/agent-context.json]] → find checkpoint matching `<task-slug>`
   - Read the checkpoint file at `path`
   - Reconstruct from `decisions[]`, `filesChanged[]`, `remainingWork[]`
   - Continue from first `remainingWork[]` item

3. **Normal invocation**: Read relevant context (memory, conventions, codebase).

---

## §Handoff — Protocol

When handing off to the next agent in a join:

```
§handoff(from:@<self>, to:@<next>, join:<joinId>, step:N/M)
  context: <1-2 sentences: what was done, key decisions>
  artifacts: <comma-separated file paths>
  task: <exactly what the next agent should do>
→ @<next> [HANDOFF from @<self>] <task line>
```

Before handing off:
1. Complete your memory write (§memory-write below)
2. Update [[agent-context|agent-context.json]] — add step to `completedSteps[]`, artifacts to `artifacts[]`
3. Output the handoff block above
4. Tell user: copy last line → fresh chat thread

---

## §JoinComplete — Terminal Agent Close

If you are the terminal agent in a join:

```
§join-complete(id:<joinId>, pattern:<name>)
  agents: @a → @b → @c
  built: [file list]
  open: [follow-ups]
```

Then: remove join from [[agent-context|agent-context.json]] `activeJoins[]`, delete checkpoint file if any.

---

## §Checkpoint — Context Hygiene

Checkpoint when: >20 turns, >5 files read, or losing thread.

```
§checkpoint(slug:<task-slug>)
  decisions: [d1, d2]
  files: [f1, f2]
  remaining: [r1, r2]
→ Resume: @<self> [RESUME: <task-slug>]
```

Write to `_memory/rna-method/checkpoints/<YYYY-MM-DD>_<task-slug>.json`.
Add pointer to [[agent-context|agent-context.json]] `checkpoints[]`.

---

## §memory-write — Session Log

After completing work, write a dated session log:

```
Path: _memory/agents/<agent-id>/YYYY-MM-DD_<task-slug>_session.md
```

Contents: what was done, decisions made, files changed, follow-ups.

---

## §rna-state — RNA Network State Hygiene

Keeping `_memory/rna-method/` current is **mandatory** — single source of truth for the team.

### Before every task
1. Read [[timeline|_memory/rna-method/timeline.json]] → note `activePhase`, `recentDecisions`, open questions.
2. Read [[agent-context|_memory/rna-method/agent-context.json]] → note active joins, open checkpoints, blockers.

### After every task
1. Write session log (§memory-write above).
2. Append to [[timeline|timeline.json]] `recentDecisions[]`:
   `{ "date": "YYYY-MM-DD", "agent": "<id>", "decision": "<what>", "rationale": "<why>" }`
3. If `projectState` changed: update [[timeline|timeline.json]] `projectState`.
4. Clear resolved items: remove completed checkpoints from [[agent-context|agent-context.json]].
5. Output the §task-complete block.

---

## §task-complete — Post-Task Output

After every task, output:

```
§task-complete(@<agent>)
  status:    ✅ Done | ⚠️ Partial | ❌ Blocked
  what:      <1-2 sentences: what was delivered>
  files:     [<created / modified paths>]
  decisions: [<key decisions made>]
  next-actions:
    - [@<agent> or You] <specific action>
  open:      [<blocker or follow-up question>]
```

Rules:
- Output for **every task**, including small ones.
- `next-actions` MUST have at least one item.
- For join handoffs: output §task-complete first, then §handoff.

---

## §limits — Common Hard Limits

- Never skip reading existing code/context before acting
- Never commit credentials, secrets, or API keys
- Never leave `console.log` / `debugger` in production paths
- Be honest about gaps — say "I don't know" rather than hallucinate
- Follow the project's type system strictly — no `any` without justification
- Keep context usage efficient — see TOON registry for canonical abbreviations

---

## §lifecycle-hooks — Session Lifecycle (v1.2.0)

RNA agents participate in a lifecycle that captures and compresses knowledge automatically.

### onSessionStart
1. Load compact observation index from `_memory/observations/index.tsv` (Layer 1 — titles only, ~100 tokens/entry).
2. Load recent session summaries from `_memory/context/` (last 3 sessions max).
3. If a relevant prior observation exists, use `/rna.recall <topic>` to load Layer 2 (timeline) or Layer 3 (full details) on demand.
4. Check [[agent-context|_memory/rna-method/agent-context.json]] for pending signals assigned to this agent.

### onToolComplete (async, non-blocking)
After significant tool executions (file reads, terminal commands, API calls), capture a one-line observation:
```
[YYYY-MM-DD HH:mm] <agent-id> | <tool> | <1-line summary of what was learned>
```
Append to `_memory/observations/raw/{date}_{session-id}.tsv`.

### onSessionEnd
1. Run `/rna.compress` to convert raw observations into structured learnings.
2. Write session log per §memory-write.
3. Update timeline per §rna-state.

---

## §resilience — Ralph Loop (v1.2.0)

When a command or operation fails:

1. **Analyze**: Read the error message. Identify root cause category:
   - `missing-dep` — package/tool not installed
   - `bad-input` — wrong arguments or file path
   - `api-error` — external service failure
   - `permission` — access denied
   - `unknown` — unclear error
2. **Search**: Look for a solution in project docs, error messages, or known patterns.
3. **Fix**: Attempt the most likely fix (install dep, correct path, retry with backoff).
4. **Retry**: Re-execute the original operation.
5. **Log**: If still failing after `maxRetries` (default: 3), log to `_memory/failures/{date}_{error-slug}.md`:
   ```
   ## Failure: <error-slug>
   - Agent: <id>
   - Error: <message>
   - Attempts: <count>
   - Tried: <list of fixes attempted>
   - Status: UNRESOLVED | RESOLVED
   ```
6. **Escalate**: Report the failure to the user. Do NOT loop infinitely.

This is bounded retry — NOT autonomous iteration. For open-ended loops, use `/rna.loop`.

---

## §progressive-context — 3-Layer Retrieval (v1.2.0)

Context is loaded progressively to save tokens:

- **Layer 1 — Index** (~100 tokens/entry): `_memory/observations/index.tsv`
  Columns: `date | agent | type | title | file_ref`
  Loaded automatically at session start. Gives the agent awareness of past work.

- **Layer 2 — Timeline** (~300 tokens): On-demand via `/rna.recall <query>`
  Returns chronological narrative around matching observations.

- **Layer 3 — Full Details** (~500-1000 tokens): On-demand via `/rna.recall <query> --full`
  Returns complete observation records with tool outputs and learnings.

**Rule**: Always start with Layer 1. Only drill into Layer 2/3 when a specific past observation is relevant to the current task.

---

## §output-modes — Token-Efficient Output (v1.2.0)

RNA supports three output modes. Set via `outputFormat` in schema or toggled per-session with `/rna.toon`.

| Mode | Description | When to use |
|------|-------------|-------------|
| `verbose` | Full prose output (default) | Human-facing work, explanations |
| `toon` | Compressed abbreviations from TOON registry | Agent-to-agent handoffs, long sessions |
| `structured` | JSON output for machine consumption | Pipeline integration, logs |

When `toon` mode is active:
- Use TOON acronym table for known abbreviations (RSC, RCC, PC, DS, AC, TC, etc.)
- Compress file lists: `[3 files: app/, lib/, test/]` instead of listing each
- Truncate large outputs: `"... (N items omitted, /rna.recall to expand)"`
- Skip decorative prose — use `key: value` pairs

---

## §loop-protocol — Autonomous Iteration (v1.2.0)

When `/rna.loop` is invoked, follow this protocol:

1. **Parse goal**: Extract target metric, max iterations, guard command.
2. **Baseline**: Run metric command, record initial value.
3. **Iterate** (max N times, default 10):
   a. Read current state
   b. Plan one focused change
   c. Execute the change
   d. Run metric command — record new value
   e. Run guard command — check for regressions
   f. If metric improved AND guard passes → keep change, log iteration
   g. If metric worsened OR guard fails → rollback change, log failure
4. **Report**: Output iteration summary table:
   ```
   | # | Change | Metric | Delta | Guard | Status |
   |---|--------|--------|-------|-------|--------|
   ```
5. **Stop** when: metric target met, max iterations reached, or 3 consecutive failures.
6. **Log**: Write iteration history to `_memory/loops/{date}_{slug}/iterations.tsv`.

**Safety**: The human can interrupt at any time. If guard is not provided, each change must be manually approved.

---

## §upgrade-protocol — RNA Collective Upgrade (v1.2.0)

When `/rna.upgrade` is invoked:

1. **Snapshot**: Save current project's RNA customizations:
   - Agent names, personas, custom system prompts
   - Project-specific rules and skills
   - Custom hooks and joining patterns
   - `_memory/` contents (never overwritten)
2. **Diff**: Compare current RNA version against latest release template.
3. **Merge**: Apply new base protocol sections, commands, and schema fields while preserving:
   - All project-level customizations (agent names, personas, rules)
   - All `_memory/` data
   - Custom hooks and skills not in the base template
4. **Validate**: Run `tools/validate-registry.js` to ensure merged config is healthy.
5. **Report**: Show what was added, what was preserved, and what needs manual review.
