---
description: "Test creation standards for the project test framework"
applyTo: "**/*.{ts,tsx}"
---

# Testing Standards

Test creation standards for the project test framework

# Coding Standards

These rules apply to all code changes across all contributors and agents in this project.

---

## Code Quality

- **Early returns over nested conditionals.** Fail fast; happy path last.
- **DRY principle.** No copy-pasted logic. Extract shared logic into a shared utility module.
- **Minimal diffs.** Change only what is required by the task. Do not silently refactor adjacent code.
- **JSDoc on all public functions.** Single-line `/** ... */` is sufficient for simple functions. Required for all exported functions in `lib/` and `api/`.
- **Event handler naming:** prefix with `handle` — e.g. `handleSave`, `handleKeyDown`.

---

## TypeScript

- Strict mode throughout. No `any`, no `@ts-ignore` without an explanatory comment.
- Prefer `type` over `interface` for plain data shapes.
- Use `unknown` instead of `any` for external data; narrow with Zod or type guards.
- No implicit `any` via untyped exports.

---

## Error Handling

- API routes: return `{ error: string }` with appropriate HTTP status codes.
- UI: surface user-friendly messages; never display raw error objects.
- `console.error` in dev only — remove all debug logs before committing.
- Handle all `Promise` rejections; no floating `async` calls without `await` or `.catch()`.

---

## Naming Conventions

- Filenames: `lowercase-kebab-case` everywhere.
- Variables and functions: `camelCase`.
- Types and classes: `PascalCase`.
- Constants: `SCREAMING_SNAKE_CASE` for module-level constants.
- Test files: mirror source tree — `lib/__tests__/utils.test.ts` for `lib/utils.ts`.

---

## Commit Hygiene

Every commit message must follow:

```
<type>(<scope>): <short description>

[optional body]
```

Types: `feat` `fix` `refactor` `docs` `test` `chore` `style`
Scope: the affected module (e.g. `api/save`, `components/ui`, `lib/content`)

Examples:
- `feat(api/users): add pagination support`
- `fix(components/nav): correct active link state`
- `chore(deps): update dependencies`

---

## Session Memory Update

At the end of every coding session, update `_memory/rna-method/timeline.json`:
- Mark resolved signals as complete
- Add new signals or open questions discovered during the session
- Note the exact stopping point if work is incomplete

**Stack:** TypeScript / Node.js  
**Trigger keywords:** test, spec, coverage, unit test, integration test  
**Owner agent:** reviewer