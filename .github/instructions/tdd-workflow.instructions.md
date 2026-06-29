---
description: "Test-first development: write a failing test before implementation code, every time"
applyTo: "**/*.{ts,tsx,py}"
---

# TDD Workflow

Test-first development: write a failing test before implementation code, every time

# Test-Driven Development

This rule governs how new features and bug fixes are implemented. It applies before any
implementation code is written — not as a follow-up step.

---

## The Cycle: RED → GREEN → REFACTOR

### RED — Write a failing test first
- Write the smallest test that captures the behavior being added or fixed.
- Run it and confirm it **fails** for the expected reason (not a typo, import error, or syntax error).
- For a bug fix: the failing test must reproduce the bug. If the test passes against the buggy code, it isn't testing the bug.

### GREEN — Make it pass with minimal code
- Write the simplest implementation that makes the test pass.
- Do not add behavior the current test doesn't require.
- Run the full test file (not just the new test) and confirm everything is green.

### REFACTOR — Clean up with tests green
- Remove duplication, improve naming, simplify — only with passing tests as a safety net.
- Re-run tests after every refactor step.
- Stop when the code is clear; do not refactor adjacent code outside the task scope.

---

## Non-Negotiables

- **No implementation before a failing test exists**, for both new features and bug fixes.
- **Never modify a test to make it pass** without re-confirming the test still expresses the original intended behavior. If the requirement changed, say so explicitly before changing the test.
- **One behavior per test.** Tests that assert multiple unrelated things should be split.
- **Tests must be deterministic** — no reliance on timing, network, or external services (per existing testing-standards rule). Mock at the boundary, not the unit under test.

---

## When TDD Doesn't Fit

Skip the strict cycle only for:
- Exploratory spikes explicitly marked as throwaway (must be deleted or formalized with tests before merge).
- Pure config/markup changes with no logic branch.
- Generated code (e.g. RNA schema adapters) where the generator is the unit to test, not the output.

If skipping, state why in the PR description.

---

## Test Placement

Follow `testing-standards.instructions.md` for file naming and structure:
- TypeScript/Svelte: mirror source tree under `__tests__/` or co-located `*.test.ts`.
- Python (`api/`): co-locate as `test_*.py` or under `api/tests/`, run with `pytest`.

**Stack:** TypeScript / Python
**Trigger keywords:** implement, add feature, fix bug, new endpoint, new function
**Owner agent:** developer
