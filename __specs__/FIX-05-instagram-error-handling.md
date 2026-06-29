# FIX-05 — Instagram: Profile Page Detection + Download Error Clarity

## Priority
P0 — Instagram profile/reels page URLs currently crash the probe with "Unsupported URL". Single reel downloads fail with a confusing bot-challenge-style error. Both need clear, actionable user-facing messages.

---

## Reality audit — what actually works vs what doesn't

### What works ✓
- **Single reel probe** (`instagram.com/reel/<shortcode>`): oEmbed path returns title + thumbnail in 0.5s. No login needed.
- **Error classification**: `_INSTAGRAM_AUTH_PATTERNS` already exists in `sidecar.py` and maps to `INSTAGRAM_LOGIN_REQUIRED` code.
- **Desktop cookie injection**: `_inject_ig_browser_cookies` in `transcribe_core.py` already tries Safari/Chrome/Firefox browser cookies for download.

### What is broken ✗

**Problem 1 — Profile/reels page (`instagram.com/<username>/reels`)**
The URL `https://www.instagram.com/thelanguageblondie/reels` hits `_is_instagram_url()` → true → falls into `_probe_ig_oembed()`. But oEmbed only accepts single-post URLs, not profile pages. It returns HTTP 400. The frontend receives a generic `NETWORK_ERROR` with no useful message.

**Problem 2 — Single reel download**
After a successful probe (oEmbed works), when the user clicks Transcribe, yt-dlp hits Instagram's GraphQL API for media info. Since late 2024, Instagram returns empty media responses for all requests without a valid session — even from residential IPs. The error is correctly classified as `INSTAGRAM_LOGIN_REQUIRED` but the message shown to the user is the raw yt-dlp error string, which is technical and confusing.

**Problem 3 — No guidance in the UI**
Neither error tells the user what to do. They need to be told: "Drop a cookies file at a specific location" or "Log into Instagram in Safari first".

---

## After state

| Scenario | Before | After |
|---|---|---|
| Paste `instagram.com/<user>/reels` | oEmbed fails with `NETWORK_ERROR` | Instant clear error: "Paste individual reel URLs instead" |
| Paste `instagram.com/<user>/` (profile) | Same | Same instant clear error |
| Paste `instagram.com/reel/<id>` — probe | ✓ works | Unchanged |
| Paste `instagram.com/reel/<id>` — transcribe | Raw yt-dlp error in red | Friendly message + specific file path for cookies |
| User has cookies file at app-data path | Download attempted with cookies | ✓ unchanged (already implemented) |

---

## Target files

1. **`api/sidecar.py`** — `probe_url` function: add profile page detection before `_probe_ig_oembed`.
2. **`api/sidecar.py`** — `classify_ydl_error` or `run_transcription`: improve the user-facing message for `INSTAGRAM_LOGIN_REQUIRED`.
3. **Frontend `src/lib/desktop/UrlInputPanel.svelte`** (or wherever error messages are rendered): map `INSTAGRAM_LOGIN_REQUIRED` and `IG_PROFILE_UNSUPPORTED` error codes to readable inline messages with a file path hint.

---

## Change 1 — Profile page detection in `probe_url`

In `api/sidecar.py`, in the `probe_url` function, **before** the `_probe_ig_oembed(url)` call, add this check:

```python
# Instagram profile / listing pages (not individual posts) are not supported
# without login. Detect them early and return a helpful error instead of
# letting oEmbed fail with a confusing network error.
if _is_instagram_profile_url(url):
    return {
        "type": "error",
        "code": "IG_PROFILE_UNSUPPORTED",
        "message": (
            "Instagram profile pages are not supported. "
            "Paste individual reel URLs instead — "
            "for example: instagram.com/reel/ABC123"
        ),
    }
```

Add the helper function `_is_instagram_profile_url` near `_is_instagram_url` in `api/transcribe_core.py`:

```python
def _is_instagram_profile_url(url: str) -> bool:
    """True for instagram.com/<username>/reels, /videos, /tagged, or profile root.

    False for individual posts: /p/<id>, /reel/<id>, /tv/<id>.
    """
    if not _is_instagram_url(url):
        return False
    try:
        path = urlparse(url).path.rstrip("/")
    except ValueError:
        return False
    parts = [p for p in path.split("/") if p]
    # Individual post paths: ['p', '<id>'], ['reel', '<id>'], ['tv', '<id>']
    individual_prefixes = {"p", "reel", "tv", "stories"}
    if parts and parts[0] in individual_prefixes:
        return False
    # Profile root: ['<username>'] or listing: ['<username>', 'reels'/'videos'/'tagged']
    return len(parts) >= 1
```

Also import `_is_instagram_profile_url` in `sidecar.py` alongside `_is_instagram_url`.

---

## Change 2 — Better download error message for IG login wall

In `api/sidecar.py`, update the `_INSTAGRAM_AUTH_PATTERNS` error message. When `classify_ydl_error` returns `INSTAGRAM_LOGIN_REQUIRED`, the calling code in `run_transcription` currently passes the raw yt-dlp string to `emit_error`. Replace that string with a plain-English message.

In `run_transcription` (or the except block that catches it), add a special case:

```python
except Exception as e:
    code = classify_ydl_error(e)
    if code == "INSTAGRAM_LOGIN_REQUIRED":
        message = (
            "Instagram requires login to download this video. "
            "To fix: open Instagram in Safari and log in, then try again. "
            "Or export a cookies.txt file from your browser and place it at: "
            "~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt"
        )
    else:
        message = str(e)
    emit_error(code, message)
```

---

## Change 3 — Frontend error display

In `src/lib/desktop/UrlInputPanel.svelte` (or wherever probe errors are displayed), map the two new Instagram error codes to styled inline messages:

**For `IG_PROFILE_UNSUPPORTED`:**
```
Instagram profile pages aren't supported.
Paste an individual reel URL — e.g. instagram.com/reel/ABC123
```
Show as an amber/warning inline block (not red — it's not a failure, just a guidance nudge).

**For `INSTAGRAM_LOGIN_REQUIRED`** (shown in the queue row or transcript panel on download failure):
```
Instagram requires login to download this video.
→ Open Instagram in Safari and log in, then try again.
→ Or place a cookies.txt file at:
  ~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt
```
Show as a red error row in QueueView with the full message expandable below the title (same pattern as other failed rows in F09 spec).

---

## Before / after user flow

**Profile page paste — before:**
1. Paste `https://www.instagram.com/thelanguageblondie/reels`
2. Spinner for 10 seconds
3. Red error: "HTTPError 400" — user has no idea what to do

**Profile page paste — after:**
1. Paste `https://www.instagram.com/thelanguageblondie/reels`
2. Amber inline message appears instantly (< 100ms, no network call):
   "Instagram profile pages aren't supported. Paste an individual reel URL — e.g. instagram.com/reel/ABC123"

**Single reel transcription failure — before:**
1. Probe succeeds (shows thumbnail + title)
2. User clicks Transcribe
3. Queue row turns red with raw yt-dlp error string

**Single reel transcription failure — after:**
1. Probe succeeds (shows thumbnail + title)
2. User clicks Transcribe
3. Queue row turns red with:
   "Instagram requires login. Open Instagram in Safari and log in, then try again. Or place a cookies.txt file at ~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt"
4. User follows either path and retries successfully

---

## Acceptance criteria

1. Pasting `https://www.instagram.com/thelanguageblondie/reels` shows an amber guidance message within 500ms — no network call, no spinner.
2. The message clearly says to paste individual reel URLs with an example.
3. Pasting `https://www.instagram.com/reel/DTiglLVjLld` (single reel) still shows the oEmbed preview card (thumbnail + title). No regression.
4. When a single reel transcription fails (login required), the queue row error message is plain English and includes the cookies file path.
5. The cookies file path shown matches `~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt` exactly on macOS.
6. No yt-dlp stack traces or raw Python exception strings are ever shown in the UI.
7. `_is_instagram_profile_url` correctly identifies profile URLs and does NOT match `/reel/<id>`, `/p/<id>`, or `/tv/<id>` paths.

---

## Note for the coding agent

`_inject_ig_browser_cookies` in `transcribe_core.py` already handles the browser-cookies strategy for download — do not change it. This spec only adds:
1. A profile page detector (no network call — pure URL parsing)
2. A better error message at the point where the existing auth error is classified

The `_ig_cookies_file_path()` function already returns the correct platform-specific path — use it in the user-facing error message instead of hardcoding the macOS path.

Do not attempt to scrape Instagram profile pages or implement any login flow. The product position is: individual reels via oEmbed probe + browser cookies for download.
