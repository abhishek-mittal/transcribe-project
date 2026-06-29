---
description: "Run and show evidence before claiming a task is done, fixed, or passing"
applyTo: "**/*.{ts,tsx,py}"
---

# Verification Before Completion

Run and show evidence before claiming a task is done, fixed, or passing

# Verification Before Completion

This rule governs how work is declared finished. It applies before saying "done", "fixed",
"passing", or opening a PR.

---

## Evidence Before Assertions

Never claim any of the following without having just run the check and observed the output:

| Claim | Required evidence |
|---|---|
| "Tests pass" | Actual test run output (`npm run test`, `pytest`) showing pass, in this session |
| "TypeScript compiles" | `npx tsc --noEmit` (or `npm run check`) output, in this session |
| "Bug is fixed" | The original reproduction (manual steps or failing test) now succeeds |
| "Build works" | `npm run build` completes without error, in this session |
| "Feature works in the UI" | Manually exercised in a running dev server (or Playwright), not inferred from reading code |

If a check wasn't run, say so explicitly: "I haven't run the test suite — want me to?" Do not imply
verification happened when it didn't.

---

## Minimum Bar Before Marking a Task Complete

1. Run the relevant test suite — not just the new test, the surrounding file/module.
2. Run `npx tsc --noEmit` (or `npm run check`) for TypeScript/Svelte changes.
3. For UI changes: start the dev server and exercise the golden path plus at least one edge case in a browser. Type checking is not a substitute for this.
4. For API changes: hit the endpoint (curl, test client) with both a valid and an invalid input.
5. Re-read the diff once, end to end, before presenting it as final.

## Minimum Bar Before Opening a PR

All of the above, plus the `pr-description.instructions.md` checklist in full — including the
security gate and the three-section PR description (What changed / Why / Testing evidence).

---

## Anti-Patterns

- **Narrating intent as if it were a result** — "this should fix it" presented as "this fixes it."
- **Reading code and concluding it's correct** without executing it, when execution is possible.
- **Declaring tests green from memory** of a prior run instead of re-running after the latest change.
- **Skipping verification because the change looks small.** Small changes still break things; the
  cost of checking is far lower than the cost of a false "done."

If a verification step genuinely cannot be run (no test runner available, no browser access),
state that limitation explicitly rather than silently skipping it.

**Stack:** TypeScript / Python
**Trigger keywords:** done, fixed, complete, passing, ready, finished, ship it
**Owner agent:** reviewer
