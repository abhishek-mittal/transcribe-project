# FIX-08 — Instagram `/reels/<id>/` (plural) oEmbed 400

## Priority
P0 — confirmed in UAT. Instagram's own app/web UI generates `/reels/<id>/` (plural)
share links for content surfaced via the Reels tab; pasting one fails outright.

## Reality audit — root cause (reproduced, not guessed)

`_probe_ig_oembed()` in `api/sidecar.py:509` passes the user's URL through to
Instagram's oEmbed endpoint unmodified. Verified directly against Instagram's API:

```
GET /api/v1/oembed/?url=https://www.instagram.com/reels/DaIP5FpK8Sh/
→ HTTP 400: invalid param 'url'

GET /api/v1/oembed/?url=https://www.instagram.com/reel/DaIP5FpK8Sh/   (singular)
→ HTTP 200: {"title": "...", "author_name": "...", ...}
```

Instagram's oEmbed API only accepts the legacy singular `/reel/<id>/` path. The
plural `/reels/<id>/` form — which Instagram's UI now generates for links shared
from the Reels surface — 400s. `_probe_ig_oembed` catches this as a generic
`urllib.error.HTTPError` whose code (400) doesn't match the explicit `404`/`401`/`403`
branches, so it falls through to the catch-all `return {"type": "error", "code":
"NETWORK_ERROR", ...}` (line 547) — surfacing as a misleading `NETWORK_ERROR` /
"HTTP Error 400: Bad Request" in the UI, exactly as seen in UAT, even though the
network call succeeded and the post genuinely exists and is public.

`_is_instagram_profile_url()` (`api/transcribe_core.py:200`) already correctly
classifies `/reels/<id>/` as an individual post, not a profile listing — that
function's `segments[:-1]` post-keyword check matches "reels" as a post keyword and
returns `False` (not a profile page) — so the URL correctly reaches
`_probe_ig_oembed`. The bug is entirely inside oEmbed's own URL-format requirement,
not in our classification logic.

## After state

| URL form | Before | After |
|---|---|---|
| `instagram.com/reel/<id>/` (singular) | ✓ works | Unchanged |
| `instagram.com/reels/<id>/` (plural) | `NETWORK_ERROR` / "HTTP Error 400: Bad Request" | ✓ works — normalized to singular before the oEmbed call |
| `instagram.com/<user>/reel/<id>/` | ✓ works (already handled) | Unchanged |
| `instagram.com/<user>/reels/<id>/` (if it exists) | Likely same 400 | ✓ works (same fix covers it) |

## Target files

- `api/sidecar.py` — `_probe_ig_oembed()` (line ~509)

## Exact change

Before building `oembed_url`, normalize any `/reels/` path segment to `/reel/`
(singular) in the incoming `url`. A simple, scoped string/regex replace on the path
segment is sufficient — do not rewrite query strings or other parts of the URL.

```python
# Instagram's oEmbed API only accepts the legacy singular /reel/<id>/ path;
# the UI now also generates /reels/<id>/ (plural) share links that 400 if
# passed through unmodified. Normalize before calling oEmbed.
normalized_url = re.sub(r"(?<=instagram\.com/)(?:[^/]+/)?reels/", lambda m: m.group(0).replace("reels/", "reel/"), url)
```

(Exact regex/approach left to implementer — the requirement is: any `/reels/<id>/`
path segment, with or without a leading username segment, must become `/reel/<id>/`
before the oEmbed request, while the *returned* result's `"url"` field should still
reflect the original user-supplied URL, not the normalized one, so downstream
download/transcribe steps aren't affected by this probe-only normalization.)

Also worth fixing while in this function: the catch-all `NETWORK_ERROR` branch
(line 547) doesn't distinguish "we successfully reached Instagram and it told us our
request was malformed" (genuine 4xx client errors after normalization, which
shouldn't happen often now) from actual network failures (DNS, timeout, connection
reset). Not required for this fix, but if touched, classify non-2xx HTTP responses
with a clearer code/message than the current generic `NETWORK_ERROR`.

## Verification steps

```bash
source .venv/bin/activate
python3 -c "
import sys; sys.path.insert(0, '.')
from api.sidecar import _probe_ig_oembed
result = _probe_ig_oembed('https://www.instagram.com/reels/DaIP5FpK8Sh/')
print(result)
assert result['type'] == 'video', result
"
```

Confirm the same call with the original singular `/reel/<id>/` form still returns
`type: video` unchanged (no regression).

## Acceptance criteria
- Pasting `instagram.com/reels/<id>/` into the URL box returns a valid video probe
  (title, thumbnail, uploader) instead of `NETWORK_ERROR` / "HTTP Error 400: Bad
  Request".
- Existing singular `/reel/<id>/` and `/<user>/reel/<id>/` forms continue to work
  unchanged (no regression — verify against FIX-05's existing test cases).
