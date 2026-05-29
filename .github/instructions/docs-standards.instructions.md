---
description: "Documentation standards: structure, epistemic labels, source requirements"
applyTo: "**/*.{ts,tsx}"
---

# Documentation Standards

Documentation standards: structure, epistemic labels, source requirements

# Documentation Standards

These rules apply to all documentation files (`docs/`, `README.md`, `CHANGELOG.md`, agent files, rule files).

---

## Frontmatter (for `.md` and `.mdx` files in `docs/` and `content/`)

Required fields:
```yaml
---
title: <human-readable title>
description: <one-sentence summary>
date: YYYY-MM-DD
status: draft | review | published
---
```

Optional fields:
```yaml
tags: [tag1, tag2]
related: [path/to/related.md]
```

---

## README.md Requirements

Every module, package, or sub-directory with significant logic must have a `README.md` with:

1. **Purpose** — one paragraph explaining what this module does.
2. **Usage** — a working code example or CLI invocation.
3. **API** (if applicable) — key exports with their signatures and purpose.
4. **Configuration** — any environment variables or config files.

---

## JSDoc Standards

Required on all exported functions in `lib/` and `api/`:

```typescript
/**
 * Loads a content file and returns its parsed frontmatter and body.
 * @param filePath - Absolute path to the `.md` or `.mdx` file.
 * @returns ContentFile with `meta` and `content` fields, or throws if not found.
 */
export async function loadContent(filePath: string): Promise<ContentFile> { ... }
```

- Use `@param` for each parameter.
- Use `@returns` to describe the return value.
- Use `@throws` if the function throws in known conditions.
- Single-line `/** ... */` is acceptable for trivial functions.

---

## CHANGELOG.md Format

Follow [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) — semver grouping:

```markdown
## [1.2.0] — YYYY-MM-DD
### Added
- New feature description

### Changed
- What was modified

### Fixed
- Bug fixed

### Removed
- What was removed

### Deprecated
- What is being deprecated and when it will be removed
```

---

## Documentation Freshness

- API route documentation must be updated when the route changes.
- Agent files must be updated when agent responsibilities change.
- Schema reference must be updated with every schema version bump.
- README examples must be tested against the current codebase — no stale examples.

---

## Prohibited Documentation Patterns

- **Vague descriptions:** "handles stuff" or "does things" — be specific.
- **Stale examples:** code samples that are not runnable against the current API.
- **Summary-only files:** Do not create `CHANGES.md`, `SUMMARY.md`, or catch-all doc files unless explicitly requested.
- **Undated entries:** Every doc change should have a date or be tied to a version.

**Stack:** TypeScript / Node.js  
**Trigger keywords:** document, docs, readme, write up, spec  
**Owner agent:** Any