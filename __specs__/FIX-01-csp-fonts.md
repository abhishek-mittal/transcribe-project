# FIX-01 — Bundle Fonts Locally (CSP Fix)

## Priority
P0 — BLOCKING. The app renders with broken fonts in the packaged `.app`.

---

## Current state
`+page.svelte` loads two typefaces from Google Fonts via `<link>` tags in `<svelte:head>`:
- **Inter** (weights 400/450/500/600/700)
- **Fraunces** (weights 400/500, regular + italic)

The Tauri security policy (`tauri.conf.json`) only allows content from `'self'`, `ipc:`, and `http://ipc.localhost`. In the packaged `.app`, all requests to `fonts.googleapis.com` and `fonts.gstatic.com` are blocked. The app falls back to the system font — the entire visual design looks broken.

This has no impact in `npm run dev` (browser mode) because the browser has open network access.

---

## After state
Both typefaces are bundled inside the app itself. The `<link>` tags are removed. Fonts load from local files — no network request, no CSP change needed.

| Location | Before | After |
|---|---|---|
| `+page.svelte` `<svelte:head>` | Two `<link>` tags pointing to fonts.googleapis.com | Removed |
| `src/lib/fonts/` | Does not exist | Contains Inter + Fraunces `.woff2` files |
| `+page.svelte` `<style>` | No `@font-face` declarations | `@font-face` blocks for all variants |
| `tauri.conf.json` CSP | Unchanged | Unchanged (no modification needed) |

---

## Target component
`src/routes/+page.svelte` — the `<svelte:head>` block (lines 384–388) and the `<style>` block (`:global(body)` font-family declaration).

---

## What to do

### Step 1 — Download font files
Download these as `.woff2` format into `src/lib/fonts/`:

**Inter** (from Google Fonts or fonts.bunny.net):
- `inter-400.woff2`
- `inter-450.woff2` (if available; fall back to 400 if not)
- `inter-500.woff2`
- `inter-600.woff2`
- `inter-700.woff2`

**Fraunces** (from Google Fonts):
- `fraunces-400.woff2`
- `fraunces-400-italic.woff2`
- `fraunces-500.woff2`

### Step 2 — Remove the `<link>` tags
In `+page.svelte`, delete the three `<link>` lines inside `<svelte:head>` (the preconnect and the stylesheet link).

### Step 3 — Add `@font-face` declarations
In `+page.svelte`, inside the `<style>` block (at the top, before `:global(*)`), add `@font-face` rules for each downloaded file. Use `url('$lib/fonts/inter-400.woff2')` path syntax (SvelteKit resolves `$lib` at build time).

Example shape (agent should write all variants):
```
@font-face {
  font-family: 'Inter';
  font-style: normal;
  font-weight: 400;
  font-display: swap;
  src: url('$lib/fonts/inter-400.woff2') format('woff2');
}
```

---

## Acceptance criteria
1. Run `npm run tauri:build` and open the resulting `Transcribe.app`.
2. The heading text renders in Fraunces (serif with visible optical size).
3. Body text renders in Inter (clean, geometric sans-serif).
4. No fallback to system fonts (San Francisco / Helvetica Neue).
5. DevTools Network panel shows zero requests to `fonts.googleapis.com` or `fonts.gstatic.com`.

---

## Note
Do not modify `tauri.conf.json` CSP. The point of Option A (bundling) is precisely that the CSP stays locked down. If the font files are too large to bundle, the agent may switch to Option B (expand CSP) — but this is the fallback, not the preference.
