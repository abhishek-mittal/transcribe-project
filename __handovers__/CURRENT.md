# Handover — 2026-06-29 — claude-code

## Status
Done — FIX-10 (the `int('')` crash, previously blocked on missing reproduction
evidence) is root-caused and fixed. Pushed to `main` (`37b703d`). DMG asset on
the existing v0.1.1 GitHub release was overwritten in place (no new tag cut,
per explicit instruction). Notion entry updated to match. **Not yet confirmed
against the real affected M2 hardware** — verified by simulating the exact
broken input, not by the user re-testing the new build.

## What changed this session
- Got the real evidence first. User reported v0.1.1 still crashing with
  `invalid literal for int() with base 10: ''` on an M2 Mac — but now on
  **every URL type**, not just `/shorts` like the original UAT screenshot.
  That single fact (every URL, not one shape) was the key signal something
  systemic was wrong, not a yt-dlp parsing edge case for one channel-tab URL.
- First log-path guess was wrong: told the user to check
  `~/Library/Application Support/com.shuhari.transcribe/sidecar.log` — that
  file never gets created there. Correct path (per `src-tauri/src/lib.rs`,
  `app_log_dir()` not `app_data_dir()`) is
  `~/Library/Logs/com.shuhari.transcribe/sidecar.log` — but that was also
  empty on their machine (the log-file open likely failed silently, `.ok()`
  swallows the error). Live capture was the only path that worked: had them
  run `/Applications/Transcribe.app/Contents/MacOS/transcribe` directly from
  Terminal so stderr prints live, then use the app normally.
- **Real root cause** (from the actual traceback): `platform.mac_ver()[0]`
  returns a malformed version string on some macOS versions (an empty/blank
  dot-separated segment). yt-dlp's plugin discovery
  (`yt_dlp/update.py:_get_variant_and_executable_path`, called unconditionally
  the first time *any* `YoutubeDL()` is constructed — explaining why it hit
  every URL) calls `version_tuple()` on that string WITHOUT yt-dlp's own
  `lenient=True` option, so `int('')` raises. Reproduced by simulating the
  exact broken `mac_ver()` output locally and confirming the same crash, then
  confirming the fix prevents it.
- `api/sidecar.py` — patches `platform.mac_ver()` before `import yt_dlp`,
  normalizing any malformed release string to `"0.0.0"` if it has an
  empty/blank segment, passing through valid strings unchanged.
- `api/__tests__/test_mac_ver_workaround.py` (new) — 2 tests, both passing.
  Full suite now 7/7.
- Rebuilt sidecar + DMG, verified integrity, **overwrote the existing
  v0.1.1 GitHub release asset in place** (`gh release delete-asset` +
  `gh release upload`, not a new tag) and updated its release notes. Updated
  the matching Notion "App Versions" row's Fixed Bugs / Dev Notes.
- `_memory/rna-method/timeline.json` updated; FIX-10's old "needs repro"
  open question removed, replaced with "waiting on M2 user to confirm fix."

## Next action
**Tell the M2 user to redownload the v0.1.1 DMG from the same link** (the
asset was overwritten, same URL:
https://github.com/abhishek-mittal/transcribe-project/releases/download/v0.1.1/Transcribe_0.1.1_aarch64.dmg)
and re-test. If their browser/Finder cached the old download, they may need
to force a fresh download (not just re-open a previously-downloaded copy).
Once confirmed, FIX-10 can be marked fully closed — until then, treat it as
"fixed but unconfirmed on the affected hardware."

## Do NOT
- Don't tell anyone the sidecar log is at `~/Library/Application Support/...`
  — that was this session's own wrong guess, corrected to
  `~/Library/Logs/com.shuhari.transcribe/sidecar.log` (per `app_log_dir()` in
  `src-tauri/src/lib.rs`). Even that path can come up empty if the log file
  failed to open — the Terminal-direct-launch method
  (`/Applications/Transcribe.app/Contents/MacOS/transcribe`) is the reliable
  fallback, not a last resort.
- Don't assume this is the only macOS-version-sensitive landmine in yt-dlp.
  The same class of bug (yt-dlp code that doesn't defensively handle OS
  quirks) could exist elsewhere — if another "crashes on some machines but
  not others" report comes in, check whether it's similarly OS-version- or
  environment-sensitive before assuming it's URL-specific.
- Don't re-cut a new version tag for DMG fixes unless asked — this session
  overwrote the v0.1.1 asset in place per explicit instruction, diverging
  from the v0.1.0→v0.1.1 pattern (new tag) used for the ffmpeg fix. Ask which
  approach is wanted each time rather than assuming.

## Open questions / blockers
- Unconfirmed on real affected hardware — see Next action.
- `src-tauri/Cargo.toml` version still says 0.1.0 vs `tauri.conf.json`'s
  0.1.1 (cosmetic, carried over from last session).
- FIX-08 (Instagram `/reels/` plural) and FIX-09 (Safari cookie permission)
  remain open, root-caused last session, not yet implemented.

## Related
- OpenSpec change: none this session (FIX-10 was a `__specs__`-grain fix, not OpenSpec)
- __specs__ file: `FIX-10-youtube-shorts-int-crash-NEEDS-REPRO.md` — title is now
  stale (it's fixed, not blocked) but not renamed this session; consider
  renaming to drop `-NEEDS-REPRO` and updating its content to reflect the
  actual fix next time it's touched.
- _memory/rna-method/timeline.json updated: yes
