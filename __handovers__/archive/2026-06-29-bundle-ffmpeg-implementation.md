# Handover — 2026-06-29 — claude-code

## Status
Done — `bundle-ffmpeg-sidecar` OpenSpec change implemented, verified end-to-end,
and shipped as GitHub release v0.1.1 + matching Notion entry. 22/25 tasks
checked off in `tasks.md`; remaining 3 are this close-out step (now also done).
Ready to archive the OpenSpec change.

## What changed this session
- `scripts/fetch_ffmpeg.py` (new) — downloads pinned ffmpeg/ffprobe 8.1.2 from
  evermeet.cx (exact versioned URLs, not the floating `getrelease` redirect),
  extracts, chmods, and verifies each binary actually runs (`-version`) before
  returning — raises loudly on any failure so a build can never silently ship
  without working ffmpeg.
- `scripts/build_sidecar.py` — now calls `fetch_ffmpeg_binaries()` after moving
  the PyInstaller `--onedir` output into place, so `npm run sidecar:build`
  bundles ffmpeg/ffprobe automatically with no extra manual step.
- `api/sidecar.py` — `check_ffmpeg()` replaced with `resolve_ffmpeg() -> str`:
  checks `<exe_dir>/ffmpeg` when frozen (PyInstaller build), falls back to
  `shutil.which("ffmpeg")` otherwise (dev mode, unchanged). All 3 call sites
  updated; the two that call `download_audio` now pass the resolved path.
- `api/transcribe_core.py` — `download_audio()` gained an `ffmpeg_location: Optional[str] = None`
  parameter, sets `ydl_opts["ffmpeg_location"]` when provided. Flask's call
  sites don't pass it — fully backward compatible, zero behavior change there.
- `api/__tests__/` (new) — first pytest harness in this repo (pytest installed
  into `.venv`, none existed before). 5 tests, all passing:
  `test_ffmpeg_resolution.py` (3), `test_ffmpeg_location.py` (2).
- `src-tauri/tauri.conf.json` — version bumped 0.1.0 → 0.1.1 (no other changes
  needed — the existing `bundle.resources` directory entry picked up the new
  binaries automatically, confirmed by inspecting the built `.app`).
- Built and published **GitHub release v0.1.1**
  (https://github.com/abhishek-mittal/transcribe-project/releases/tag/v0.1.1)
  with the new DMG (~159MB, up from ~100MB in v0.1.0). Added a matching row to
  the Notion "App Versions" tracker (under Transcribe APP page) with the
  download link.
- `_memory/rna-method/timeline.json` — new `recentDecisions[]` entry; 2 new
  `openQuestions[]` (Cargo.toml version still says 0.1.0 — cosmetic mismatch;
  evermeet.cx binaries are x86_64-via-Rosetta, not native arm64).

## How this was verified (not just unit tests)
Ran the **real frozen** `transcribe-sidecar` binary directly with `PATH`
stripped to `/usr/bin:/bin` (no Homebrew, no system ffmpeg anywhere):
1. Confirmed it got past `resolve_ffmpeg()` cleanly (no `FFMPEG_MISSING`) on a
   request to an intentionally-unreachable URL — proved the ffmpeg gate passes
   before ever touching the network.
2. Ran a full real download against a real public YouTube video
   (`jNQXAC9IVRw`) — yt-dlp downloaded the stream, the *bundled* ffmpeg's
   `FFmpegExtractAudio` postprocessor transcoded it to mp3, sidecar emitted
   `download-done`, and the output file was confirmed valid via `ffprobe`
   (128kbps MP3, proper ID3 tags). This is the strongest possible proof short
   of a literal clean VM — the bundled binary is what actually ran, not a
   PATH fallback that happened to also be present.
3. Confirmed dev-mode (unfrozen) resolution still correctly falls back to
   PATH (`/opt/homebrew/bin/ffmpeg` on this machine) — no regression for
   `tauri:dev`/`dev:all`.

## Next action
Run `/opsx:archive` (or equivalent) for `bundle-ffmpeg-sidecar` now that all
25 tasks are checked off and verified — syncs the `python-sidecar` delta spec
into `openspec/specs/`. Then move on to FIX-08 and FIX-09 (both root-caused
last session, ready to implement, see `__specs__/INDEX.md` Track 5).

## Do NOT
- Don't re-pick a different ffmpeg source without re-verifying end-to-end —
  evermeet.cx 8.1.2 was deliberately pinned (not "latest") and tested; a
  silent version bump would need the same `-version` + real-download
  verification redone, not just a URL swap.
- Don't assume the DMG size growth (~60MB) needs trimming — it was an
  explicit, accepted trade-off in `design.md`'s Risks section for a genuinely
  self-contained desktop app; don't "optimize" it away by going back to a
  PATH-dependent or first-run-download approach (both were explicitly
  considered and rejected this session).
- Don't touch `src-tauri/Cargo.toml`'s version field reflexively to "fix" the
  0.1.0/0.1.1 mismatch with `tauri.conf.json` unless you're already touching
  that file for another reason — flagged as a cosmetic open question, not
  urgent.

## Open questions / blockers
- FIX-10 (YouTube `/shorts` `int('')` crash) still needs a real traceback —
  unrelated to this session's work, carried over from the previous session.
- Cargo.toml/tauri.conf.json version mismatch (cosmetic).
- No native arm64 ffmpeg source found yet; x86_64-via-Rosetta works correctly
  but isn't the "purest" possible solution.

## Related
- OpenSpec change: `bundle-ffmpeg-sidecar` — 25/25 tasks complete, ready to archive
- __specs__ files: FIX-08, FIX-09 (ready to implement, untouched this session), FIX-10 (still blocked)
- _memory/rna-method/timeline.json updated: yes
