---
description: "Systematic 6-phase code optimization: measure first, no premature optimization"
applyTo: "**/*.{ts,tsx}"
---

# Optimization Workflow

Systematic 6-phase code optimization: measure first, no premature optimization

# Optimization Workflow

This rule defines the mandatory process for any optimization or performance work.

---

## Golden Rule

**Measure first, optimize second.** No optimization without a baseline measurement and a clear target metric.

---

## 6-Phase Process

### Phase 1 — Define the Problem
- State the performance symptom (e.g. "page load takes 4.2s, target is < 1.5s")
- Identify the metric: latency, throughput, memory, bundle size, or other
- Set a measurable success target

### Phase 2 — Measure Baseline
- Profile with appropriate tools (DevTools, Lighthouse, `perf_hooks`, database EXPLAIN)
- Record baseline numbers in a reproducible environment
- Identify the bottleneck — do not guess

### Phase 3 — Analyze Root Cause
- Trace the hot path through the bottleneck
- Categorize: algorithmic, I/O, rendering, network, or resource
- Document the root cause before proposing solutions

### Phase 4 — Implement Fix
- Address only the identified bottleneck — one concern at a time
- Keep diffs minimal and reversible
- Add inline comments explaining "why" for non-obvious optimizations

### Phase 5 — Measure Result
- Re-measure using the same environment and tools as Phase 2
- Compare against baseline and target
- If the target is not met, return to Phase 3

### Phase 6 — Document
- Record the optimization as an ADR or session log entry
- Include: before/after metrics, root cause, solution, and trade-offs
- Update `timeline.json` with the decision

---

## Anti-Patterns

- **Premature optimization:** Optimizing code without evidence of a problem
- **Micro-benchmarking in isolation:** Optimizing a function that accounts for < 1% of total time
- **Optimizing readability away:** Sacrificing code clarity for marginal gains
- **Caching without invalidation strategy:** Adding caches without a plan for staleness

**Stack:** TypeScript / Node.js  
**Trigger keywords:** optimize, performance, benchmark, slow, latency  
**Owner agent:** architect