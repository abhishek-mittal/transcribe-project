---
description: "Security checklist: no hardcoded secrets, input validation, injection prevention"
applyTo: "**/*.{ts,tsx}"
---

# Security Gate

Security checklist: no hardcoded secrets, input validation, injection prevention

# Security Gate

This rule is evaluated by **@reviewer** before approving any pull request.

---

## Mandatory Security Checks

### Secrets and Credentials
- [ ] No API keys, passwords, tokens, or private keys in any committed file
- [ ] No hardcoded environment-specific values (URLs, database names, account IDs)
- [ ] `.env` files are in `.gitignore`
- [ ] Any CI/CD credentials are stored as encrypted secrets, not in source

**If a secret was accidentally committed:**
1. Rotate the credential immediately — do not wait.
2. Use `git filter-branch` or BFG Repo Cleaner to scrub history.
3. Force push and notify all collaborators to re-clone.

### Input Validation
- [ ] All API route inputs validated with Zod (or equivalent schema validator)
- [ ] File path inputs validated — no path traversal (`../`, `%2e%2e`)
- [ ] No user-controlled data flows into `eval()`, `exec()`, `Function()`, or dynamic queries without sanitization

### Authentication and Authorization
- [ ] Authentication checked before data access on all protected routes
- [ ] Authorization verified — user can only access their own resources
- [ ] Session tokens have appropriate expiry
- [ ] No authentication logic bypassed in "dev mode" paths that reach production

### Dependency Safety
- [ ] New dependencies reviewed for known vulnerabilities (Snyk or `npm audit`)
- [ ] No `@latest` in `package.json` for production dependencies — pin major versions
- [ ] No `postinstall` scripts from unverified packages

### Error Handling
- [ ] Error messages do not expose internal stack traces to end users
- [ ] Error messages do not expose schema or file structure
- [ ] 500 responses return `{ error: "Internal server error" }` — not the raw exception

---

## Severity Guide for Review Comments

| Severity | Label | Merge Policy |
|---|---|---|
| P0 — Security | BLOCKER | Must fix before merge |
| P1 — Correctness | REQUEST_CHANGES | Must fix before merge |
| P2 — Standards | WARNING | Should fix; merge with tracking |
| P3 — Quality | SUGGESTION | Optional; at author's discretion |

---

## Snyk Scan Requirement

All new first-party code must be scanned with Snyk before merge:

```bash
# Code scan
snyk code test --severity-threshold=high

# Dependency scan
snyk test --severity-threshold=high
```

A clean Snyk scan (no high/critical issues) is required. Medium issues require a documented decision.

**Stack:** TypeScript / Node.js  
**Trigger keywords:** security, auth, credential, secret, vulnerability, injection  
**Owner agent:** reviewer