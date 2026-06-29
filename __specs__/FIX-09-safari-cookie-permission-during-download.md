# FIX-09 — Safari Cookie TCC PermissionError Surfaces as Generic INTERNAL During Download

## Priority
P0 — confirmed on-disk (`app.db` job_items), root-caused and reproduced locally.
Breaks Instagram downloads for any user whose only signed-in browser is Safari and
who hasn't placed a cookies file.

## Reality audit — root cause (reproduced, not guessed)

`_inject_ig_browser_cookies()` in `api/transcribe_core.py:252` already has a documented
defense for exactly this failure mode: its own docstring (lines 264-269) explains that
reading Safari's cookie jar directly (to validate it before use) can raise
`PermissionError` because macOS sandboxes Safari's `Cookies.binarycookies` behind a TCC
permission a bundled PyInstaller binary doesn't have, and that this validation call
(`extract_cookies_from_browser(browser)` at line 296, inside a `try/except Exception`)
exists specifically to catch that and fall through to the next browser candidate.

**That validation call works correctly.** Reproduced directly:
```python
from yt_dlp.cookies import extract_cookies_from_browser
extract_cookies_from_browser('safari')
# → PermissionError: [Errno 1] Operation not permitted: '.../Safari/Data/Library/Cookies/Cookies.binarycookies'
# caught by the try/except at transcribe_core.py:297, correctly skipped
```

**The actual bug is one layer deeper.** Once `_inject_ig_browser_cookies` decides
Safari's jar is usable and sets `ydl_opts["cookiesfrombrowser"] = ("safari",)`,
yt-dlp itself re-reads that same cookie jar a second time, *lazily*, the first time
`YoutubeDL.extract_info()` actually needs the cookies — and that second read is
**not** wrapped in any try/except on our side. Reproduced directly:
```python
import yt_dlp
ydl_opts = {'cookiesfrombrowser': ('safari',), 'skip_download': True}
with yt_dlp.YoutubeDL(ydl_opts) as ydl:
    ydl.extract_info('https://www.instagram.com/reel/<id>/', download=False)
# → DownloadError: ERROR: [Errno 1] Operation not permitted: '.../Cookies.binarycookies'
```

This is the exact error string recorded in `app.db.job_items.error_message` for two
real failed jobs on this machine (`error_code = 'INTERNAL'`), confirming this is not
hypothetical — it's the real failure path. `classify_ydl_error()` (`api/sidecar.py:263`)
has no pattern matching "Operation not permitted" / `PermissionError`, so it falls
through to the generic `INTERNAL` code with the raw yt-dlp string as the message —
confusing and gives the user no path to resolution.

**Why the existing validation doesn't prevent this:** validating that a cookie jar
*can* be read at one point in time doesn't guarantee yt-dlp's own internal re-read
(via a different code path/library internals, possibly without the same permission
context) will succeed too. The two reads are independent.

## After state

| Scenario | Before | After |
|---|---|---|
| User has cookies file at app-data path | ✓ works (cookiefile takes priority, never reaches browser fallback) | Unchanged |
| User signed into Chrome/Firefox only | ✓ works (Safari fails validation, falls through correctly) | Unchanged |
| User signed into Safari only, no cookies file | Validation passes (Safari jar readable at validation time), but `extract_info` raises `PermissionError` mid-download → generic `INTERNAL` error, raw Python traceback string shown to user | Either (a) caught and falls through to next browser/no-auth path with a clear message, or (b) if no fallback succeeds, a specific `code` (e.g. `INSTAGRAM_COOKIES_PERMISSION_DENIED`) with an actionable message ("Safari cookies aren't accessible to this app — drop a cookies file instead, see Settings") instead of a raw `PermissionError` string |

## Target files

- `api/transcribe_core.py` — `_inject_ig_browser_cookies()` (the validation read is fine; the bug is that its result doesn't predict the later real read's outcome)
- `api/sidecar.py` — `classify_ydl_error()` (add a pattern for permission-denied cookie errors), and whichever call site wraps the `extract_info`/`download_audio` call that can raise this

## Exact change

1. In `classify_ydl_error()` (`api/sidecar.py:263`), add a new pattern/branch
   recognizing `PermissionError`-shaped messages (`"operation not permitted"`,
   `"permission denied"`) combined with `"cookie"` or `"binarycookies"` in the
   message, mapped to a new code, e.g. `INSTAGRAM_COOKIES_PERMISSION_DENIED` (or
   reuse `INSTAGRAM_LOGIN_REQUIRED` with a distinguishing message — implementer's
   call, but it must not remain generic `INTERNAL`).
2. Decide whether to also make this self-healing: since
   `_inject_ig_browser_cookies` already has a fallback chain (file → Safari → Chrome
   → Firefox → none), consider whether the *download* call site should retry with
   `cookiesfrombrowser` unset/next-candidate if the first attempt fails with this
   specific error — this would let a Safari-only user with Chrome also installed
   (but not realized as a candidate because Safari "validated" first) actually
   succeed. At minimum, the error message must be clear even if no retry is added.
3. Update the user-facing message for the new/adjusted code to mention the
   concrete fix: "drop a cookies file at `~/Library/Application Support/
   com.shuhari.transcribe/instagram_cookies.txt`" (matching the existing
   `_ig_cookies_file_path()` convention already documented elsewhere in this file).

## Verification steps

```bash
source .venv/bin/activate
python3 -c "
import sys; sys.path.insert(0, '.')
from api.sidecar import classify_ydl_error
exc = Exception(\"ERROR: [Errno 1] Operation not permitted: '/Users/x/Library/Containers/com.apple.Safari/Data/Library/Cookies/Cookies.binarycookies'\")
code = classify_ydl_error(exc)
assert code != 'INTERNAL', f'still falling through to INTERNAL: {code}'
print('classified as:', code)
"
```

End-to-end: on a machine signed into Safari (Instagram) with no cookies file and no
other browser logged in, attempt a real Instagram reel transcription and confirm the
UI shows the new, specific, actionable error instead of the raw `PermissionError`
string.

## Acceptance criteria
- The exact error string seen in UAT/`app.db` no longer surfaces as generic
  `INTERNAL` with a raw Python exception message.
- User gets a specific, actionable message naming the cookies-file workaround.
- No regression to the cookies-file-present or Chrome/Firefox-signed-in paths.
