# .learnings/ — Index

Reference codebases kept locally for agents to mine for implementation patterns and
error-handling knowledge — **read for ideas, never copy wholesale, never edit in place.**
Licenses differ per project (see each project's own LICENSE) — treat anything copied
near-verbatim as subject to that project's license, not this repo's.

This file is read by Claude Code, GitHub Copilot, and OpenCode at session start (wired
via `AGENTS.md`) whenever a task touches yt-dlp invocation, sidecar/binary management,
bot-detection workarounds, local SQLite persistence, or download/job queueing — the
areas where transcribe-project has known open problems (see `__specs__/INDEX.md`).

---

## neodlp-main

**What it is:** [NeoDLP](https://github.com/neosubhamoy/neodlp) — a mature, shipped,
cross-platform (Windows/Linux/macOS) Tauri 2 + Rust + React/TypeScript desktop app for
downloading video/audio via yt-dlp from 2.5k+ sites (YouTube, Instagram, Facebook, X,
etc.). Closest real-world analog available to transcribe-project: same Tauri-desktop +
yt-dlp shape, same bot-detection problems, further along in solving them. MIT licensed.

**Where:** `.learnings/neodlp-main/`

**Key structural difference to keep in mind:** neodlp spawns yt-dlp as a **CLI
subprocess** via `Command.sidecar()` (TypeScript side, `src/helpers/use-downloader.ts`)
and parses stdout. transcribe-project uses yt-dlp as a **Python library**
(`yt_dlp.YoutubeDL(ydl_opts)` in `api/transcribe_core.py` / `api/sidecar.py`). Don't
port CLI flag strings directly — translate them to the equivalent `ydl_opts` dict key
(e.g. neodlp's `--extractor-args youtubepot-bgutilhttp:base_url=...` is the CLI form of
the same bgutil plugin transcribe-project already drives via
`ydl_opts["extractor_args"]["youtube"]["fetch_pot"] = ["always"]` — see
`api/transcribe_core.py:390`). The *concepts* (retry chains, progress parsing, error
classification) port; the *syntax* doesn't.

### 1. YouTube PO-token / bot-detection (directly relevant — we have this problem now)

- `api/transcribe_core.py:374-394` already does player-client fallback
  (`android,ios,tv`) + bgutil PO-token plugin via `fetch_pot: ["always"]`. This is the
  same underlying mechanism neodlp uses, confirming our approach is the same family of
  fix as a project that ships this successfully across 3 OSes.
- neodlp runs bgutil as a **separate long-running HTTP sidecar** (`neodlp-pot` binary,
  `src/helpers/use-pot-server.ts`) that yt-dlp talks to via
  `--extractor-args youtubepot-bgutilhttp:base_url=http://localhost:{port}`, rather than
  the in-process plugin form. Tradeoff: an HTTP server needs lifecycle management
  (spawn, health-check via a `/ping` endpoint, kill on app exit) but is shareable across
  concurrent yt-dlp invocations without re-deriving a token each time. Worth considering
  if our VPS-deployed Flask path (`api/sidecar.py`) ever needs to serve many concurrent
  YouTube requests — one bgutil HTTP server instead of one plugin-spawned process per
  request.
- Error taxonomy worth copying conceptually: neodlp's Python provider
  (`src-tauri/resources/plugins/yt-dlp-plugins/bgutil-ytdlp-pot-provider/yt_dlp_plugins/extractor/getpot_bgutil_http.py`)
  distinguishes `TransportError` (server unreachable) vs `HTTPError` (bad status) vs
  `JSONDecodeError` (malformed response) vs missing-`poToken`-in-response, each a
  distinct, named failure rather than one generic catch. Our own
  `_INSTAGRAM_AUTH_PATTERNS` classification in `api/sidecar.py` already does this kind
  of thing for Instagram — the same rigor could apply to YouTube PO-token failures,
  which currently likely surface as a generic yt-dlp `DownloadError`.
- `disable_innertube=1` is passed conditionally — only when the standard token request
  fails to extract a BotGuard challenge from the webpage. This "try once, then escalate
  to the bigger hammer" sequencing (rather than always paying the innertube-disabled
  cost) is a pattern worth matching if our retry chain currently tries all
  player-clients unconditionally rather than narrowing based on what failed.

### 2. Instagram bot-detection (directly relevant — see `__specs__/FIX-05`)

neodlp doesn't have an Instagram-specific PO-token-equivalent (Instagram's blocking is
different from YouTube's), but the general shape — "if this code path resolves to
'requires login', say so explicitly and tell the user what file to drop where" — is the
exact gap FIX-05 identifies in our own `sidecar.py`/`transcribe_core.py`: we classify
the error correctly (`INSTAGRAM_LOGIN_REQUIRED`) but surface the raw yt-dlp string
instead of an actionable message. neodlp's pattern of mapping every known failure mode
to a structured `{code, message, action}`-shaped object before it ever reaches the UI
(rather than passing the exception through) is the concrete model to follow when
implementing FIX-05.

### 3. yt-dlp invocation patterns (concepts port, syntax doesn't)

- Progress reporting: neodlp uses `--progress-template` with a custom
  `status:...,progress:...,speed:...` string parsed line-by-line from stdout
  (`use-downloader.ts:335-336`). transcribe-project's Python-library approach gets this
  for free via `ydl_opts["progress_hooks"]` (already used in
  `api/transcribe_core.py:462`) — no porting needed, just confirms the hook-based
  approach is the right one for the library style.
- Final-file-path capture: neodlp uses a yt-dlp post-exec hook,
  `--exec after_move:echo Finalpath: {}`, then greps stdout for the `Finalpath: ` prefix
  (`use-downloader.ts:341-342, 681-682`). The library equivalent is reading
  `info['requested_downloads'][0]['filepath']` from the dict `extract_info` returns —
  worth double-checking our code uses the dict return value rather than re-deriving the
  path some other way.
- Retry/backoff: `--retries`, `--sleep-requests` / `--sleep-interval` /
  `--max-sleep-interval`, scaled by playlist size (small playlists sleep 5-10s between
  items, 500+-item playlists sleep 40-60s) — `use-downloader.ts:374-410`. If our own
  playlist/batch probing (`FIX-04`'s generator-crash territory) starts hitting rate
  limits at scale, this auto-scaling-by-size heuristic is a concrete number to start
  from rather than guessing.
- Cookie handling: `--cookies-from-browser {browser}` vs `--cookies {filepath}` as two
  distinct paths (`use-downloader.ts:530-536`) — matches our own
  `_inject_ig_browser_cookies` (browser-sourced) vs `IG_DLP_COOKIES_FILE` /
  `YT_DLP_COOKIES_FILE` (file-sourced) split in `transcribe_core.py`. Confirms the
  two-path design is standard, not something to consolidate.

### 4. Playlist / generator handling (directly relevant — see `__specs__/FIX-04`)

FIX-04's root cause — `process=False` returning a lazy generator for `entries`, crashing
on `len()`/iteration — is a yt-dlp-library-specific footgun that a CLI-based tool like
neodlp never encounters (the CLI always fully resolves before printing). This is one
place neodlp's source has *nothing* to teach us, because the bug class doesn't exist on
their side of the fence — worth recording so a future agent doesn't waste time
searching neodlp's source for an analogous fix that isn't there.

### 5. Local persistence (SQLite) — comparable to our history/queue storage

- `src-tauri/src/migrations.rs`: versioned migrations (v1 → v2 → v3), each migration a
  forward-only SQL diff; v2 added columns via the shadow-table pattern (create new
  table, copy data, drop old, rename) rather than `ALTER TABLE` everywhere. If
  transcribe-project's history store (`src-tauri/src/lib.rs` — `save_transcript`,
  `load_history`, etc., per `__specs__/INDEX.md`'s F04 entry) ever needs a schema change,
  this versioned-migration approach is the pattern to adopt rather than hand-editing the
  JSON-file-based history store it currently uses.
- Separate tables for cached source metadata (`video_info`, `playlist_info`) vs.
  task/job state (`downloads`) — i.e. "what is this URL" is cached independently from
  "what did we do about it." Our job-history/queue work (`F09`, `F10` in `__specs__/`)
  should keep the same separation: video/URL metadata cache vs. transcription-job
  records, rather than one denormalized table.
- Status enum on the job table (`queued/starting/downloading/paused/completed/errored`)
  plus a `queue_index` column, with new jobs marked `starting` if under a
  max-concurrency setting or `queued` otherwise — directly applicable to F09 (Queue
  View / Live Batch Progress): don't spawn unbounded concurrent faster-whisper
  processes, gate on a max-parallel setting the same way.

### 6. Sidecar / binary management

- `scripts/download-bins.js`: per-OS-per-arch binary URLs in one `versions` object,
  archive extraction (`.zip`/`.tar.xz`/`.tar.bz2`) into `src-tauri/binaries/` at build
  time, with macOS needing a universal binary split into separate arch-specific copies.
  Comparable to `scripts/build_sidecar.py` + `transcribe-sidecar.spec` (PyInstaller) in
  this repo — neodlp's per-arch download step happens at *build* time same as ours;
  no new approach here, just confirms build-time-bundle (vs. runtime-download) is the
  standard choice for desktop apps shipping native binaries.
- Sandboxed-environment fallback (Flatpak/AppImage): detect `$FLATPAK`/`$APPDIR`, fall
  back from `Command.sidecar()` to `Command.create('sh', ['-c', ...])` with binaries
  resolved from `$XDG_DATA_HOME` instead of the app bundle. Not currently relevant (no
  Flatpak/AppImage target for transcribe-project) but worth knowing if Linux
  distribution ever expands beyond a plain native build.

---

## How to add the next reference codebase

1. Drop the cloned repo under `.learnings/<name>/` (already gitignored if large/binary —
   check `.gitignore`; these are reference material, not something to commit wholesale
   unless small and license-clean).
2. Identify the 2-4 areas of genuine overlap with transcribe-project's actual open
   problems (check `__specs__/INDEX.md` and `_memory/rna-method/timeline.json`
   `openQuestions[]` first — don't research in a vacuum).
3. Add a new `##` section here following the same shape: what it is, the one structural
   difference to watch for, then findings grouped by the specific pain point they
   address, each with a `path/to/file:line` citation and one sentence on *why* it
   matters for us — not what the code does in the abstract.
4. Cross-reference: if a finding maps to an existing `__specs__/FIX-##` or
   `openspec/changes/*`, name it explicitly so agents connect the two.
