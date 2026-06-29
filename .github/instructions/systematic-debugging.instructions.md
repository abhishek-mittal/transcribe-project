---
description: "Root cause before fix: reproduce, isolate, and explain before changing code"
applyTo: "**/*.{ts,tsx,py}"
---

# Systematic Debugging

Root cause before fix: reproduce, isolate, and explain before changing code

# Systematic Debugging

This rule governs how bugs, test failures, and unexpected behavior are investigated. It applies
before proposing or writing any fix.

---

## Mandatory Sequence

### 0. Check known knowledge first
- Check `.learnings/index.md` if the bug touches yt-dlp invocation, sidecar/binary
  packaging, YouTube/Instagram bot-detection, or local SQLite/queue state — a similar
  shipped project may have already solved the same class of problem.
- Check `__specs__/INDEX.md` — this exact bug may already be a documented `FIX-##` spec
  with a confirmed root cause and fix.

### 1. Reproduce
- Trigger the bug reliably — manually or with a failing test (see `tdd-workflow.instructions.md`).
- If it can't be reproduced, say so explicitly. Do not guess at a fix for a bug you can't see.

### 2. Isolate
- Narrow down to the smallest unit that exhibits the problem: one function, one request, one component.
- Read the actual error/stack trace/log output — do not infer from symptoms alone.
- Check recent changes in the affected area (`git log -p`, `git blame`) before assuming new code is at fault.

### 3. Form a hypothesis
- State what you believe is causing the bug, in one sentence, before touching code.
- The hypothesis must explain *all* observed symptoms — if it only explains some, keep investigating.

### 4. Verify the hypothesis
- Add a print/log statement, breakpoint, or minimal test that would prove or disprove the hypothesis.
- Confirm before writing the fix. If disproven, return to step 2 — do not pivot to a new guess without evidence.

### 5. Fix the root cause
- Fix the actual cause, not the first symptom encountered.
- If the true fix is large, a minimal patch is acceptable only if explicitly flagged as a stopgap with a follow-up noted.

### 6. Confirm the fix
- Re-run the reproduction from step 1 — it must now pass.
- Run the full surrounding test suite to check for regressions.

---

## Anti-Patterns

- **Shotgun debugging.** Changing multiple things at once and seeing if the bug goes away. You won't know which change mattered, and you may have hidden a second bug.
- **Fixing the stack trace location, not the cause.** A null check at the crash site can mask a logic error upstream.
- **Silently swallowing errors** (empty `catch`, broad `except:`) to make a symptom disappear.
- **Guessing from memory instead of reading the actual error.** Stack traces, log lines, and failing assertions are ground truth — read them.
- **Adding retries/timeouts to paper over a race condition** without understanding why the race exists.

---

## When the Bug Resists Reproduction

- Check environment differences (env vars, Node/Python version, OS — relevant given the Vultr deploy target vs local dev).
- Check for non-determinism: shared mutable state, unawaited promises, timing assumptions.
- Add logging at the suspected boundary and ask the user to reproduce with it, rather than guessing further.

**Stack:** TypeScript / Python
**Trigger keywords:** bug, broken, fails, error, unexpected behavior, regression, flaky
**Owner agent:** developer
