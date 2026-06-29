# Transcribe

A desktop app that transcribes audio from any URL — YouTube, Instagram, TikTok, Twitter/X, and 1000+ other sites. All processing happens **on your machine**: yt-dlp downloads the audio and faster-whisper transcribes it locally. No server, no tracking, no rate limits.

## Quick start (macOS)

### 1. Install prerequisites

```bash
# FFmpeg (required for audio extraction)
brew install ffmpeg

# Rust toolchain (required by Tauri)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python dependencies for the sidecar
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

### 2. Build the sidecar binary

```bash
npm run sidecar:build
```

This produces `src-tauri/binaries/transcribe-sidecar-aarch64-apple-darwin` (or `x86_64-apple-darwin` on Intel Macs) using PyInstaller.

### 3. Install npm dependencies

```bash
npm install
```

### 4. Run in development

```bash
npm run tauri:dev
```

### 5. Build a distributable `.app`

```bash
npm run tauri:build
```

Output: `src-tauri/target/release/bundle/macos/Transcribe.app` and `src-tauri/target/release/bundle/dmg/Transcribe_<version>_<arch>.dmg`.

### 6. First-run model download

On first transcription, the app downloads the `tiny` Whisper model (~75 MB) and caches it in `~/Library/Caches/transcribe-app/models/tiny/` on macOS. Subsequent runs use the cached model.

## Gatekeeper workaround

The `.app` is unsigned in v1. macOS will warn "Transcribe cannot be opened because the developer cannot be verified." After installing:

```bash
xattr -dr com.apple.quarantine /Applications/Transcribe.app
```

Or right-click the app → Open → Open in the dialog. v1.1 will ship a notarized build.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+Enter` | Trigger transcription |
| `Cmd+.` | Cancel in-flight transcription |
| `Cmd+S` | Save transcript (native save dialog) |

## Architecture

```
┌──────────────────────┐      invoke('run_sidecar')     ┌──────────────────────┐
│  SvelteKit webview   │  ───────────────────────────►  │  Rust Tauri layer    │
│  (src/routes/...)    │  ◄─────────────────────────    │  (src-tauri/src/)    │
│                      │  emit('transcribe-progress')  │                      │
└──────────────────────┘                                └──────────┬───────────┘
                                                                   │ spawn
                                                                   ▼
                                                        ┌──────────────────────┐
                                                        │  Python sidecar      │
                                                        │  (PyInstaller bin)   │
                                                        │                      │
                                                        │  yt-dlp + Whisper    │
                                                        └──────────────────────┘
```

- **Frontend**: SvelteKit SPA wrapped by Tauri's webview. Calls `invoke('run_sidecar', ...)` and listens for `transcribe-progress` events.
- **Rust bridge** (`src-tauri/src/lib.rs`): Spawns the sidecar, pipes its stdout to the frontend as Tauri events, manages single-flight concurrency and cancellation.
- **Python sidecar** (`api/sidecar.py`): Reads URL/model/timestamps from CLI args, downloads audio via yt-dlp (no anti-blocking workarounds needed — runs from the user's residential IP), transcribes via faster-whisper, streams progress events as JSON-lines to stdout.
- **Core logic** (`api/transcribe_core.py`): Pure transcription module shared between the Flask server (reference / future hosted path) and the Tauri sidecar.

## Why this exists

The previous architecture ran the same pipeline on a Vultr VPS. Datacenter IPs get flagged by YouTube and Instagram, breaking transcription for users despite extensive anti-blocking workarounds (cookies, PO-token plugins, player-client fallbacks). Moving the work to the user's machine eliminates the entire class of problems.

## v1 limitations

- **macOS only**. Linux and Windows builds are deferred to v1.1.
- **Unsigned `.app`**. Notarization is deferred to v1.1.
- **FFmpeg required**. Must be installed separately on the user's machine. Bundling is deferred to v1.1.
- **No auto-update**. yt-dlp version drift requires a manual rebuild. v1.1 will add an updater.
- **Single model** (`tiny` only). Larger models are deferred to v1.1.
- **Browser dev mode shows "must run in Tauri"**. Use `npm run dev` (web UI) only for static UI work; transcription requires the desktop app.

## Development

### Project layout

```
api/
  transcribe_core.py     # Pure transcription logic (download_audio, transcribe_audio)
  transcribe.py          # Flask wrapper (reference / future hosted)
  sidecar.py             # Tauri sidecar entry point (JSON-lines over stdout)
src/
  routes/
    +page.svelte         # Main UI (URL input, transcribe button, result tabs)
  lib/desktop/           # Desktop-specific UI layer
scripts/
  dev_api.py             # Local Flask dev server
  predownload_model.py   # Pre-download Whisper model (VPS deploy)
  build_sidecar.py       # PyInstaller build for sidecar binary
src-tauri/
  tauri.conf.json        # Tauri app config
  capabilities/          # Permission scopes for plugins
  src/                   # Rust source
  binaries/              # Sidecar binary location (PyInstaller output)
  icons/                 # App icons
svelte.config.js         # adapter-static, SPA fallback (Tauri build)
```

### Local Flask dev (for testing core logic)

The Flask server is preserved as a reference / future hosted path. Run it locally:

```bash
source .venv/bin/activate
python scripts/dev_api.py
```

In another terminal:

```bash
npm run dev
```

Vite proxies `/api/*` to the Flask server on port 8787.

### Verification

```bash
# Rust compiles cleanly
cargo check --manifest-path src-tauri/Cargo.toml

# Python syntax
python3 -c "import ast; ast.parse(open('api/sidecar.py').read())"
```

## Deployment (legacy / reference)

The Vultr + Nginx + Terraform deployment in `deploy/` is preserved for reference or a future hosted offering. It is **not** the primary product anymore. See `deploy/README.md` for the dormant deployment workflow.

## License

Private project. Not yet licensed for public distribution.