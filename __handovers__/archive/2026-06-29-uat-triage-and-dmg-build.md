# Handover — 2026-06-29 — claude-code

## Status
In progress — first beta DMG built and shared for UAT; 3 UAT-reported bugs
triaged, 2 root-caused with specs ready to implement (FIX-08, FIX-09), 1 blocked
on missing reproduction evidence (FIX-10). FFmpeg-bundling gap scoped as an
OpenSpec change but not implemented. No code changes landed yet this session —
this was a build + investigation + spec-writing session only.

## What changed this session
- Built `Transcribe_0.1.0_aarch64.dmg` via `npm run tauri:build` (unsigned,
  aarch64-only). DMG step initially failed with Apple Events error -1743
  (Automation permission); fixed by granting the VS Code helper process Finder
  automation access in System Settings — one-time per machine, already done here.
- `openspec/changes/bundle-ffmpeg-sidecar/` — full proposal/design/specs/tasks
  written (not implemented) for bundling the `ffmpeg` CLI binary into the sidecar
  so a clean machine doesn't need `brew install ffmpeg`. This was the first gap
  found, before UAT screenshots came in.
- UAT screenshots surfaced 3 more bugs. Root-caused 2 with direct reproduction:
  - `__specs__/FIX-08-instagram-reels-plural-oembed-400.md` — Instagram's oEmbed
    API only accepts singular `/reel/<id>/`, but Instagram's own UI now generates
    `/reels/<id>/` (plural) share links, which 400. Verified directly against
    Instagram's live API both ways.
  - `__specs__/FIX-09-safari-cookie-permission-during-download.md` — the existing
    Safari-cookie validation in `_inject_ig_browser_cookies` (which already
    documents and guards against TCC `PermissionError`) only validates once; yt-dlp
    re-reads the same cookie jar lazily inside `extract_info`, unguarded, and that
    second read is what's actually failing and surfacing as generic `INTERNAL`.
    Reproduced both the working validation and the failing re-read directly.
  - `__specs__/FIX-10-youtube-shorts-int-crash-NEEDS-REPRO.md` — could NOT
    reproduce. `invalid literal for int() with base 10: ''` from pasting a
    `/shorts` channel tab URL. Ran the exact `probe_url()` ydl_opts against the
    same URL — completed clean, no exception. Checked every `int()` call site in
    `api/sidecar.py`/`transcribe_core.py` — all but two are `or 0`-guarded, and
    the two unguarded ones (`--page-start`/`--page-end` parsing) are unreachable
    from the initial-probe code path that was used. No local log/db evidence
    matches this error. Left explicitly unimplemented rather than guessing.
- `__specs__/INDEX.md` — added Track 5 for FIX-08/FIX-09, a "blocked" section for
  FIX-10, and a pointer to the OpenSpec ffmpeg change.
- `_memory/rna-method/timeline.json` — new `recentDecisions[]` entry, new
  `openQuestions[]` entry for FIX-10.

## Next action
Implement FIX-08 and FIX-09 (both are P0, both have verified root causes and
exact target files/functions named in their specs) — test-first per `AGENTS.md`.
FIX-08 is the smaller of the two (one regex/normalize line in `_probe_ig_oembed`).

## Do NOT
- Don't attempt to fix FIX-10 without a real Python traceback. Three separate
  reproduction attempts this session all failed to trigger it — guessing a fourth
  fix would violate the systematic-debugging Iron Law and likely just mask a
  different bug. Get a live repro via `npm run tauri:dev` + the exact failing URL,
  or get sidecar stderr from whoever ran the original UAT session, first.
- Don't implement `bundle-ffmpeg-sidecar` by hand-patching `check_ffmpeg()` alone —
  the design doc explains why the fix must also thread the resolved path into
  yt-dlp's `ffmpeg_location` (today nothing sets that at all), otherwise the
  bundled binary would exist but silently go unused.
- Don't re-grant the Automation permission — it's already done on this machine.
  If the DMG build fails with error -1743 again on a *different* machine, that
  machine needs the same one-time System Settings step (see proposal omitted —
  just open Privacy & Security → Automation → enable the calling app for Finder).

## Open questions / blockers
- FIX-10 needs reproduction evidence (see Do NOT above) before it can be fixed.
- `bundle-ffmpeg-sidecar`'s design.md flags that `openspec/changes/tauri-desktop-app/`
  is still unarchived and explicitly deferred FFmpeg bundling to "v1.1" — check that
  change's `tasks.md` for unchecked items touching `api/sidecar.py` before starting,
  to avoid clobbering concurrent work.
- The DMG is unsigned/unnotarized — testers on other Macs need `xattr -cr` on the
  "can't be opened" (not "unidentified developer") variant of the Gatekeeper block,
  confirmed this session via direct user report.

## Related
- OpenSpec change: `bundle-ffmpeg-sidecar` (all 4 artifacts complete, 0 tasks done)
- __specs__ files: FIX-08, FIX-09 (ready to implement), FIX-10 (blocked)
- _memory/rna-method/timeline.json updated: yes
