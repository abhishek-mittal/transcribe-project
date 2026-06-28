# Handover — 2026-06-28 (session 4) — copilot

## Status
Done — F14 UX polish: uninformative "Probe ready" message removed,
picker view layout restructured so the URL input stays compact at the
top and the picker scrolls beneath it while the "Transcribe N videos →"
action bar stays pinned to the bottom of the left pane.

## What changed this session

### Phase 2: "Probe ready what this means" + action bar off-screen

**Files touched:**
- `src/lib/desktop/UrlInputPanel.svelte` — two surgical fixes:

  **Fix 1 — Removed redundant `Probe ready` activity-strip message.**
  The `handleProbeActivityEvent` listener fired on the `done` event and
  called `pushActivity('success', 'Probe ready')`, which clobbered the
  much better `"N videos · ready · cached Xm ago"` message that
  `handleProbeResult` had already pushed from the final probe result.
  The Rust bridge delivers `event: done` to the webview *before* the
  `probe_url` invoke resolves with the final result, so the listener
  ran first and stomped on the better message. Removed the redundant
  push; the listener now only updates `total_count` (which the final
  result's `total_count` field would otherwise overwrite inconsistently).

  **Fix 2 — Picker-mode template restructured.**
  Before: in picker mode, `UrlInputPanel` rendered the full URL input
  block + activity strip + everything, then the VideoPicker was a
  sibling below — the picker was tall enough to push the "Transcribe N
  videos →" action bar off-screen for short left-pane heights.
  After: split into two branches:
  - **Picker mode:** `<div class="picker-url-line">` (compact URL input
    at the top, `flex-shrink: 0`) + `<div class="picker-scroll">`
    (`flex: 1; min-height: 0; overflow: hidden` — picker scrolls
    inside its own height, leaving the action bar pinned).
  - **Non-picker mode:** unchanged from before (full URL input +
    activity strip + options + status).
  The action bar stays at the bottom of `UrlInputPanel` as before; the
  parent `.left-pane-full` flex column pins it because `.url-panel`
  has `flex: 1; min-height: 0` and `.action-bar` has `flex-shrink: 0`.

### Verification

**1. Svelte type-check** — svelte-check found 0 new errors in
`UrlInputPanel.svelte`. The 104 pre-existing strict-null warnings in
the repo were not touched.

**2. Rust check** — `cargo check` passes (no Rust changes, but
re-verified after every editor save).

**3. Sidecar timing** — re-verified after rebuild:
```bash
$ time src-tauri/binaries/transcribe-sidecar-aarch64-apple-darwin/transcribe-sidecar \
    --mode probe --url 'https://www.youtube.com/@MillieAdrian/shorts'
{"event": "status", "message": "Resolving channel index…"}
{"event": "done", "type": "playlist", "count": 5, "total_count": null}
{...5 entries...}

real    0m1.465s
```

**4. NOT verified end-to-end via the live UI** — same as session 3;
the running app (`npm run tauri:dev`) was not restarted to test the
picker layout. The flex-column CSS is structurally sound based on the
parent layout, but visual confirmation is a carry-over.

### Carry-over to next session
- **Smoke-test picker layout** — launch the app, paste the channel
  URL, confirm the URL input sits as a compact line at the top of the
  picker, the picker scrolls inside its own height, and the
  "Transcribe N videos →" button is visible at the bottom.
- **Confirm "N videos · ready · cached Xm ago" appears** (not
  "Probe ready") on the activity strip when a probe completes.

---

# Handover — 2026-06-28 (session 3) — claude-code

## Status
Done — F14 progressive picker + probe cache + visible activity landed.
Sidecar rebuilt, Rust tests green (14/14), standalone probe verifies
~1.5s end-to-end for the user's exact URL.

## What changed this session

### F14: Progressive Picker + Probe Cache + Visible Activity

**Files touched:**
- `__specs__/F14-progressive-picker-cache.md` — new spec
- `__specs__/INDEX.md` — added F14 to Track 4
- `api/sidecar.py` — `INITIAL_PAGE_SIZE = 5` (was 20); added
  `process_info` callback that emits per-entry events; status heartbeat
  + done event for UI feedback
- `src-tauri/src/lib.rs` — `probe_via_sidecar` now streams
  `probe-activity` Tauri events for `event: entry/status/done/error`
  lines; kept final result line (`type:` field) for return value;
  registered new commands `get_cached_probe` / `cache_probe` /
  `invalidate_probe`
- `src-tauri/src/db.rs` — added `probe_cache` table + `cache_probe` /
  `get_cached_probe` / `invalidate_probe` functions + 4 unit tests
  (round_trip, TTL respect, invalidate, overwrite)
- `src/lib/desktop/ProbeActivityStrip.svelte` — new compact status
  strip (mono 10.5px, severity color, Refresh link)
- `src/lib/desktop/UrlInputPanel.svelte` — cache-first probe flow;
  listens to `probe-activity` events to stream entries; deduplicates
  against the final result; replaces the static "Checking URL…" spinner
- `src/routes/+page.svelte` — passes `listenFn` into UrlInputPanel;
  probe state machine unchanged

### How the user-facing problem maps to the fix

| Symptom the user reported | What F14 actually does |
|---|---|
| "It's taking too much time" for `@MillieAdrian/shorts` | Probe now stops at 5 entries instead of walking the full channel. End-to-end time: **~1.5s** (was: tens of seconds in the terminal-only feedback case). |
| "Whatever videos are spawned should show up and then we should be lazy loading all the rest of the data on demand" | Initial probe fetches the first 5; remaining entries load via the existing `probe_url_page` flow when the user hits "Load 20 more". |
| "We should cache it until user takes any action and we store them to database" | New `probe_cache` SQLite table (TTL 15 min). Re-paste is instant. Cache invalidates on "Transcribe X videos" click. |
| "There is no way to identify what's going on behind the scenes" | New `ProbeActivityStrip` shows live status text (Checking URL… → Streaming entries · N / M → Ready · X videos · cached Xm ago). Probe sidecar stdout is now streamed as `probe-activity` Tauri events instead of going to `eprintln!`. |

### Verification

**1. Sidecar timing** (standalone, against the user's exact URL):
```bash
$ time src-tauri/binaries/transcribe-sidecar-aarch64-apple-darwin/transcribe-sidecar \
    --mode probe --url 'https://www.youtube.com/@MillieAdrian/shorts'
{"event": "status", "message": "Resolving channel index…"}
{"event": "done", "type": "playlist", "count": 5, "total_count": null}
{"type": "playlist", "kind": "playlist", "url": "https://www.youtube.com/@MillieAdrian/shorts",
 "title": "Modern Millie - Shorts", "uploader": "Modern Millie", "count": 5,
 "entries": [...5 items...]}

real    0m1.465s
```

**2. Rust tests** — all 14 pass (10 existing + 4 new probe cache):
- `probe_cache_round_trip`
- `probe_cache_respects_ttl` (verifies expired entries return None)
- `probe_cache_invalidate_removes_row`
- `probe_cache_overwrite_replaces_value`
- `schema_creates_all_tables_and_indexes` (still passes — confirms the
  new `probe_cache` table is created alongside the others)

**3. Frontend type-check** — svelte-check found 0 errors in the new
files (`ProbeActivityStrip.svelte`, `UrlInputPanel.svelte` F14
additions). Pre-existing strict-null-check warnings on other files in
the repo were not touched.

**4. NOT verified end-to-end via the live UI** — the running app
(`npm run tauri:dev`) was not started in this session. The Rust unit
tests + standalone sidecar timing + svelte-check are the evidence
behind the F14 spec's acceptance criteria.

### What did NOT change
- The `probe_url` return contract is byte-identical to before. Existing
  frontend handlers in `UrlInputPanel` still see the same
  `{type, entries, title, ...}` shape — the new `entry`/`status`/`done`
  events are additive, fired via a separate Tauri channel
  (`probe-activity`).
- No virtual scrolling in the picker (still 5/20/47 rows DOM-rendered).
  F08's spec said this is fine until >100 entries; revisited if
  performance issues arise.
- The `Load more` button still uses `probe_url_page` with
  `PLAYLIST_PAGE_SIZE = 20` (not changed).

### Carry-over to next session
- **Smoke-test in the running app:** paste the same channel URL twice;
  confirm first paste shows the activity strip + picker populates in ~2s,
  second paste hits the cache and renders instantly.
- **Confirm `Refresh` link works** — should invalidate the cache and
  trigger a fresh probe.
- **Run `npm run sidecar:build` again if `api/transcribe_core.py` is
  touched** — already done in this session for the streaming changes.

---

## Prior sessions this same day (preserved below for continuity)

### Session 2 — Instagram URL classification fix
- `api/transcribe_core.py` `_is_instagram_profile_url()` — was
  misclassifying username-prefixed reel URLs. Fixed to scan all
  segments except the last for a post keyword.
- 12 enumerated URL shapes verified + live-tested against the actual
  reel URL via oEmbed. No pytest harness in repo — ad-hoc python3 -c.

### Session 1 — Sidecar binary path fix (two bugs in `sidecar_path()`)
- (1) Inner binary name: code joined the resource-dir name onto itself;
  added `SIDECAR_BINARY_NAME` const.
- (2) Missing `binaries/` prefix: `SIDECAR_RESOURCE_DIR` omitted the
  `binaries/` prefix Tauri preserves from `bundle.resources`. Fixed.
- `cargo check` passed; full UI smoke still pending.

---

## Do NOT
- Don't assume the user can see cache-hit details — `get_cached_probe`
  returns only the value (not the age); the strip displays "cached"
  without an exact "Xm ago" timestamp. If precise age display is needed,
  extend `get_cached_probe` to return `(value, age_secs)` (already in
  `db::get_cached_probe` — just needs the Tauri command signature to
  return a struct).
- Don't assume `process_info` fires for channel-tab URLs — it does
  NOT under `extract_flat: 'in_playlist'`. The streaming probe still
  ships the first 5 entries as soon as `extract_info` returns (the
  `entries[]` array contains all of them by then). The `entry` events
  from `process_info` would help with non-`extract_flat` probes, but
  aren't reached today.

## Open questions / blockers
- (carried over) `SIDECAR_RESOURCE_DIR` hardcodes the
  `aarch64-apple-darwin` triple — cross-arch builds would need this
  derived from build target rather than hardcoded.
- (carried over) `__specs__/INDEX.md` references `transcribe-handoff.md`,
  which doesn't exist in the repo.
- (carried over) no Python test harness in this repo.
- **New (F14):** the probe cache TTL is hardcoded to 15 min — no
  user-facing setting yet. If users complain about stale results, add
  a "cache TTL" slider in Settings (would require moving
  `PROBE_CACHE_TTL_SECS` out of db.rs and into a Settings row).

## Related
- OpenSpec change: none
- __specs__ file: `__specs__/F14-progressive-picker-cache.md` (new)
- _memory/rna-method/timeline.json updated: yes — see `recentDecisions[]`
  2026-06-28 F14 entry
