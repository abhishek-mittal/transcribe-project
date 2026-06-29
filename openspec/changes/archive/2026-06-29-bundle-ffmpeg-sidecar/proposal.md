## Why

The desktop app currently requires `ffmpeg` to be pre-installed and on `PATH` on the
user's machine (`api/sidecar.py:check_ffmpeg()` calls `shutil.which("ffmpeg")` and
fails with `FFMPEG_MISSING` otherwise). This breaks the "local-first, on-device,
no setup" value proposition the app is built around: every other native dependency
(SQLite via `rusqlite`'s `bundled` feature, FFmpeg's *libraries* via PyAV, faster-whisper,
CTranslate2) is already bundled into the app or sidecar, but the FFmpeg *CLI binary*
that yt-dlp shells out to for stream merging/remuxing is not. A fresh test machine
(verified during beta DMG testing) cannot transcribe anything until someone manually
runs `brew install ffmpeg`. This change closes that last gap so the DMG is genuinely
self-contained.

## What Changes

- Vendor a static, statically-linked `ffmpeg` (and `ffprobe`) binary per target
  architecture (`aarch64-apple-darwin` to start, matching the existing sidecar binary
  naming convention) into `src-tauri/binaries/`, downloaded at build time rather than
  committed to git (same pattern as `scripts/build_sidecar.py`'s PyInstaller step and
  neodlp-main's `scripts/download-bins.js` reference implementation — see
  `.learnings/index.md` section 6).
- Add a new build step (`scripts/fetch_ffmpeg.py` or extend `build_sidecar.py`) that
  downloads the binary from a pinned, versioned upstream release URL and places it
  inside the sidecar's PyInstaller `--onedir` output directory (or as a sibling Tauri
  resource) so it ships inside the `.app` bundle.
- Change `api/sidecar.py:check_ffmpeg()` to resolve ffmpeg in this order: (1) a bundled
  path relative to the sidecar executable, (2) fall back to `PATH` via
  `shutil.which("ffmpeg")` for the Flask/dev-server code path. Only raise
  `FFMPEG_MISSING` if neither resolves.
- Pass the resolved ffmpeg path explicitly to yt-dlp via `ydl_opts["ffmpeg_location"]`
  in `api/transcribe_core.py` instead of relying on yt-dlp's own implicit `PATH`
  lookup, so the bundled binary is actually used once bundled (today nothing sets
  `ffmpeg_location` at all).
- Update `tauri.conf.json` `bundle.resources` / the PyInstaller build script so the
  ffmpeg binary travels with the `--onedir` sidecar directory automatically (no new
  Rust-side resource entry needed if it lives inside the existing sidecar resource dir).

## Capabilities

### New Capabilities
(none — this extends the existing sidecar capability, it doesn't introduce a new one)

### Modified Capabilities
- `python-sidecar`: the `FFMPEG_MISSING` error requirement changes from "raise
  immediately if `ffmpeg` is not on PATH" to "raise only if neither a bundled ffmpeg
  binary nor a PATH-resolved one is found"; adds a new requirement that yt-dlp is
  invoked with an explicit `ffmpeg_location` pointing at whichever path was resolved.

## Impact

- **Affected code**: `api/sidecar.py` (`check_ffmpeg`), `api/transcribe_core.py`
  (`ydl_opts` construction), `scripts/build_sidecar.py` (or new fetch script),
  `src-tauri/tauri.conf.json` (bundle resources, if the binary needs its own resource
  entry rather than living inside the sidecar's existing resource dir), `src-tauri/src/lib.rs`
  if the Rust side needs to resolve/pass the ffmpeg path (likely not — the sidecar
  resolves it internally).
- **Affected tests**: `api/` test suite needs coverage for the new resolution order
  (bundled path found / bundled path missing+PATH found / neither found) per this
  repo's test-first discipline (`AGENTS.md`).
- **Build/CI**: adds a new download step to the build pipeline; needs a pinned ffmpeg
  build URL/version (mirroring how `scripts/build_sidecar.py` pins nothing today but
  `neodlp-main`'s `download-bins.js` pins exact versions — this change should pin a
  version too, not float on "latest", for build reproducibility).
- **Distribution size**: a static ffmpeg binary adds roughly 40-80MB to the bundled
  sidecar directory and therefore the DMG — acceptable for a desktop app shipped as a
  single download, but worth calling out since this is the largest deliberate
  size increase since project inception.
- **Platforms**: this proposal scopes to macOS (`aarch64-apple-darwin`) only, matching
  the existing sidecar's current single-target build. `x86_64-apple-darwin` /
  `universal` and Windows/Linux targets are out of scope until those platforms are
  otherwise supported.
