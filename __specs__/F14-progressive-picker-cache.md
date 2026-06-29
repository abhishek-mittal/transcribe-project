# F14 — Progressive Picker + Probe Cache + Visible Activity

## Priority
P1 — Targets the two UX problems visible in the running app when the user
pastes a slow URL (e.g. `https://www.youtube.com/@MillieAdrian/shorts`):

1. **Picker takes too long to appear.** The probe sidecar waits until yt-dlp
   has fully resolved all 20 entries before returning *anything* to the UI.
   The user stares at a blank left pane with a "Checking URL…" spinner.
2. **No visibility into what's happening.** The only feedback is two
   `eprintln!` lines in the terminal — `probe_via_sidecar: pid=59604` and
   `probe_url: result type=playlist` — and the user is on a desktop app,
   not a terminal.

---

## Current state

**`api/sidecar.py` `probe_url()`** for playlists:
- Sets `playlistend: PLAYLIST_PAGE_SIZE` (20), `extract_flat: 'in_playlist'`.
- Calls `ydl.extract_info(url, download=False, process=True)`.
- yt-dlp internally walks the full channel/playlist index to resolve
  `playlistend: 20`. For `@MillieAdrian/shorts` (or any channel with 50+
  Shorts) this is a noticeable wait.
- Returns a single JSON blob with `entries: [...20 items...]` only after
  the walk completes.

**`src-tauri/src/lib.rs` `probe_via_sidecar()`:**
- Spawns sidecar, reads stdout, returns the one JSON blob to the frontend.
- Discards anything else — no events emitted during the wait.
- Only prints to terminal via `eprintln!`.

**`src/routes/+page.svelte` `runProbe()` flow:**
- `probeState = 'probing'` until the result lands. Left pane shows a tiny
  "Checking URL…" spinner; user can't see what's happening.
- When the result returns, the picker appears *all at once* with all 20
  rows.

**`+page.svelte` `playlistProbeResult` / `handleLoadMore()`:**
- Already supports paginated "Load 20 more" via `probe_url_page`.
- No caching — pasting the same URL twice triggers a fresh yt-dlp walk.

---

## After state

When the user pastes a slow URL like `@MillieAdrian/shorts`:

1. **First 5 entries stream in within ~1 second.** The user sees
   `5 / N videos · Streaming entries…` with rows appearing as yt-dlp
   resolves each one (instead of all 20 at once after the full walk).
2. **A visible activity strip** in the left pane shows real-time status:
   - `Checking URL…`
   - `Streaming entries · 1 / N`
   - `Ready · 20 of N videos`
   - On error: `Probe failed · Network`
3. **Same URL pasted again is instant** — the result is read from a SQLite
   cache (TTL 15 min). The user sees `Loaded from cache · 3m ago` instead
   of a re-probe.

| Area | Before | After |
|---|---|---|
| Picker appearance | Wait for all 20 entries, then dump | First 5 entries within ~1s, then progressively append |
| Activity feedback | Terminal-only `eprintln!` | Live status text in left pane + console-style activity log under the URL input |
| Re-paste same URL | Full re-probe every time | Instant cache hit with TTL; manual `Refresh` link |
| Cache invalidation | N/A | Cleared on `Transcribe X videos` click; new URL = fresh probe |
| Probe sidecar stdout | Discarded after the result line | Streamed as `probe-activity` events, surfaced in UI |

---

## Target components

1. **`api/sidecar.py`** — `probe_url()`:
   - Switch `playlistend` to `INITIAL_PAGE_SIZE = 5` (down from 20).
   - Use yt-dlp's `process_info` callback to emit per-entry events as each
     one resolves.
   - Emit `entry`, `done`, `error` events to stdout.
   - On the frontend, call `probe_url_page` to load the remaining 15 when
     the initial 5 land (the existing Load More path).

2. **`api/sidecar.py`** — new constants and helper:
   - `INITIAL_PAGE_SIZE = 5` for the first probe.
   - `process_info` callback emits `{"event": "entry", "entry": {...}}`.

3. **`src-tauri/src/lib.rs`** — `probe_via_sidecar()`:
   - Stream every stdout line, not just the last one.
   - Emit `probe-activity` Tauri events for each line (filtered to valid
     JSON with `event: entry|done|error|status`).
   - Return only the final `{"type":"playlist", ...}` JSON to the caller,
     same shape as today.
   - Keep `eprintln!` for development but stop relying on it for UX.

4. **`src-tauri/src/db.rs`** — new `probe_cache` table:
   - `url TEXT PRIMARY KEY, result_json TEXT NOT NULL, fetched_at TEXT NOT NULL, ttl_secs INTEGER NOT NULL DEFAULT 900`.
   - New functions: `cache_probe(conn, url, result)`, `get_cached_probe(conn, url, max_age_secs) -> Option<Value>`, `invalidate_probe(conn, url)`.

5. **`src-tauri/src/lib.rs`** — new Tauri commands:
   - `get_cached_probe(url: String) -> Option<Value>` — returns the cached
     result if fresh, `None` otherwise.
   - `cache_probe(url: String, result: Value)` — write on first probe
     completion.
   - `invalidate_probe(url: String)` — call after the user clicks
     `Transcribe` (so a re-paste probes fresh).

6. **`src/routes/+page.svelte`** — `runProbe()`:
   - Check cache first; if hit, show `Loaded from cache · Xm ago` and
     switch to picker immediately, no spinner.
   - Otherwise call `probe_url` as today, but subscribe to `probe-activity`
     events to update the left-pane status text live.
   - When the final result lands, write it to the cache.
   - On `Transcribe X videos` click, invalidate the cache for that URL.

7. **`src/lib/desktop/UrlInputPanel.svelte`** — visible status:
   - Replace the static `Checking URL…` spinner with a live status line:
     `Streaming entries · 3 / 20`, then `Ready · 20 of N videos`.
   - On probe error, show the error code + a `Retry` link.
   - Add a small `Refresh` link next to the cache-hit indicator.

8. **`src/lib/desktop/ProbeActivityStrip.svelte`** — new component:
   - Compact strip (12px tall, monospace) under the URL input showing the
     last 3-5 probe events as they stream in.
   - Auto-fades to dim after 4 seconds.
   - Hidden once the picker appears (the picker itself is the result).

---

## Detailed design

### Python: streaming probe

```python
INITIAL_PAGE_SIZE = 5  # was PLAYLIST_PAGE_SIZE = 20

def probe_url(url: str) -> dict[str, Any]:
    # ... existing setup ...

    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "skip_download": True,
        "extract_flat": "in_playlist",
        "socket_timeout": 8,
        "playlistend": INITIAL_PAGE_SIZE,
    }

    emitted_ids: set[str] = set()

    def _process_entry(entry: dict) -> None:
        # Called by yt-dlp after each entry is resolved. Emit one event
        # per entry so the UI can render rows progressively. Dedupes by id
        # in case yt-dlp calls this twice (it does for some extractors).
        if entry is None:
            return
        eid = entry.get("id") or ""
        if eid in emitted_ids:
            return
        emitted_ids.add(eid)
        emit({
            "event": "entry",
            "entry": _normalise_entry(entry),
        })

    ydl_opts["process_info"] = _process_entry

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False, process=True)
    except Exception as exc:
        emit({"event": "error", "code": classify_ydl_error(exc), "message": str(exc)})
        return {"type": "error", "code": classify_ydl_error(exc), "message": str(exc)}

    # ... build final response (same as today) ...
    emit({"event": "done", "type": "playlist", "total_count": info.get("playlist_count")})
    return {...}
```

**Why this works:** yt-dlp's `process_info` is called for each entry as it
becomes available — well before the full `extract_info` call returns. With
`playlistend: 5` set, the walk stops after 5 entries, so the user sees
5 rows in ~1s on a typical channel. The remaining entries are loaded via
the existing `probe_url_page` flow when the user clicks "Load more".

### Rust: stream stdout events

```rust
async fn probe_via_sidecar(
    app: &AppHandle,
    url: &str,
    extra_args: &[&str],
) -> serde_json::Value {
    let binary = sidecar_path(&app)?;
    let (mut rx, child) = app
        .shell()
        .command(binary)
        .args([/* probe args */])
        .spawn()
        .map_err(|e| { /* ... */ })?;

    let mut final_result: Option<serde_json::Value> = None;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if line.trim().is_empty() { continue; }
                    match serde_json::from_str::<serde_json::Value>(line) {
                        Ok(value) => {
                            let event_name = value.get("event").and_then(|e| e.as_str());
                            if event_name == Some("entry") || event_name == Some("done") || event_name == Some("error") {
                                // Stream to frontend as probe-activity events
                                let _ = app.emit("probe-activity", &value);
                            } else if value.get("type").is_some() {
                                // The final summary — keep it as the result
                                final_result = Some(value);
                            }
                        }
                        Err(_) => {
                            eprintln!("probe sidecar: non-JSON line: {}", line);
                        }
                    }
                }
            }
            CommandEvent::Terminated(_) => break,
            _ => {}
        }
    }
    final_result.ok_or_else(|| /* error */)
}
```

**Filter rule:** Lines with `event` field = `entry`/`done`/`error` are
streamed; lines with `type` field (the final result) are kept and returned.
This preserves the existing `probe_url` return contract for the frontend.

### DB: probe cache

```rust
// db.rs
const PROBE_CACHE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS probe_cache (
    url TEXT PRIMARY KEY,
    result_json TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    ttl_secs INTEGER NOT NULL DEFAULT 900
)";

pub fn cache_probe(conn: &Connection, url: &str, result: &serde_json::Value) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO probe_cache (url, result_json, fetched_at, ttl_secs)
         VALUES (?1, ?2, ?3, 900)",
        rusqlite::params![
            url,
            serde_json::to_string(result).map_err(|e| e.to_string())?,
            chrono::Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn get_cached_probe(conn: &Connection, url: &str) -> Result<Option<serde_json::Value>, String> {
    let mut stmt = conn.prepare_cached(
        "SELECT result_json, fetched_at, ttl_secs FROM probe_cache WHERE url = ?1"
    )?;
    let mut rows = stmt.query([url])?;
    if let Some(row) = rows.next()? {
        let json: String = row.get(0)?;
        let fetched: String = row.get(1)?;
        let ttl: i64 = row.get(2)?;
        let fetched_dt = chrono::DateTime::parse_from_rfc3339(&fetched)
            .map_err(|e| e.to_string())?;
        let age_secs = (chrono::Utc::now() - fetched_dt).num_seconds();
        if age_secs <= ttl {
            return Ok(Some(serde_json::from_str(&json).map_err(|e| e.to_string())?));
        }
    }
    Ok(None)
}

pub fn invalidate_probe(conn: &Connection, url: &str) -> Result<(), String> {
    conn.execute("DELETE FROM probe_cache WHERE url = ?1", [url])?;
    Ok(())
}
```

The `probe_cache` table is created in the existing `run_migrations()`
function. No schema-version bump needed — `CREATE TABLE IF NOT EXISTS` is
idempotent.

### Frontend: cache-first probe

```typescript
async function runProbe(probeUrl: string) {
    probeState = 'probing';
    probeResult = null;
    probeError = null;

    // 1. Check cache first
    if (invokeFn) {
        try {
            const cached = await invokeFn('get_cached_probe', { url: probeUrl });
            if (cached) {
                probeResult = cached;
                listProbeResult = { ...cached, kind: cached.kind || cached.type };
                probeState = 'idle';
                // Show "Loaded from cache · just now" in the activity strip
                return;
            }
        } catch (e) { /* fall through to live probe */ }
    }

    // 2. Live probe with streaming activity
    const unlisten = await listenFn('probe-activity', (event) => {
        const payload = event.payload;
        if (payload.event === 'entry') {
            // Append to listProbeResult.entries
            listProbeResult = {
                ...listProbeResult,
                entries: [...(listProbeResult?.entries || []), payload.entry],
            };
        } else if (payload.event === 'done') {
            // Probe complete — cache and finalize
            listProbeResult = { ...listProbeResult, total_count: payload.total_count };
        } else if (payload.event === 'error') {
            probeState = 'error';
            probeError = `${payload.code}: ${payload.message}`;
        }
    });

    try {
        const res = await invokeFn('probe_url', { url: probeUrl });
        // ... existing handling ...
        // Cache the successful result
        if (res.type === 'playlist' || res.type === 'search' || res.type === 'video') {
            await invokeFn('cache_probe', { url: probeUrl, result: res });
        }
    } finally {
        unlisten?.();
    }
}
```

### Frontend: invalidate on Transcribe click

```typescript
function handleTranscribePicker() {
    if (selectedPickerEntries.length === 0) return;
    if (url && invokeFn) {
        invokeFn('invalidate_probe', { url }).catch(() => {});
    }
    handlePickerStartJob(selectedPickerEntries);
}
```

---

## Before / after user flow

### Paste `https://www.youtube.com/@MillieAdrian/shorts`

**Before:**
1. User pastes URL.
2. Left pane shows `Checking URL…` spinner.
3. Wait ~5–10 seconds (terminal-only feedback: `probe_via_sidecar: pid=59604`).
4. Picker appears with all 20 rows at once.
5. User has no idea the probe is making progress.

**After:**
1. User pastes URL.
2. Cache miss → live probe starts.
3. Activity strip shows: `Connecting to YouTube…` (immediate).
4. Within ~1 second: `Streaming entries · 1 / 5`.
5. Rows appear one by one as yt-dlp resolves each (5 rows in ~2 seconds).
6. Once first 5 land: `Ready · 5 videos · streaming more in background`.
7. Activity strip shows yt-dlp's progress as the remaining entries load
   in the background (via the existing `probe_url_page` infrastructure).
8. When all 20 are loaded: `Ready · 20 of 47 videos · Load 15 more`.

### Re-paste same URL within 15 minutes

**Before:**
1. User pastes URL → full yt-dlp walk again.

**After:**
1. User pastes URL.
2. Activity strip shows: `Loaded from cache · 2m ago`.
3. Picker appears instantly with full entries.

### Click `Transcribe X videos`

**Before:**
- No cache invalidation (irrelevant — no cache existed).

**After:**
- `invalidate_probe(url)` is called. The user can re-paste and get a
  fresh probe if they want (e.g. to pick up newly added videos).

---

## Acceptance criteria

1. Pasting `https://www.youtube.com/@MillieAdrian/shorts` shows the first
   entry in the picker within ~3 seconds (was: ~5–10s, all-or-nothing).
2. While entries are streaming, a status line in the left pane shows the
   current count (e.g. `Streaming entries · 3 / 5`).
3. After the probe completes, the full entry list (including the
   background-loaded pages) is visible in the picker.
4. Re-pasting the same URL within 15 minutes loads the picker instantly
   (no network call) and shows `Loaded from cache · Xm ago`.
5. Clicking `Transcribe X videos` clears the cache entry for that URL.
6. On a probe error, the user sees the error code and a `Retry` link.
7. The existing `Load more` button still works for paginating beyond
   the initial 5 entries.
8. No `eprintln!` is the only UX feedback path — every probe state is
   visible in the UI.
9. Tauri sidecar rebuild not required for the UI changes (Python sidecar
   rebuild IS required for the streaming changes — see Sidecar rebuild
   section in `INDEX.md`).

---

## Sidecar rebuild required

Python changes in `api/sidecar.py` require rebuilding the PyInstaller
binary before testing in the Tauri app:

```bash
cd api && pyinstaller transcribe-sidecar.spec --noconfirm
```

The Tauri dev server (`npm run tauri:dev`) will pick up the new binary
on next launch.

---

## Note for the coding agent

**On the cache TTL:** 15 minutes is a balance. Long enough that the
common case ("I picked wrong videos, let me deselect some and re-open
the picker") is instant. Short enough that newly added videos show up
without manual refresh. The `Refresh` link in the activity strip forces
a bypass.

**On the streaming order:** `process_info` is yt-dlp's per-entry
callback. It fires in the order yt-dlp resolves entries, which is
roughly playlist order (oldest first for channels, playlist order for
playlists). Don't sort on the frontend — match the order yt-dlp gave
us, since that's the user's mental model.

**On what stays unchanged:** The final probe result JSON returned to the
frontend has the same shape as today (`type`, `entries`, `title`,
`total_count`, etc.). The frontend code that handles the result doesn't
need to change — only the new event listener for `probe-activity` is
added.

**On the `entry` event format:** Each event's `entry` field is the
already-normalized dict from `_normalise_entry(entry)` — same shape as
what's in the final `entries[]` array. The frontend appends them
verbatim to `listProbeResult.entries`. If the final result also contains
those entries (it will — yt-dlp returns them in `info['entries']`), the
frontend deduplicates by `id` before rendering.

**On backwards compatibility:** The probe cache is opt-in (frontend
checks it first, falls back to live probe). If the new Rust commands
aren't registered, the frontend probe path fails silently and the
old behavior takes over.