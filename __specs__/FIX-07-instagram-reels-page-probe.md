# FIX-07 — Instagram Reels Page Probe (Profile Listing → Video Picker)

## Priority
P1 — Requires valid Instagram sessionid cookie (already supported via existing cookie flow).

## Depends on
FIX-05 (Instagram cookie infrastructure already in place), F12 (picker must be working).

---

## Current state

Pasting `https://www.instagram.com/thelanguageblondie/reels/` returns an immediate error:

```
"Instagram profile pages aren't supported. Paste an individual reel URL."
```

This is `IG_PROFILE_UNSUPPORTED` from `_is_instagram_profile_url()` in `transcribe_core.py`, which blocks all `/reels/` tab URLs. There is no way to bulk-select reels from an IG profile the way you can with a YouTube channel shorts tab.

---

## Why yt-dlp can't do this

- `InstagramUserIE` — explicitly marked `_WORKING = False` in yt-dlp source. Uses old GraphQL `rhx_gis` / `query_hash` approach Instagram disabled.
- `InstagramIE` — works for individual reels (`/reel/<id>/`), NOT for listing pages.
- `/username/reels/` URL — not matched by any extractor. yt-dlp returns "Unsupported URL".

No amount of cookies fixes this — yt-dlp's listing extractor is broken at the code level, not an auth level.

---

## What does work (2025/2026)

Instagram's internal mobile API, still functional with a valid `sessionid` cookie:

**Step 1 — resolve username → user_id**
```
GET https://www.instagram.com/api/v1/users/web_profile_info/?username=<username>
Headers:
  X-IG-App-ID: 936619743392459
  Cookie: sessionid=<value>
  User-Agent: Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15
```
Response: `data.user.id` (numeric string)

**Step 2 — paginate reels**
```
POST https://www.instagram.com/api/v1/clips/user/
Headers:
  X-IG-App-ID: 936619743392459
  Cookie: sessionid=<sessionid>; csrftoken=<csrftoken>
  X-CSRFToken: <csrftoken>
  Content-Type: application/x-www-form-urlencoded
Body (URL-encoded):
  target_user_id=<user_id>&page_size=12&max_id=<cursor_or_empty>
```
Response: `{ items: [ { media: { code, thumbnail_url, caption: { text }, video_duration, ... } } ], paging_info: { more_available, max_id } }`

From `media.code` → construct `https://www.instagram.com/reel/<code>/` — this URL is what `InstagramIE` downloads.

**Cookie source** — already handled by `_inject_ig_browser_cookies()` and the existing Netscape cookies file at `~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt`. The `sessionid` and `csrftoken` values are parsed directly from whichever cookie source is active.

---

## After state

| URL pasted | Before | After |
|---|---|---|
| `instagram.com/thelanguageblondie/reels/` | Instant error | Probe runs, returns picker with reels list |
| `instagram.com/thelanguageblondie/` (bare profile) | Instant error | Same — treated as reels tab |
| `instagram.com/thelanguageblondie/reels/` (no cookies) | Instant error | Clear message: "Log in to Instagram to browse profile reels" |
| `instagram.com/reel/<id>/` | Works (individual) | Unchanged |

The video picker shows each reel as a row: thumbnail, caption (truncated to 60 chars as title), duration. User selects any subset and queues for transcription. Individual reel downloads use the existing `InstagramIE` yt-dlp path (unchanged).

---

## Target components

1. **`api/transcribe_core.py`** — `_is_instagram_profile_url()` currently blocks all profile URLs. Change: don't block, let `probe_url` handle them.
2. **`api/sidecar.py`** — `probe_url()` function, Instagram branch. Currently calls `_probe_ig_oembed` for non-profile and returns `IG_PROFILE_UNSUPPORTED` for profile. New: add `_probe_ig_reels_page()` for profile/reels URLs.
3. **`api/transcribe_core.py`** — add `_parse_sessionid_from_cookies()` and `_parse_csrftoken_from_cookies()` helpers.

No Rust changes. No frontend changes (picker already handles playlist-shaped results). No new Tauri commands.

---

## New function: `_probe_ig_reels_page(url, page_size=12, max_id=None)`

Lives in `api/sidecar.py` alongside `_probe_ig_oembed`.

```
function _probe_ig_reels_page(url, page_size=12, max_id=None):

  1. Parse username from URL path
     - strip trailing slash, take last non-empty segment that isn't "reels"
     - instagram.com/thelanguageblondie/reels/ → "thelanguageblondie"
     - instagram.com/thelanguageblondie/ → "thelanguageblondie"

  2. Read sessionid + csrftoken from cookies
     - call _read_ig_session_from_cookies()
     - if no sessionid found → return error IG_LOGIN_REQUIRED with message:
       "To browse Instagram reels, log in to Instagram in Safari and save
        your cookies (Settings → Instagram → Save cookies), then try again."

  3. GET /api/v1/users/web_profile_info/?username=<username>
     - headers: X-IG-App-ID, Cookie, User-Agent (iPhone UA)
     - if 401/403 → return IG_LOGIN_REQUIRED (sessionid expired)
     - if 404 → return INVALID_URL "Instagram user not found"
     - parse: user_id, is_private, full_name, profile_pic_url

  4. if is_private → return error IG_PRIVATE_ACCOUNT
     "This Instagram account is private. You can only transcribe reels from
      public accounts."

  5. POST /api/v1/clips/user/
     - body: target_user_id=<user_id>&page_size=<page_size>&max_id=<max_id or "">
     - headers: X-IG-App-ID, Cookie, X-CSRFToken, Content-Type
     - parse items: each item.media → { code, thumbnail_url, caption.text, video_duration }
     - construct entry: { id: code, title: caption[:80] or "(no caption)", 
                          thumbnail: thumbnail_url, duration: video_duration,
                          url: "https://www.instagram.com/reel/<code>/" }
     - parse paging_info: { more_available, max_id }

  6. return {
       type: "playlist",
       kind: "ig_reels",
       url: url,
       title: "<full_name>'s Reels",
       uploader: full_name,
       entries: [...normalised entries],
       count: len(entries),
       total_count: None,          ← Instagram doesn't expose total reel count
       next_cursor: max_id if more_available else None
     }
```

---

## New helper: `_read_ig_session_from_cookies()`

Lives in `api/transcribe_core.py`.

Reads the Netscape cookies file (or browser session) and returns `(sessionid, csrftoken)` tuple. Both values are needed — `sessionid` for auth, `csrftoken` for the POST's `X-CSRFToken` header.

```python
def _read_ig_session_from_cookies() -> tuple[str, str]:
    """Return (sessionid, csrftoken) from the active IG cookie source.
    
    Returns ('', '') if no cookies are available or cookies contain no
    sessionid — caller must check and return IG_LOGIN_REQUIRED.
    """
    cookies_path = _ig_cookies_file_path()
    if not _is_valid_netscape_cookies(cookies_path):
        return '', ''
    
    sessionid = ''
    csrftoken = ''
    with open(cookies_path) as f:
        for line in f:
            line = line.strip()
            if line.startswith('#') or not line:
                continue
            parts = line.split('\t')
            if len(parts) < 7:
                continue
            domain, _, _, _, _, name, value = parts[:7]
            if 'instagram.com' in domain:
                if name == 'sessionid':
                    sessionid = value
                elif name == 'csrftoken':
                    csrftoken = value
    return sessionid, csrftoken
```

---

## Load more (pagination)

The existing `probe_url_page()` function handles YouTube pagination via `playliststart`/`playlistend`. For Instagram we need cursor-based pagination.

**Add a new mode to `probe_url_page()`:**

```
probe_url_page(url, page_start=None, page_end=None, ig_cursor=None)
```

- If `ig_cursor` is present AND url is an IG profile URL → call `_probe_ig_reels_page(url, max_id=ig_cursor)`
- Frontend passes `ig_cursor` (the `next_cursor` from the previous response) instead of `page_start`/`page_end`

**Frontend change:** when `kind === "ig_reels"` and `next_cursor` is set, the "Load more" button passes `{ igCursor: result.next_cursor }` to `probe_url_page` instead of `{ pageStart, pageEnd }`.

This is a small frontend change to `UrlInputPanel.svelte` — the picker itself doesn't change.

---

## `probe_url()` routing change

In `sidecar.py`, the Instagram branch currently:

```python
if _is_instagram_url(url):
    if _is_instagram_profile_url(url):
        return { "type": "error", "code": "IG_PROFILE_UNSUPPORTED", ... }
    return _probe_ig_oembed(url)
```

Change to:

```python
if _is_instagram_url(url):
    if _is_instagram_profile_url(url):
        return _probe_ig_reels_page(url)   # new path
    return _probe_ig_oembed(url)           # individual reel — unchanged
```

---

## Error states and messages

| Condition | Error code | User-facing message |
|---|---|---|
| No cookies saved | `IG_LOGIN_REQUIRED` | "To browse Instagram reels, log in to Instagram in Safari and save your session cookies (Settings → Instagram → Save cookies), then try again." |
| Sessionid expired (401/403) | `IG_SESSION_EXPIRED` | "Your Instagram session has expired. Go to Settings → Instagram → Save cookies to refresh." |
| User not found (404) | `INVALID_URL` | "Instagram user not found: @username" |
| Private account | `IG_PRIVATE_ACCOUNT` | "This Instagram account is private." |
| Rate limited (429) | `RATE_LIMITED` | "Instagram rate limit hit — wait a few minutes and try again." |
| Network error | `NETWORK` | standard |

All these map to existing error display in the probe UI — no frontend changes needed.

---

## Before/after user flow

**Before:**
1. User pastes `instagram.com/thelanguageblondie/reels/`
2. Probe fires instantly → "Instagram profile pages aren't supported" error appears
3. No way to bulk-transcribe an IG creator's reels

**After (cookies saved):**
1. User pastes `instagram.com/thelanguageblondie/reels/`
2. Probe fires → spinner "Resolving channel index…"
3. Picker appears: 12 reels listed with thumbnails, captions as titles, durations
4. User selects reels → queues for transcription
5. "Load more" appends next 12 (cursor-based)
6. Individual reel downloads use existing InstagramIE path (no change)

**After (no cookies):**
1. Paste same URL
2. Probe fires → returns `IG_LOGIN_REQUIRED` error
3. UI shows: "To browse Instagram reels, save your cookies first → Settings → Instagram"

---

## Download throttling (account protection)

When processing a queue that contains Instagram reel items, the pipeline must insert a random delay of **2–5 seconds between each IG reel download** to avoid triggering Instagram's anti-bot rate limits.

This applies in `runPipelineJob` (frontend) or wherever `download_item` is called for IG URLs — not between YouTube items, only Instagram ones.

**Why this matters:** The app passes the user's real `sessionid` cookie to every download request. Instagram correlates rapid back-to-back requests from the same session as automated behaviour. A 2–5s random sleep between IG downloads costs almost nothing in practice (transcription takes far longer) but meaningfully reduces the bot signal.

**Implementation:** In the download runner, after each `download-done` event for an item whose URL contains `instagram.com`, sleep a random 2–5 seconds before starting the next download in the chunk.

**This is a low-risk, personal-use app** — no proxy rotation, no burner accounts, no 30-120 second delays needed. The throttle is a precaution, not a hard requirement. Recommend the user use a secondary IG account for heavy use (>50 reels/session), but do not enforce this in the app.

---

## What doesn't change

- Individual reel probe: `_probe_ig_oembed` unchanged
- Individual reel download: `InstagramIE` via yt-dlp, unchanged
- Cookie saving flow: `_inject_ig_browser_cookies`, `_ig_cookies_file_path`, Rust `save_instagram_cookies` — all unchanged
- YouTube channel/shorts probing: unchanged
- Picker component: unchanged (already handles `kind: "playlist"` and `kind: "ig_reels"` via the same entry shape)
- YouTube downloads: no throttle added — only IG URLs get the delay

---

## Sidecar rebuild required

Any change to `api/sidecar.py` or `api/transcribe_core.py` requires rebuilding the PyInstaller binary:

```
cd api && pyinstaller transcribe-sidecar.spec --noconfirm
```

---

## Acceptance criteria

1. Pasting `instagram.com/<username>/reels/` or `instagram.com/<username>/` (bare profile) with valid cookies saved: probe returns a picker with reels list (≥1 entry), thumbnails visible, captions as titles, durations shown.
2. Pasting the same URL with NO cookies saved: probe returns `IG_LOGIN_REQUIRED` with a clear message pointing to Settings → Instagram.
3. Pasting the same URL with an expired sessionid (401 from API): returns `IG_SESSION_EXPIRED` with a message to refresh cookies.
4. Pasting a private account URL: returns `IG_PRIVATE_ACCOUNT` with appropriate message.
5. "Load more" button appears when `next_cursor` is returned; clicking it appends the next 12 reels to the picker.
6. Selecting reels from the picker and clicking "Add to queue" queues each reel as an individual item. Each item's URL is `instagram.com/reel/<code>/` — the format `InstagramIE` can download.
7. Queued reels download and transcribe successfully using the existing cookie-authenticated yt-dlp path.
8. Individual reel URLs (`instagram.com/reel/<id>/`) continue to work exactly as before — `_probe_ig_oembed` is unchanged.
9. `instagram.com/thelanguageblondie/reels/` and `instagram.com/thelanguageblondie/` both route to `_probe_ig_reels_page` (same function, same result).
10. No changes to Rust layer or frontend picker component required.
11. When a queue contains IG reel items, there is a 2–5 second random delay between each IG download. YouTube items in the same queue are not delayed.

---

## Note for the coding agent

**`_WORKING = False` on `InstagramUserIE` is the blocker** — do not try to use yt-dlp for the listing step. The function `_probe_ig_reels_page` makes raw HTTP calls directly to Instagram's internal API. yt-dlp is only used for the individual reel download step, where `InstagramIE` is still working.

**User-Agent must be iPhone** — Instagram's `/api/v1/clips/user/` endpoint returns empty results with a desktop UA. Use `Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1`.

**`X-IG-App-ID: 936619743392459`** — this is Instagram's own web app ID, required in headers for both the profile lookup and the clips endpoint.

**csrftoken in X-CSRFToken header** — required for the POST. Without it the clips endpoint returns 403. Read it from the same cookies file as sessionid.

**`next_cursor` is the `max_id` field from `paging_info`** — pass it back as-is in the next POST request. It's an opaque string, not a page number.

**Test standalone before wiring into probe_url:**
```bash
python3 -c "
from api.sidecar import _probe_ig_reels_page
import json
result = _probe_ig_reels_page('https://www.instagram.com/thelanguageblondie/reels/')
print(json.dumps(result, indent=2, default=str))
"
```
Expected: `type: playlist`, `kind: ig_reels`, entries list with real reel codes.
