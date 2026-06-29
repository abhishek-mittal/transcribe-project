# FIX-10 — YouTube `/shorts` Tab: `int()` ValueError on Empty String (NEEDS REPRODUCTION)

## Priority
P0 if real — blocks transcription of any YouTube channel's `/shorts` tab. **Status:
unconfirmed.** This spec exists to record investigation done so far and prevent
re-deriving it from scratch, not as a ready-to-implement fix — do not implement a fix
based on guesswork; reproduce first.

## What was observed (UAT screenshot, not yet independently reproduced)

Pasting `https://www.youtube.com/@MillieAdrian/shorts` into the URL box and clicking
Transcribe produced, in the Queue view:
```
error_code: INTERNAL
message: invalid literal for int() with base 10: ''
```
with "+3 earlier" shown, implying 3 prior failed attempts on the same item before this
message was captured (retry history not visible in the screenshot).

## Investigation done so far (this session) — all dead ends, recorded so they aren't repeated

1. **Direct repro attempt**: ran the exact `probe_url()` code path
   (`api/sidecar.py:670-682`, `ydl_opts` with `extract_flat: in_playlist`,
   `playlistend: 5`, `socket_timeout: 8`) against
   `https://www.youtube.com/@MillieAdrian/shorts` in this repo's `.venv` — **completed
   successfully**, returned 5 entries, no exception.
2. **Checked every `int(...)` call site** in `api/sidecar.py` and
   `api/transcribe_core.py` (10 total). All except two are guarded with `or 0`
   (`int(x.get("duration") or 0)` — safe against both `None` and `""` since both are
   falsy). The two unguarded calls (`api/sidecar.py:795,797` — `int(v)` for
   `--page-start`/`--page-end` CLI args) are only reachable via
   `probe_url_page`/Rust's `probe_url_page` command (`src-tauri/src/lib.rs:806-825`),
   which always supplies both flags as `u32::to_string()` — never empty. The initial
   `/shorts` probe (`probe_url` in Rust, `src-tauri/src/lib.rs:778-799`) calls
   `probe_via_sidecar(&app, &url, &[])` with **no** extra args at all, so
   `parse_args()`'s `--page-start`/`--page-end` branches are never even reached for
   this flow — ruling out this code path as the source.
3. **Verified yt-dlp version parity**: dev `.venv` and the already-built PyInstaller
   sidecar binary (`src-tauri/binaries/transcribe-sidecar-aarch64-apple-darwin/`) are
   both `yt-dlp==2026.06.09` — ruled out a version-skew explanation.
4. **Checked `app.db` (`job_items` table) on this machine** for a matching
   `error_message` — no match. The UAT screenshot is from a different
   machine/session than this one; no local log or crash-log evidence captured the
   actual Python traceback. `~/Library/Application Support/com.shuhari.transcribe/
   crash-log.json` on this machine contains unrelated frontend (Svelte) errors from
   dev-mode sessions on 2026-06-27/28, not this bug.

## What's needed before this can be fixed (do this first, not a speculative patch)

Pick one:
- **Reproduce with full traceback**: paste the exact same URL
  (`https://www.youtube.com/@MillieAdrian/shorts`) into a running dev build
  (`npm run tauri:dev`) and capture the sidecar's stderr (visible in the Tauri dev
  console, or wherever `eprintln!`/Python `print(..., file=sys.stderr)` output
  surfaces) — the full Python traceback will name the exact file/line, unlike the
  bare exception string currently surfaced to the UI.
- **If it doesn't reproduce locally**: get the actual sidecar log / stderr capture
  from whoever ran the UAT session that produced this screenshot — ask what build
  (commit/binary) they were running, since "+3 earlier" implies this was retried
  multiple times, suggesting it's deterministic for this URL on their machine, not a
  one-off flake.
- **Once a traceback is available**: re-open this spec (or supersede it) with the
  actual file:line, apply the systematic-debugging skill's Phase 1 (root cause before
  fix) using that concrete evidence instead of this session's necessarily-incomplete
  investigation.

## Open question worth checking once reproduced
Could this be from the **transcribe phase** (after probing/picking a video from the
shorts list) rather than the probe phase itself — e.g. something in
`download_audio`/`transcribe_audio` (`api/transcribe_core.py`) parsing a duration or
byte-count field that's empty-string for Shorts specifically (which may have
different metadata shape than regular videos)? This session only checked the probe
path; the transcribe path for an individual Short selected from that list was not
checked, since the UAT screenshot's URL was pasted directly as a single
transcribe-box entry rather than picked from a `VideoPicker` list — worth confirming
which UI flow was actually used before assuming it's the channel-probe code path at
all.
