---
generated_by: rna-method/init.js
generated_at: 2026-05-29T18:02:07.949Z
project: transcribe-project
platform: copilot
---

# RNA Method — Session Zero

## What this project uses

| Field       | Value                      |
|-------------|-----------------------------|
| Project     | transcribe-project             |
| Description | transcribe-project — built with RNA Method      |
| Domain      | web-app           |
| Platform    | copilot                |
| Stack       | TypeScript / Node.js|
| Deploy      | local |
| Agents      | developer               |
| Init date   | 2026-05-29T18:02:07.949Z                      |

## Activate your first agent

```
@developer Implement a user authentication endpoint
```

## Personalise your agents

Run `/rna.setup` in your AI agent chat to tailor agents for your project domain,
stack, and conventions. This step contextualises each agent with your specific needs.

## Key files

| File | Purpose |
|------|---------|
| `rna-schema.json` | Source of truth — agents, rules, skills, hooks |
| `_memory/rna-method/receptors.json` | Agent registry |
| `_memory/rna-method/timeline.json` | Project state |
| `.github/copilot-instructions.md` | copilot loader |

> All RNA config is in `.rna/`, runtime state in `_memory/`.
> Add `_memory/` to `.gitignore` — it is managed by agents during sessions.

## How to re-run

```bash
# Update an existing install:
bash tools/install.sh --update

# Or with the Node installer:
node tools/init.js --update
```

## How to validate

```bash
node .rna/validate-registry.js --root ./
```
