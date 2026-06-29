# FIX-04 — Probe URL: YouTube Channel / Search Generator Crash

## Priority
P0 — Blocks all YouTube channel, shorts, and search-results probes. Without this fix, pasting any of the following URL types causes a silent crash inside `probe_url` and the user sees a spinner that never resolves.

---

## Current state

`probe_url` in `api/sidecar.py` calls yt-dlp with `process=False` in two places:

1. **Generic playlist branch** (`ydl.extract_info(url, download=False, process=True)` — already updated) — fetches channels, playlists, and shorts pages.
2. **Search results branch** (`ytsearch20:<query>`) — same issue.

**Verified broken URLs:**
- `https://www.youtube.com/@MillieAdrian/shorts` → crashes
- `https://www.youtube.com/@MillieAdrian/videos` → crashes
- `https://www.youtube.com/results?search_query=ipad+air+m4` → crashes

**Root cause:** When yt-dlp is called with `process=False`, the `entries` key in the returned dict is a **lazy generator**, not a list. Any call to `len()`, `list()`, or iteration on `info.get('entries')` without first converting it raises `TypeError: object of type 'generator' has no len()`. The error is swallowed and the probe returns nothing to the frontend.

**Confirmed working fix:** `process=True` + `list(info.get('entries') or [])`. Tested:
- `@MillieAdrian/shorts` → 20 entries in 0.8s ✓
- `@MillieAdrian/videos` → 20 entries in 1.1s ✓
- `ytsearch20:ipad air m4` → 20 results in 1.6s ✓

**Second issue (same fix area):** `extract_flat='in_playlist'` does not populate `thumbnail` on individual entries for channel pages. Entry thumbnails come back as empty string. Fix is described in FIX-06 (thumbnail fallback from video ID).

---

## After state

| URL type | Before | After |
|---|---|---|
| `youtube.com/@Channel/shorts` | Silent crash, spinner hangs | Returns playlist result with 20 entries |
| `youtube.com/@Channel/videos` | Silent crash, spinner hangs | Returns playlist result with 20 entries |
| `youtube.com/results?search_query=...` | Silent crash, spinner hangs | Returns search result with up to 20 entries |
| `youtube.com/watch?v=...` (single) | ✓ already works | Unchanged |
| `youtube.com/playlist?list=...` | ✓ already works | Unchanged |

---

## Target file

**`api/sidecar.py`** — two changes, no other files touched.

---

## Exact changes

### Change 1 — Generic playlist branch

Find the block that calls `ydl.extract_info` for non-Instagram, non-search URLs (roughly line 530–555). It currently has:

```python
ydl_opts = {
    "quiet": True,
    "no_warnings": True,
    "skip_download": True,
    "extract_flat": "in_playlist",
    "socket_timeout": 8,
}

try:
    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(url, download=False, process=False)
```

Change to:

```python
ydl_opts = {
    "quiet": True,
    "no_warnings": True,
    "skip_download": True,
    "extract_flat": "in_playlist",
    "socket_timeout": 10,
    "playlistend": 20,          # cap at 20 entries on first probe
}

try:
    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(url, download=False, process=True)
```

Then where the code reads entries, change:

```python
entries = _normalise_entries(info.get("entries") or [])
```

to:

```python
entries = _normalise_entries(list(info.get("entries") or []))
```

### Change 2 — Search results branch

Find the `ytsearch` block (roughly line 485–515). It currently has:

```python
with yt_dlp.YoutubeDL(ydl_opts) as ydl:
    info = ydl.extract_info(synthetic_url, download=False, process=False)
```

Change `process=False` → `process=True`.

Then:

```python
raw_entries = info.get("entries") or []
entries = _normalise_entries(raw_entries)
```

Change to:

```python
raw_entries = list(info.get("entries") or [])
entries = _normalise_entries(raw_entries)
```

---

## `_normalise_entries` — no change needed

The existing `_normalise_entries` function already handles absolute URL reconstruction from bare video IDs. No changes needed there. FIX-06 extends it to add thumbnail fallback.

---

## Before / after user flow

**Before:**
1. User pastes `https://www.youtube.com/@MillieAdrian/shorts`
2. Spinner appears in URL input panel
3. Spinner never stops — probe crashed silently
4. User sees no feedback and cannot proceed

**After:**
1. User pastes `https://www.youtube.com/@MillieAdrian/shorts`
2. Spinner appears ("Checking URL…")
3. Within 2 seconds: left pane shows "Found 20 videos · @MillieAdrian"
4. Right pane switches to video picker with 20 rows

---

## Verification steps

After applying the fix, run this test directly in the repo root:

```
python3 -c "
import sys, json
sys.path.insert(0, '.')
# Import only the probe function without loading WhisperModel
import yt_dlp
urls = [
    'https://www.youtube.com/@MillieAdrian/shorts',
    'https://www.youtube.com/@MillieAdrian/videos',
    'https://www.youtube.com/results?search_query=ipad+air+m4',
]
opts = {'quiet':True,'no_warnings':True,'skip_download':True,'extract_flat':'in_playlist','socket_timeout':10,'playlistend':20}
for url in urls:
    with yt_dlp.YoutubeDL(opts) as ydl:
        info = ydl.extract_info(url, download=False, process=True)
    entries = list(info.get('entries') or [])
    print(f'OK {len(entries)} entries | {url[-50:]}')
"
```

Expected: three `OK 20 entries` lines, all under 3 seconds each.

---

## Acceptance criteria

1. Pasting `https://www.youtube.com/@MillieAdrian/shorts` into the URL input produces a playlist probe result with at least 1 entry within 5 seconds.
2. Pasting `https://www.youtube.com/@MillieAdrian/videos` produces the same.
3. Pasting `https://www.youtube.com/results?search_query=ipad+air+m4` produces a search result with at least 1 entry within 5 seconds.
4. Single YouTube video URLs (`/watch?v=...`) continue to work as before — no regression.
5. The app never shows an infinite spinner for these URL types.
6. `playlistend: 20` is set so channel probes return at most 20 entries and complete quickly.

---

## Note for the coding agent

This is a two-line change in `api/sidecar.py`. Do not modify `_normalise_entries`, `probe_url`'s Instagram branch, the download flow, or any Rust files. The only risk is a regression on single-video URLs — verify those still work after the change.

After the Python fix, rebuild the sidecar binary:
```
cd api && pyinstaller transcribe-sidecar.spec --noconfirm
```
Then hot-reload the Tauri dev server to pick up the new binary.
