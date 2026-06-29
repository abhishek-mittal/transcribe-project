# Transcribe — Feature Specs Index

All specs live in `projects/transcribe-project/__specs__/`.
Coding agents should read the relevant spec file before starting any work.
Architecture reference: `projects/transcribe-project/transcribe-handoff.md`

---

## Open — work these in order

**Track 1: URL probe fixes (do these first — everything downstream depends on them)**

| Order | Spec | Title | Priority | Depends on |
|---|---|---|---|---|
| 1 | [FIX-04](FIX-04-probe-generator-crash.md) | Probe: YouTube Channel/Search Generator Crash | P0 | — |
| 2 | [FIX-05](FIX-05-instagram-error-handling.md) | Instagram: Profile Page Detection + Download Error Clarity | P0 | — |
| 3 | [FIX-06](FIX-06-thumbnail-fallback.md) | Thumbnail Fallback for Flat-Extract Playlist Entries | P1 | FIX-04 |

**Track 2: Views (can start in parallel with Track 1 fixes)**

| Order | Spec | Title | Priority | Depends on |
|---|---|---|---|---|
| 4 | [FIX-03](FIX-03-window-decorations.md) | Restore Native OS Window Chrome | P0 | — |
| 5 | [F05](F05-history-view.md) | History View (per-transcript) | P1 | — |
| 6 | [F06](F06-settings-model-selection.md) | Settings View + Model Selection | P1 | — |

**Track 3: Batch flow (needs Track 1 fixes done first)**

| Order | Spec | Title | Priority | Depends on |
|---|---|---|---|---|
| 7 | [F07](F07-url-detection-video-preview.md) | URL Detection + Single Video Preview | P0 | FIX-04, FIX-05 |
| 8 | [F08](F08-video-picker.md) | Video Picker (Playlist / Page) | P1 | F07, FIX-06 |
| 9 | [F09](F09-queue-view.md) | Queue View (Live Batch Progress) | P1 | F07, F08 |
| 10 | [F10](F10-job-history.md) | Job History View | P1 | F09 |

**Track 4: UI polish (do last — applies on top of all working features)**

| Order | Spec | Title | Priority | Depends on |
|---|---|---|---|---|
| 11 | [F11](F11-ui-improvements.md) | UI Improvements: Probe UX, Picker, Queue Row Polish | P1 | FIX-04, FIX-05, FIX-06, F07, F08, F09 |
| 12 | [F12](F12-picker-queue-ux.md) | Picker Load More, Transcribed State, Queue Row Fix, Active Row | P1 | FIX-04, FIX-06, F11 |
| 13 | [F13](F13-pipeline-queue-architecture.md) | Pipeline Queue: Chunked Downloads + Sequential Transcription + Resume | P1 | F12 |
| 14 | [F14](F14-progressive-picker-cache.md) | Progressive Picker + Probe Cache + Visible Activity | P1 | F08, F12 |
| 15 | [FIX-07](FIX-07-instagram-reels-page-probe.md) | Instagram Reels Page Probe (Profile → Picker) | P1 | FIX-05, F12 |

**Track 5: UAT-found bugs (2026-06-29) — root-caused and reproduced, ready to implement**

| Order | Spec | Title | Priority | Depends on |
|---|---|---|---|---|
| 16 | [FIX-08](FIX-08-instagram-reels-plural-oembed-400.md) | Instagram `/reels/<id>/` (plural) oEmbed 400 | P0 | — |
| 17 | [FIX-09](FIX-09-safari-cookie-permission-during-download.md) | Safari Cookie PermissionError Surfaces as Generic INTERNAL | P0 | — |

**Blocked — needs reproduction before implementation, do not guess a fix**

| Spec | Title | Status |
|---|---|---|
| [FIX-10](FIX-10-youtube-shorts-int-crash-NEEDS-REPRO.md) | YouTube `/shorts` tab `int('')` ValueError | Unconfirmed — investigation dead-ends recorded, needs real traceback from a repro or the original UAT session's logs |

**Resolved — see archived OpenSpec change**

FFmpeg bundling (previously required `brew install ffmpeg` on a clean machine) was
implemented and shipped in v0.1.1 — see
`openspec/changes/archive/2026-06-29-bundle-ffmpeg-sidecar/` for the full
proposal/design/specs/tasks. `ffmpeg`/`ffprobe` now ship inside the app bundle;
`api/sidecar.py`'s `resolve_ffmpeg()` resolves the bundled binary when frozen, falls
back to PATH in dev mode. Verified end-to-end with PATH stripped of all ffmpeg.

---

## URL support matrix — what works after all fixes are applied

| URL example | Probe result | Transcription |
|---|---|---|
| `youtube.com/watch?v=...` | Single video preview | ✓ Works |
| `youtube.com/shorts/...` | Single video preview | ✓ Works |
| `youtube.com/@Channel/shorts` | Playlist (20 entries, load more up to total) | ✓ Works per-video |
| `youtube.com/@Channel/videos` | Playlist (20 entries, load more up to total) | ✓ Works per-video |
| `youtube.com/playlist?list=...` | Playlist (20 entries, load more up to total) | ✓ Works per-video |
| `youtube.com/results?search_query=...` | Search results (20 entries) | ✓ Works per-video |
| `instagram.com/reel/<id>` | Single video preview (oEmbed) | Requires Safari login or cookies file |
| `instagram.com/<user>/reels` | Reels picker (12 at a time, load more) — requires cookies | ✓ Works per-reel (with cookies) |
| `instagram.com/<user>/` (profile) | Same as /reels — routed to reels probe | ✓ Works per-reel (with cookies) |

---

## Already shipped — do not re-implement

| Spec | What is already done | Verified by |
|---|---|---|
| FIX-01 | Fonts bundled locally. `src/lib/fonts/` has all woff2 files. `@font-face` declarations in `+page.svelte`. No Google Fonts link tags anywhere. | `ls src/lib/fonts/` |
| FIX-02 | `TranscriptPanel` accepts `streamSegments`, `phase`, `timestamps` props and renders live streaming segments. `+page.svelte` passes all three. Desktop live streaming works. | `head -20 src/lib/desktop/TranscriptPanel.svelte` |
| F04 | Rust layer has `save_transcript`, `load_history`, `delete_transcript`, `clear_history` commands. `+page.svelte` calls `save_transcript` fire-and-forget on every result event. History JSON stored at `~/Library/Application Support/com.shuhari.transcribe/history.json`. | `src-tauri/src/lib.rs` lines 104–285 |

---

## Sidecar rebuild required

Any change to `api/sidecar.py` or `api/transcribe_core.py` requires rebuilding the PyInstaller binary before testing in the Tauri app:

```
cd api && pyinstaller transcribe-sidecar.spec --noconfirm
```

Then hot-reload the Tauri dev server (`tauri dev`) to pick up the new binary.

---

## What each spec contains

Each spec follows this structure:
- **Current state** — what exists today, what is broken, verified test output
- **After state** — what changes, shown as before/after table
- **Target component(s)** — exactly which file(s) the agent must open first
- **Exact changes** — the specific lines or functions to modify (not code to copy-paste, but precise location + what changes)
- **Before/after user flow** — observable sequence of actions
- **Verification steps** — command to run to confirm the fix works before marking done
- **Acceptance criteria** — user-observable outcomes
