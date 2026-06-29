# FIX-06 — Thumbnail Fallback for Flat-Extract Playlist Entries

## Priority
P1 — Depends on FIX-04 (entries now correctly returned as a list). Without this fix, every video row in the picker and queue shows a grey placeholder box — no thumbnail. The UI is functional but looks bare and makes it hard to identify videos visually.

---

## Current state

When yt-dlp fetches a YouTube channel or playlist with `extract_flat='in_playlist'`, individual entries **do not include thumbnail URLs** — the `thumbnail` field comes back as an empty string or missing entirely. This is by design in yt-dlp's flat-extract mode: it skips per-video metadata fetches for speed.

Confirmed from test output:
```
[0] title=Day in the life of a full-time mom & content  
[0] thumb=                    ← empty
[0] duration=None             ← also missing for /shorts
```

The current `_normalise_entries` function passes the empty thumbnail straight through:
```python
"thumbnail": entry.get("thumbnail") or "",
```

Result: every entry in `VideoPicker` and `QueueView` renders a grey placeholder box instead of the actual video thumbnail.

---

## After state

Every YouTube video entry in the picker and queue shows the correct thumbnail image, constructed from its video ID using YouTube's standard CDN URL format. No additional network round-trip is needed — YouTube serves thumbnails at predictable public URLs.

| Location | Before | After |
|---|---|---|
| VideoPicker row | Grey placeholder box | Video thumbnail (mqdefault quality) |
| QueueView row | Grey placeholder box | Video thumbnail |
| Single video preview card | ✓ already correct (full metadata path) | Unchanged |

---

## Target file

**`api/sidecar.py`** — `_normalise_entries` function only. No other files touched.

---

## How YouTube thumbnail URLs work

YouTube serves thumbnails at predictable CDN URLs based on video ID:

```
https://i.ytimg.com/vi/<VIDEO_ID>/mqdefault.jpg   ← 320×180, always exists
https://i.ytimg.com/vi/<VIDEO_ID>/hqdefault.jpg   ← 480×360, always exists
https://i.ytimg.com/vi/<VIDEO_ID>/maxresdefault.jpg ← 1280×720, may 404 for older videos
```

`mqdefault` (medium quality, 320×180) is the safest choice:
- Always exists for all public videos
- Fast to load (small file size ~10–20 KB)
- Good enough for 56×32px thumbnail cells in the picker

---

## Exact change — `_normalise_entries`

Current code:
```python
def _normalise_entries(raw_entries: list[Any]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for entry in raw_entries or []:
        if entry is None:
            continue
        entry_url = entry.get("url") or entry.get("webpage_url") or ""
        if entry_url and not entry_url.startswith("http"):
            vid_id = entry.get("id") or entry.get("url", "")
            entry_url = f"https://www.youtube.com/watch?v={vid_id}"
        out.append({
            "id": entry.get("id") or "",
            "title": entry.get("title") or entry.get("id") or "Untitled",
            "thumbnail": entry.get("thumbnail") or "",
            "duration": int(entry.get("duration") or 0),
            "url": entry_url,
        })
    return out
```

Change the thumbnail line to:
```python
"thumbnail": _thumbnail_for_entry(entry),
```

Add the helper function directly above `_normalise_entries`:
```python
def _thumbnail_for_entry(entry: dict[str, Any]) -> str:
    """Return a thumbnail URL for a playlist entry.

    extract_flat=in_playlist skips per-video thumbnail fetches, so the
    thumbnail field is usually empty. For YouTube entries, construct the
    standard CDN URL from the video ID — mqdefault always exists for public
    videos and loads fast at 320x180.

    Falls back to the entry's own thumbnail field if present (e.g. when
    full metadata was fetched, or for non-YouTube sources).
    """
    # Prefer whatever yt-dlp already gave us
    existing = entry.get("thumbnail") or ""
    if existing:
        return existing

    # Construct from video ID for YouTube entries
    vid_id = entry.get("id") or ""
    if not vid_id:
        return ""

    # Detect YouTube by checking the URL domain or by the ID format
    # YouTube video IDs are exactly 11 characters [A-Za-z0-9_-]
    entry_url = entry.get("url") or entry.get("webpage_url") or ""
    is_youtube = (
        "youtube.com" in entry_url
        or "youtu.be" in entry_url
        or "youtube.com/shorts" in entry_url
        or (len(vid_id) == 11 and all(c in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-" for c in vid_id))
    )

    if is_youtube:
        return f"https://i.ytimg.com/vi/{vid_id}/mqdefault.jpg"

    return ""
```

---

## Frontend — no change needed

`VideoPicker.svelte` and `QueueView.svelte` already render thumbnails via `<img src={entry.thumbnail} loading="lazy">` with a grey placeholder on error (per F08 and F09 specs). Once the sidecar returns a real URL, thumbnails appear automatically. The CSP fix (`img-src 'self' https: data:`) required for `i.ytimg.com` was already specified in F07 — confirm it is applied before testing.

---

## Before / after user flow

**Before:**
1. Paste `https://www.youtube.com/@MillieAdrian/shorts`
2. Picker loads 20 rows — all show grey boxes, no visual context
3. User can't tell which video is which

**After:**
1. Paste `https://www.youtube.com/@MillieAdrian/shorts`
2. Picker loads 20 rows — each shows the correct video thumbnail image
3. User can visually identify and deselect videos they don't want

---

## Acceptance criteria

1. Paste a YouTube channel URL (`@Channel/shorts`, `@Channel/videos`) — all picker rows show video thumbnails, not grey placeholders.
2. Paste a YouTube search results URL — same, all rows show thumbnails.
3. Paste a YouTube playlist URL — same.
4. Single video preview card (full metadata path) still shows thumbnail correctly — no regression.
5. If a thumbnail URL somehow fails to load (404), the row layout does not break — grey placeholder shows instead.
6. `mqdefault.jpg` is used (not `maxresdefault.jpg`) to avoid 404s on older videos.

---

## Note for the coding agent

This change is isolated to `_normalise_entries` in `api/sidecar.py`. The 11-character alphanumeric YouTube ID check in `_thumbnail_for_entry` is a heuristic — it is safe to fall through to an empty string if the ID doesn't match. Never raise an exception inside `_thumbnail_for_entry`.

After the Python fix, rebuild the sidecar binary. The frontend requires no changes beyond confirming the CSP (`img-src 'self' https: data:`) is present in `src-tauri/tauri.conf.json`.
