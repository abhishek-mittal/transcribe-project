## Why

The current server-based architecture (Flask API on a Vultr VPS) is fundamentally limited by datacenter-IP blocking from YouTube and Instagram. Despite extensive workarounds (cookies jars, player-client fallback chains, PO-token plugins, residential proxies), the server's IP still gets flagged, breaking transcription for users. Moving the download and transcription to the user's own machine via a Tauri desktop app eliminates this entire class of problems — requests originate from a residential IP, no anti-bot bypasses are needed, and there are zero ongoing server costs.

**MVP scope (EOD)**: ship a working end-to-end transcription flow on macOS that proves the loop (paste URL → sidecar → transcript). Full distribution polish (notarization, FFmpeg bundling, cross-platform CI) follows in v1.1.

## What Changes

- **BREAKING**: The web app (`src/routes/+page.svelte`) no longer fetches from `/api/transcribe/stream`. It is repurposed as the Tauri webview UI, calling a local Python sidecar instead of a remote server.
- **BREAKING**: The Flask API (`api/transcribe.py`) is no longer deployed as a Gunicorn server. Its core transcription logic (`download_audio`, `transcribe_audio`, formatting helpers) is extracted into a standalone Python CLI module invoked as a Tauri sidecar process.
- Add a Tauri 2.x shell (`src-tauri/`) that wraps the SvelteKit frontend and spawns the Python sidecar.
- Add a Python sidecar entry point (`api/sidecar.py`) that reads a JSON request from stdin, streams JSON-lines progress events to stdout, and exits on completion or signal.
- Add a desktop-specific UI layer (`src/lib/desktop/`) with native-feeling components: title bar handling, system menu integration, file save dialogs, drag-and-drop URL input, keyboard shortcuts, and a desktop-mode-aware layout.
- Bundle the `tiny` Whisper model on first run (lazy download via `huggingface_hub`) so transcription works offline after install.
- Add structured error codes from the sidecar (network, bot-challenge, ffmpeg-missing, model-load-failure) so the UI can show actionable messages.
- Add cancellation support: the frontend can invoke a `cancel-transcribe` command that sends SIGTERM to the running sidecar, which cleans up temp files and exits.
- Add concurrency policy: single-flight transcription — clicking Transcribe while a job is in flight cancels the previous one.
- Add Tauri capabilities/permissions (`capabilities/default.json`) whitelisting `shell`, `event`, `dialog`, and `fs` plugins.
- Remove all server-side anti-blocking workarounds from the sidecar path (cookies, PO-token, player-client fallback, proxy) — they are unnecessary when running on the user's residential IP.
- Remove the Vultr Terraform deployment and systemd services from the active delivery path (kept in repo for reference, not deployed).

## Capabilities

### New Capabilities
- `desktop-app-shell`: Tauri application scaffolding, lifecycle management, window configuration, capabilities/permissions, and build pipeline for macOS (v1). Cross-platform distribution follows in v1.1.
- `desktop-app-ui`: Desktop-native UI layer — title bar, system menu integration, file save dialogs, drag-and-drop URL input, keyboard shortcuts, desktop-mode-aware layout. Reuses SvelteKit components but wraps them with desktop conventions.
- `python-sidecar`: Local Python process spawned by Tauri that handles audio download (yt-dlp) and transcription (faster-whisper), streaming progress events back to the frontend via stdout JSON-lines. Handles SIGTERM cancellation, structured error codes, and model download progress.
- `local-transcription-flow`: End-to-end user flow — paste URL (or drag-drop), trigger sidecar, stream progress, support cancellation, display results — running entirely on the client machine with no server dependency. Single-flight concurrency policy.
- `model-bundling`: Lazy-download strategy for shipping Whisper model weights so transcription works out-of-the-box on first run with progress feedback.

### Modified Capabilities
<!-- No existing specs in openspec/specs/ — this is the first spec-driven change. -->

## Impact

- **Code**: `api/transcribe.py` is refactored to extract reusable functions into `api/transcribe_core.py`; `src/routes/+page.svelte` is adapted to call Tauri `invoke` instead of `fetch('/api/...')`; new `src/lib/desktop/` UI module added.
- **Dependencies**: Add `@tauri-apps/api`, `@tauri-apps/cli`, Rust toolchain (Tauri 2.x), `tauri-plugin-dialog`, `tauri-plugin-fs`. Python deps (`yt-dlp`, `faster-whisper`, `av`) remain but are bundled via PyInstaller.
- **Deployment**: Vultr VPS, Nginx, systemd services, and Terraform infra are no longer required for the primary product. They remain in-repo for reference or a future hosted offering.
- **Distribution**: Users install a native macOS app instead of visiting a URL (v1). First-run experience includes a ~75 MB model download. Notarization and cross-platform builds (Linux, Windows) follow in v1.1.
- **Security**: Audio is downloaded and processed locally — no data leaves the user's machine. No server attack surface. The sidecar validates all inputs (URL length, scheme) before invoking yt-dlp.
- **UX**: Adds desktop-native affordances — drag-and-drop, keyboard shortcuts, native file save dialog, system menu items — that don't exist on the web.