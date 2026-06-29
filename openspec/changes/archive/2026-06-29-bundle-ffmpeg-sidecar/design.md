## Context

`api/sidecar.py:check_ffmpeg()` currently does a single `shutil.which("ffmpeg")` check
and fails hard with `FFMPEG_MISSING` if it's empty. `api/transcribe_core.py` never sets
`ydl_opts["ffmpeg_location"]`, so yt-dlp does its own independent `PATH` lookup
internally — meaning even if we bundle a binary, two different lookup paths
(`sidecar.py`'s check vs. yt-dlp's actual usage) would need to agree, or the check
could pass while yt-dlp silently uses a different (or no) ffmpeg.

The `python-sidecar` capability this change modifies was defined inline inside the
still-unarchived `openspec/changes/tauri-desktop-app/specs/python-sidecar/spec.md` —
that proposal explicitly scoped "FFmpeg bundling" out as v1.1 future work. There is no
`openspec/specs/python-sidecar/spec.md` yet (only `local-data-store` has been archived
to main specs so far). This change's delta spec is written against the capability name
`python-sidecar` so it applies correctly whenever that capability is archived, but
implementers should be aware `tauri-desktop-app` is still in flight and may touch the
same file (`api/sidecar.py`) concurrently — check `openspec/changes/tauri-desktop-app/tasks.md`
for unchecked items before starting, to avoid clobbering parallel work.

The sidecar ships as a PyInstaller `--onedir` bundle
(`src-tauri/binaries/transcribe-sidecar-aarch64-apple-darwin/`), referenced by Tauri as
a `bundle.resources` entry (not `externalBin`) and resolved at runtime via
`app.path().resolve_resource(...)` in `src-tauri/src/lib.rs` (`sidecar_path()`). Any
file placed inside that `--onedir` output directory ships inside the `.app` and is
addressable by the sidecar process via a path relative to its own executable
(`sys.executable` when frozen) — no Rust-side changes needed to make a new file
available to Python, only to Tauri's `resources` bundling if it needs to live
*outside* that directory.

## Goals / Non-Goals

**Goals:**
- A DMG built via `npm run sidecar:build && npm run tauri:build` works for
  transcription on a clean macOS machine with zero pre-installed dependencies (no
  Homebrew, no manual ffmpeg install).
- A single, unambiguous ffmpeg resolution order used by both the `FFMPEG_MISSING`
  check and the actual yt-dlp invocation, so the check can never pass while yt-dlp uses
  something different.
- Build-time download of a pinned ffmpeg version (reproducible builds), not a runtime
  download (would reintroduce the "needs network/setup on first run" problem this
  change is trying to eliminate, and contradicts the "no setup" goal more directly
  than today's PATH-based gap does).

**Non-Goals:**
- Windows/Linux ffmpeg bundling (this sidecar build is macOS/`aarch64` only today —
  out of scope until those platforms exist).
- `x86_64-apple-darwin` / universal binary ffmpeg bundling (sidecar itself is
  `aarch64`-only right now per `scripts/build_sidecar.py`'s default; adding ffmpeg
  doesn't change that scope).
- Bundling ffmpeg for the Flask/`dev:api` server code path — that's a dev-machine
  path where Homebrew ffmpeg is a reasonable expectation; this change only needs the
  *Flask* path to keep working via existing PATH lookup, not gain bundling.
- Code-signing/notarization of the new binary specifically (handled, if at all, by
  the existing app-level signing discussed separately — out of scope here).

## Decisions

### Decision: Source a static ffmpeg build from evermeet.cx or a pinned GitHub Releases mirror, not build from source
Building ffmpeg from source in CI/locally is slow (10+ minutes) and adds a heavy
toolchain dependency (autoconf, nasm, yasm, various codec libs) just to produce a
binary that already exists, pre-built and widely trusted, from established static-build
providers. `osxexperts.net` and `evermeet.cx` are the two most commonly used static
macOS ffmpeg build sources; neodlp-main's reference pattern
(`.learnings/neodlp-main/scripts/download-bins.js`) downloads pre-built binaries from
pinned GitHub Releases URLs for its own bundled tools (yt-dlp, aria2c) rather than
building anything from source — same approach, applied to ffmpeg here.

**Alternative considered**: use `imageio-ffmpeg` or another PyPI package that vendors
an ffmpeg binary as a Python wheel, so `--collect-all imageio_ffmpeg` in the existing
PyInstaller invocation picks it up automatically with no new download-and-place step.
Rejected for now because it adds a new Python dependency and its bundled ffmpeg build
may lag behind or differ in codec support from a dedicated static build — worth
revisiting if the manual download step proves brittle, but the explicit-binary
approach matches the existing `scripts/build_sidecar.py` pattern more closely and
keeps full control over the exact ffmpeg version/build flags (e.g. codec support for
the specific formats yt-dlp needs to remux).

### Decision: Place the ffmpeg binary inside the existing sidecar `--onedir` resource directory, not as a separate Tauri resource
`tauri.conf.json`'s `bundle.resources` already includes the whole
`binaries/transcribe-sidecar-aarch64-apple-darwin` directory. Dropping `ffmpeg` (and
`ffprobe`, since yt-dlp/ffmpeg post-processing sometimes probes before remuxing) inside
that same directory means zero changes to `tauri.conf.json` or `src-tauri/src/lib.rs`
— the binary just needs to be found at a path relative to wherever the sidecar
executable resolves itself (`Path(sys.executable).parent / "ffmpeg"` when
PyInstaller-frozen). This avoids introducing a second resource-resolution code path
in Rust for a single extra binary.

**Alternative considered**: a separate `binaries/ffmpeg-aarch64-apple-darwin` entry
alongside the sidecar's own directory, declared as its own `tauri.conf.json` resource.
Rejected — adds a second Rust-side resource lookup for no benefit; the sidecar is the
only consumer of ffmpeg, so it should own and resolve it internally rather than have
Tauri/Rust mediate.

### Decision: Resolution order is bundled-path-first, falling back to PATH only when not frozen
```
1. If running as a PyInstaller-frozen binary: check `<exe_dir>/ffmpeg` (and `ffprobe`).
   If present, use it. (This is the only path the shipped .app ever takes.)
2. Otherwise (dev mode via `scripts/dev_api.py`/Flask, or frozen but binary missing —
   treat missing-when-frozen as a packaging bug, not silently fall through): fall back
   to `shutil.which("ffmpeg")`.
3. If neither resolves: raise FFMPEG_MISSING as today.
```
This keeps dev-machine ergonomics unchanged (Homebrew ffmpeg still works for
`tauri:dev`/`dev:all`) while guaranteeing the shipped bundle is self-contained. The
*same* resolved path is passed to both `check_ffmpeg()`'s validation and
`ydl_opts["ffmpeg_location"]`, eliminating the two-divergent-lookups risk described in
Context.

**Alternative considered**: always prefer `PATH` over bundled, with bundled only as
fallback. Rejected — defeats the purpose; a stale or differently-configured Homebrew
ffmpeg on a dev machine would silently shadow the bundled, tested version, making "it
works on my machine but not the bundle" bugs more likely, not less.

### Decision: Pin an exact ffmpeg version/build, recorded in the fetch script, not "latest"
Mirrors the existing repo convention (PyInstaller pinned in `requirements`, faster-whisper
pinned) and the explicit critique already on file in `.learnings/index.md` that
neodlp's `ffmpeg-ffprobe: 'latest'` entry is the one un-pinned version in their
otherwise-pinned `versions` object — a known anti-pattern to avoid repeating.

## Risks / Trade-offs

- **[Risk] Static ffmpeg builds from third-party sites can disappear or change URLs
  without notice** → Mitigation: pin to a specific dated build URL captured at
  implementation time; if the build script's download fails, fail loudly at build time
  (not silently produce a DMG without ffmpeg) so this is caught before distribution,
  not discovered by a beta tester.
- **[Risk] DMG size grows ~40-80MB** → Mitigation: already called out in the proposal's
  Impact section as an accepted, deliberate trade-off for a genuinely self-contained
  desktop app; no further mitigation planned (e.g. no on-demand download-after-install
  scheme — that would reintroduce the exact "needs network on first use" problem this
  change exists to remove).
- **[Risk] Concurrent edits from the still-in-flight `tauri-desktop-app` change touching
  `api/sidecar.py`** → Mitigation: check `tasks.md` of that change before starting
  implementation; rebase/re-verify `check_ffmpeg()` changes land cleanly relative to
  whatever that change has done by the time this is implemented.
- **[Risk] Codesigning/notarization of a new third-party binary inside the bundle**
  could complicate future notarization work (every binary inside a notarized `.app`
  must itself be signed) → Mitigation: out of scope for *this* change per Non-Goals,
  but flag explicitly in tasks.md as a known follow-up once code signing is tackled,
  so it isn't forgotten.

## Migration Plan

No data migration. Rollout is purely build-pipeline: next `sidecar:build` run produces
a sidecar directory containing `ffmpeg`/`ffprobe`; next `tauri:build` picks it up
automatically via the existing `bundle.resources` glob on that directory. Rollback is
trivial — revert the build script and `check_ffmpeg()`/`ffmpeg_location` changes; old
behavior (PATH-only) returns immediately with no persisted state to undo.

## Open Questions

- Exact pinned ffmpeg static-build source/URL/version to use — needs to be decided
  during implementation (tasks.md should call out picking and recording this).
- Whether `ffprobe` is actually invoked anywhere in the current yt-dlp/transcribe_core
  flow, or only `ffmpeg` — worth confirming before bundling both, to avoid bundling a
  binary that's never used.
