# FIX-10 — `platform.mac_ver()` Crash on Every Probe/Download (`int('')` ValueError)

## Priority
P0 — fixed and shipped. Originally reported via a UAT screenshot on
`youtube.com/@MillieAdrian/shorts`, initially suspected URL-specific. A
second, more informative report (every URL type, not just `/shorts`) plus a
real captured traceback established the actual root cause below.

## Reality audit — root cause (confirmed via real traceback, not guessed)

Got the evidence by having the affected user run the app directly from
Terminal instead of double-clicking, so stderr prints live instead of being
swallowed:

```bash
/Applications/Transcribe.app/Contents/MacOS/transcribe
```

(The file-based log at `~/Library/Logs/com.shuhari.transcribe/sidecar.log` —
per `app_log_dir()` in `src-tauri/src/lib.rs` — was empty on the affected
machine; the log-file open likely failed silently since the code only does
`.ok()` on it. Live Terminal capture was the only path that worked.)

Real traceback:
```
File "sidecar.py", line 416, in run_download
File "api/transcribe_core.py", line 512, in download_audio
File "yt_dlp/YoutubeDL.py", line 657, in __init__
    load_all_plugins()
File "yt_dlp/plugins.py", ... -> default_plugin_paths() -> get_executable_path()
File "yt_dlp/update.py", line 88, in _get_variant_and_executable_path
    machine = '_legacy' if version_tuple(platform.mac_ver()[0]) < (10, 15) else ''
File "yt_dlp/utils/_utils.py", line 2911, in version_tuple
    return tuple(parse(e) for e in re.split(r'[-.]', v))
ValueError: invalid literal for int() with base 10: ''
```

`platform.mac_ver()[0]` returns a malformed release string on some macOS
versions (confirmed: an empty/blank dot-separated segment — e.g. equivalent
to `''`, `'14.5.'`, or `'.14.5'`). yt-dlp's plugin discovery calls
`version_tuple()` on this string **without** yt-dlp's own `lenient=True`
option (which exists specifically to swap `int` for `int_or_none` and avoid
this exact failure mode) — so a malformed string raises uncaught.

This code runs **unconditionally the first time any `YoutubeDL()` instance is
constructed in a process** — which is why it hit every URL type, not just
`/shorts`: the `/shorts` probe path and `run_download`'s path both
independently construct their own first `YoutubeDL()`, and both crash here
identically.

Reproduced directly: simulating the exact broken `platform.mac_ver()` output
(`('', ('', '', ''), 'arm64')`) and constructing a real `YoutubeDL()` crashes
with the identical error; with the fix applied, it constructs successfully.

## After state

| Scenario | Before | After |
|---|---|---|
| Affected macOS version, any URL, probe | `INTERNAL: invalid literal for int() with base 10: ''` | Works normally |
| Affected macOS version, any URL, Transcribe click | Same crash | Works normally |
| Unaffected macOS version (valid `mac_ver()` output) | Worked already | Unchanged — patch passes through valid strings as-is |

## Target files

- `api/sidecar.py` — patches `platform.mac_ver()` at module top, before `import yt_dlp`

## Exact change

```python
_real_mac_ver = platform.mac_ver

def _safe_mac_ver():
    release, versioninfo, machine = _real_mac_ver()
    if not release or any(part == "" for part in release.split(".")):
        release = "0.0.0"
    return release, versioninfo, machine

platform.mac_ver = _safe_mac_ver
```

Placed before `import yt_dlp` (and before `transcribe_core`'s import, which
also imports `yt_dlp`) so the patch is in effect regardless of which module
triggers the first real `import yt_dlp` execution. The actual crash site
(`YoutubeDL.__init__`) is lazy, not import-time, so this is conservatively
early rather than strictly required to be — defensive against any future
code path that constructs a `YoutubeDL()` earlier than expected.

## Verification steps

```bash
source .venv/bin/activate
python3 -m pytest api/__tests__/test_mac_ver_workaround.py -v
```

Direct reproduction of the original crash + fix confirmation:
```bash
python3 -c "
import platform
platform.mac_ver = lambda: ('', ('', '', ''), 'arm64')
import api.sidecar  # re-patches on top, fixing it
import yt_dlp
yt_dlp.YoutubeDL({'quiet': True})  # must not raise
print('OK')
"
```

## Acceptance criteria
- `YoutubeDL()` construction never raises due to `platform.mac_ver()`,
  regardless of what the OS reports.
- No change in behavior for machines where `platform.mac_ver()` already
  returns a well-formed string.

## Status: fixed, shipped, awaiting field confirmation
Fix verified via direct reproduction of the exact broken input. DMG asset on
the existing `v0.1.1` GitHub release was rebuilt and overwritten in place
with this fix (same download URL). **Not yet confirmed by the originally
affected M2 user re-testing the new build** — flag this spec as fully closed
once they confirm.

---

## Appendix — earlier (incomplete) investigation, kept for history

Before the real traceback was captured, the bug was suspected to be
`/shorts`-specific based on the original UAT screenshot. That investigation
is recorded here since it ruled out several real possibilities and is worth
knowing wasn't wasted effort, even though the eventual root cause was
elsewhere entirely:

1. Ran the exact `probe_url()` code path against the same `/shorts` URL in
   dev `.venv` — completed successfully, no exception. (Correct — the dev
   venv's `platform.mac_ver()` returns a well-formed string on this
   machine; the bug is OS-version-dependent, not URL-dependent.)
2. Checked every `int(...)` call site in `api/sidecar.py` and
   `api/transcribe_core.py` — all the application-level ones were already
   safely guarded. (Correct — the actual crash is inside yt-dlp's own
   library code, not anything in this codebase's direct call sites.)
3. Verified yt-dlp version parity between dev venv and the built
   PyInstaller binary — ruled out version skew. (Not the cause, but a
   reasonable thing to rule out.)
4. Checked local `app.db`/crash-log for a matching error — found nothing,
   correctly concluding the UAT screenshot was from a different
   machine/session with no local evidence available at the time.

The key fact that eventually cracked this — confirmed once a second user
report came in — was that the crash happened on **every URL type**, not
just `/shorts`. That ruled out a URL-parsing theory entirely and pointed at
something both code paths share: constructing a `YoutubeDL()` instance.
