## MODIFIED Requirements

### Requirement: Structured error codes
The sidecar SHALL classify errors into structured codes so the UI can show actionable messages. The code SHALL be one of:
- `INVALID_URL` — empty, too long, or wrong scheme
- `NETWORK` — DNS or connectivity failure
- `BOT_CHALLENGE` — yt-dlp error message contains "Sign in to confirm" or similar bot-detection phrases
- `UNSUPPORTED_PLATFORM` — yt-dlp has no extractor for the URL
- `FFMPEG_MISSING` — neither a bundled ffmpeg binary (when running as a frozen PyInstaller build) nor a PATH-resolved `ffmpeg` (`shutil.which("ffmpeg")`) can be found
- `MODEL_LOAD_FAILED` — faster-whisper or huggingface_hub raised during model load
- `INTERNAL` — any other unexpected exception

#### Scenario: Bot challenge classified
- **WHEN** yt-dlp raises a `DownloadError` whose message contains "Sign in to confirm you're not a bot"
- **THEN** the sidecar SHALL emit `{"event": "error", "code": "BOT_CHALLENGE", "message": "YouTube is blocking this video. Try a different one or sign in to YouTube in your browser."}`

#### Scenario: Bundled ffmpeg found (frozen build)
- **WHEN** the sidecar starts as a PyInstaller-frozen binary and an `ffmpeg` executable exists at the path relative to the sidecar's own executable directory (e.g. `<exe_dir>/ffmpeg`)
- **THEN** the sidecar SHALL use that bundled path for both its own validation check and as the value passed to yt-dlp's `ffmpeg_location` option, without requiring `ffmpeg` to be present on `PATH`

#### Scenario: Bundled ffmpeg missing, falls back to PATH (dev mode)
- **WHEN** the sidecar is NOT running as a frozen PyInstaller binary (e.g. invoked directly via `python api/sidecar.py` during development, or via the Flask `dev:api` path) and no bundled ffmpeg path applies
- **THEN** the sidecar SHALL fall back to resolving `ffmpeg` via `shutil.which("ffmpeg")` exactly as it does today, and use that resolved path for `ffmpeg_location`

#### Scenario: FFmpeg missing entirely
- **WHEN** the sidecar starts and neither a bundled ffmpeg binary (frozen build) nor a PATH-resolved `ffmpeg` (dev mode) can be found
- **THEN** the sidecar SHALL emit `{"event": "error", "code": "FFMPEG_MISSING", "message": "FFmpeg is required. Install with `brew install ffmpeg` (macOS) or see https://ffmpeg.org/download.html"}` and exit immediately

### Requirement: yt-dlp ffmpeg invocation
The system SHALL pass an explicit `ffmpeg_location` to yt-dlp's `ydl_opts`, set to whichever ffmpeg path was resolved by the sidecar's ffmpeg-resolution logic (bundled or PATH-resolved), rather than relying on yt-dlp's own independent, implicit `PATH` lookup.

#### Scenario: Bundled ffmpeg used for download/remux
- **WHEN** the sidecar resolves a bundled ffmpeg path at startup (frozen build)
- **THEN** every `yt_dlp.YoutubeDL(ydl_opts)` invocation in `transcribe_core.py` SHALL include `ydl_opts["ffmpeg_location"]` set to that resolved bundled path, so stream merging/remuxing uses the bundled binary regardless of what (if anything) is on the host's `PATH`

#### Scenario: PATH-resolved ffmpeg used in dev mode
- **WHEN** the sidecar resolves ffmpeg via `shutil.which("ffmpeg")` (dev mode fallback)
- **THEN** `ydl_opts["ffmpeg_location"]` SHALL be set to that resolved PATH location, preserving today's dev-machine behavior
