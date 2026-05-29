---
description: "Generate GitHub PR descriptions with What changed / Why / Testing evidence sections"
applyTo: "**/*.{ts,tsx}"
---

# PR Description

Generate GitHub PR descriptions with What changed / Why / Testing evidence sections

# Review Gate

These standards govern how pull requests are created and reviewed in this project.

---

## PR Description Template

Every pull request description must include all three sections:

```markdown
## What changed
- `path/to/file.ts` — <what changed>
- `path/to/other.ts` — <what changed>

## Why
<1–3 sentences explaining the business or technical reason>

## Testing evidence
<how this was verified — unit tests, manual steps, screenshots, or all>
```

PRs without all three sections should not be merged.

---

## Pre-Merge Checklist

Before approving, the reviewer verifies:

- [ ] PR description has all three required sections
- [ ] TypeScript compiles without errors
- [ ] Tests pass (all existing tests green, new tests added for new behavior)
- [ ] No `console.log()`, `debugger`, or TODO comments left in production paths
- [ ] No hardcoded secrets or environment-specific values
- [ ] Security gate checks passed (see `security-gate.md`)
- [ ] API routes have Zod validation on all inputs
- [ ] Error responses follow the project's shape: `{ error: string }`
- [ ] JSDoc present on new public functions in `lib/` and `api/`

---

## Review Response Format

```markdown
## Review: <PR title>

**Verdict:** APPROVE | REQUEST_CHANGES | NEEDS_DISCUSSION

### Blockers (must fix before merge)
- [ ] <issue> → <fix required>

### Warnings (should fix)
- <issue> → <suggestion>

### Suggestions (optional)
- <observation> → <recommendation>
```

---

## Definition of Done (DoD)

A task is considered **done** when all of the following are true:

1. Feature or fix is implemented and tested.
2. All existing tests still pass.
3. TypeScript compiles without errors.
4. PR is opened with complete description.
5. Reviewer has approved (no open blockers).
6. `_memory/rna-method/timeline.json` updated — signal resolved.

---

## Merge Policy

- **Squash merge** preferred for feature PRs — keeps history clean.
- **Merge commit** for long-running branches being merged back to main.
- **No force-push to main** without notice to all active contributors.
- **Revert immediately** if a merge causes test failures on main.

**Stack:** TypeScript / Node.js  
**Trigger keywords:** PR, pull request, merge, pr description  
**Owner agent:** reviewer