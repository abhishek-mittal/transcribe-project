## Context

The transcribe-project currently runs as a Flask API (`api/transcribe.py`) on a Vultr VPS, with a SvelteKit frontend that streams transcription results via SSE. The server downloads audio using yt-dlp and transcribes with faster-whisper. Extensive anti-blocking workarounds (cookies, PO-token plugin, player-client fallbacks, residential proxies) have been layered on, but the fundamental problem remains: datacenter IPs get flagged by YouTube and Instagram, breaking the core user flow unpredictably.

The decision is to ship a Tauri 2.x desktop app that runs yt-dlp + faster-whisper locally on the user's machine. This eliminates the datacenter-IP problem entirely, removes ongoing server costs, and reuses the existing SvelteKit UI and Python transcription core. The web app is repurposed as the Tauri webview UI, augmented with a desktop-native UI layer (`src/lib/desktop/`) for native-feeling affordances.

## Goals / Non-Goals

**Goals:**
- Ship a working EOD MVP on macOS that proves the loop: paste URL → sidecar → transcript.
- Reuse the existing SvelteKit frontend UI as the Tauri webview UI.
- Reuse the existing Python transcription logic (`download_audio`, `transcribe_audio`, formatting helpers) by extracting them into a sidecar-callable module.
- Stream progress events (downloading-model → downloading → transcribing → done) from the Python sidecar to the frontend in real time via JSON-lines over stdout.
- Bundle or lazy-download the `tiny` Whisper model so transcription works out-of-the-box on first run.
- Support cancellation mid-flight (user clicks Stop → SIGTERM → sidecar cleans up).
- Single-flight concurrency (clicking Transcribe while a job is in flight cancels the previous one).
- Provide a desktop-native UI layer: drag-and-drop URL input, keyboard shortcuts, native file save dialog, system menu items, title bar handling.
- Use system FFmpeg with a clear error if missing (skip FFmpeg bundling for v1).

**Non-Goals:**
- Notarization / code signing (v1.1).
- Cross-platform builds (Linux, Windows) — v1 is macOS-only (v1.1).
- Auto-update via Tauri's updater plugin (v1.1).
- FFmpeg bundling (v1.1).
- User accounts, cloud sync, or multi-device transcript access (future hosted offering).
- Browser extension or URL-capture plugin (Option 1 from the analysis — deferred).
- Supporting models larger than `tiny` in v1.
- Removing the existing Vultr deployment code from the repo (keep for reference / future hosted path).

## Decisions

### D1: Tauri 2.x over Electron

**Choice**: Tauri 2.x.
**Rationale**: Tauri uses the system webview (WebKit on macOS, WebKitGTK on Linux, WebView2 on Windows), producing binaries ~10-20 MB vs Electron's ~100+ MB. The SvelteKit frontend works in Tauri's webview with minimal adaptation. Tauri 2.x has first-class Svelte support, a clean sidecar/IPC API, and a capabilities/permissions system that gives us a clear security boundary.
**Alternatives considered**:
- *Electron*: Larger binaries, bundles Chromium, heavier resource usage.
- *Neutralino*: Lighter than Electron but less mature sidecar/IPC story and smaller ecosystem.

### D2: Python sidecar via PyInstaller-bundled binary

**Choice**: Package the Python transcription logic into a standalone binary using PyInstaller, invoked as a Tauri sidecar process.
**Rationale**: Users should not need Python installed. PyInstaller bundles the interpreter + all deps (`yt-dlp`, `faster-whisper`, `av`, `huggingface_hub`) into a single executable per platform. Tauri's sidecar API handles spawning and stdio piping natively.
**Trade-off**: PyInstaller binaries for Python + yt-dlp + faster-whisper + av can be ~150-200 MB uncompressed. We use `--onefile` with UPX compression for v1; revisit with `nuitka` if size becomes a real problem.

### D3: stdout JSON-lines protocol for sidecar ↔ frontend IPC

**Choice**: The sidecar reads a single JSON request from stdin and writes newline-delimited JSON events to stdout. Tauri's Rust layer reads stdout line-by-line and forwards events to the frontend via Tauri events (`app_handle.emit`).
**Protocol**:
```
# Request (stdin, single line):
{"url": "https://...", "model": "tiny", "timestamps": true}

# Events (stdout, one JSON object per line):
{"event": "phase", "phase": "downloading-model", "progress": 0.42}
{"event": "phase", "phase": "downloading"}
{"event": "progress", "phase": "transcribing", "segment": 3, "text": "..."}
{"event": "result", "language": "en", "plain": "...", "timestamped": "...", "srt": "..."}
{"event": "error", "code": "BOT_CHALLENGE", "message": "..."}
{"event": "done"}
```

**Rationale**: JSON-lines is simple, debuggable (pipe sidecar binary directly in terminal), and language-agnostic. Avoids custom binary protocols or HTTP-over-localhost complexity.

### D4: Structured error codes from the sidecar

**Choice**: Every `error` event from the sidecar includes a `code` field in addition to `message`. The frontend maps codes to actionable user-facing copy.
**Codes (v1)**:
- `INVALID_URL` — empty, too long (>2048), or wrong scheme
- `NETWORK` — DNS/connectivity failure
- `BOT_CHALLENGE` — yt-dlp reported "Sign in to confirm you're not a bot" or similar
- `UNSUPPORTED_PLATFORM` — yt-dlp has no extractor for the URL
- `FFMPEG_MISSING` — FFmpeg not found on PATH (v1 only)
- `MODEL_LOAD_FAILED` — faster-whisper failed to load the model
- `INTERNAL` — unexpected exception (last resort)

**Rationale**: Surfacing the raw yt-dlp exception to users leaks internals and is unfriendly. Structured codes let the UI show "YouTube is blocking this video. Try a different one" vs "Internal error: <traceback>".

### D5: Model download progress via `huggingface_hub.snapshot_download`

**Choice**: Before loading the model, the sidecar calls `huggingface_hub.snapshot_download` with a progress callback to emit `phase: "downloading-model"` events with `progress: 0.0-1.0`. After download completes, the sidecar loads the model with the local path.
**Rationale**: `WhisperModel.__init__` blocks during download and offers no progress hooks. By pre-downloading via `snapshot_download`, we get a clear progress signal and can show a "Downloading speech model (one-time, ~75 MB)" UI with a progress bar. The existing `_resolve_model_source` logic in `transcribe_core.py` is updated to know about this two-step flow.

### D6: Cancellation via SIGTERM + Rust-managed child PID

**Choice**: The Rust Tauri command stores the running sidecar's child process PID in a `Mutex<Option<Child>>`. A separate `cancel_transcribe` Tauri command sends `SIGTERM` to that PID. The Python sidecar installs a SIGTERM handler that cleans up the temp directory and exits with status 130.
**Rationale**: SIGTERM is portable (macOS, Linux; Windows has TerminateProcess in v1.1), standard, and gives the sidecar a chance to clean up. The Python `signal.signal(SIGTERM, handler)` registration works on macOS; on Windows we fall back to `taskkill /PID <pid> /T /F` from the Rust side. v1 is macOS-only so SIGTERM is sufficient.
**Trade-off**: Cleanup is best-effort. If the SIGTERM handler is mid-yt-dlp-call, cleanup runs in the finally block.

### D7: Single-flight concurrency in the Rust layer

**Choice**: The `transcribe` Tauri command holds a `Mutex<Option<Child>>`. If `transcribe` is invoked while a child is running, the Rust layer first sends SIGTERM to the previous child, awaits its exit (with a 2-second timeout), then spawns the new one. If cleanup times out, the previous child is force-killed.
**Rationale**: Two concurrent yt-dlp + faster-whisper processes on the same machine would compete for CPU/memory and produce confusing UI state. The frontend can also enforce "disable button while loading" but the Rust layer is the source of truth.

### D8: Extract transcription logic into `api/transcribe_core.py`

**Choice**: Refactor `api/transcribe.py` to extract `download_audio`, `transcribe_audio`, formatting helpers, and model management into `api/transcribe_core.py` (a pure module with no Flask dependency). The Flask app imports from it for backward compat; the new sidecar imports from it too.
**Rationale**: Keeps the existing Flask server working (for reference / future hosted path) while making the core logic reusable by the sidecar. Minimal diff, DRY.

### D9: Frontend adaptation — Tauri `invoke` + event listener + desktop UI layer

**Choice**: The SvelteKit frontend's `transcribe()` function is changed from `fetch('/api/transcribe/stream')` to `invoke('transcribe', { url, model, timestamps })`. Progress events are received via `listen('transcribe-progress', ...)`. A new `src/lib/desktop/` module wraps Svelte components with desktop affordances:
- `<DesktopTitleBar />` — uses Tauri `getCurrentWindow().setTitle()` and emits native-style title bar styles
- `<UrlDropZone />` — wraps the URL input with HTML5 drag-and-drop for video URLs from the browser
- `<SaveTranscriptButton />` — uses `tauri-plugin-dialog`'s `save` to write the transcript to disk with native file picker
- `<KeyboardShortcuts />` — global keybindings: `Cmd+Enter` triggers transcription, `Cmd+.` cancels, `Cmd+S` saves
- `<SystemMenu />` — registers "New Transcription", "Save Transcript", "Quit" items via Tauri's menu API

**Rationale**: Desktop users expect native-feeling affordances. Drag-and-drop, keyboard shortcuts, native save dialogs, and system menu items are table stakes for a desktop app — without them the app feels like a webpage in a window. The `src/lib/desktop/` module is the boundary between "Svelte component" and "Tauri-aware desktop component".

### D10: No server-side anti-blocking workarounds in sidecar

**Choice**: The sidecar invokes yt-dlp with default options. No cookies, PO-token plugin, player-client fallback, or proxy. Requests originate from the user's residential IP.
**Rationale**: Anti-blocking workarounds were necessary because the server IP was datacenter-class. On the user's machine, their IP is residential — YouTube/Meta won't flag it under normal usage. Keeping the workarounds in the sidecar would be dead code and a maintenance burden.

### D11: System FFmpeg with friendly error (v1)

**Choice**: The sidecar does NOT bundle FFmpeg. It checks for FFmpeg on `PATH` via `shutil.which("ffmpeg")` and emits an `FFMPEG_MISSING` error event if absent. The UI shows: "FFmpeg is required. Install it with `brew install ffmpeg` (macOS) or see https://ffmpeg.org/download.html."
**Rationale**: Bundling FFmpeg adds ~80 MB and platform-specific complexity (LGPL/GPL compliance, code signing, per-arch builds). For v1 on macOS, `brew install ffmpeg` is a one-line setup step most macOS developers already have done.
**Trade-off**: Extra setup step for users. Acceptable for an MVP; revisit in v1.1.

## Risks / Trade-offs

- **[No FFmpeg bundling]** → Users must install FFmpeg. Mitigation: Clear error message with install command. v1.1 ships bundled FFmpeg.
- **[No notarization]** → Unsigned macOS `.app` shows Gatekeeper warnings. Mitigation: README documents `xattr -dr com.apple.quarantine /Applications/Transcribe.app` or right-click → Open. v1.1 ships notarized.
- **[macOS-only]** → v1 doesn't ship Linux or Windows builds. Mitigation: Document v1 as macOS-only. v1.1 adds Linux (AppImage) and Windows.
- **[yt-dlp version drift]** → yt-dlp breaks as platforms change. Users can't `pip install --upgrade` in a frozen binary. Mitigation: Document that the binary is pinned to a specific yt-dlp version; v1.1 adds auto-update or a "Check for updates" button.
- **[App size]** → PyInstaller binaries for Python + yt-dlp + faster-whisper + av can be ~150-200 MB. Mitigation: `--onefile` + UPX compression; document size honestly. Re-evaluate with `nuitka` in v1.1.
- **[First-run model download]** → ~75 MB download on first transcription. Mitigation: `downloading-model` phase event with progress bar in UI (see D5).
- **[PyInstaller hidden imports]** → yt-dlp loads ~300 extractor modules dynamically. Mitigation: Explicit `--collect-all yt_dlp --collect-all faster_whisper --collect-all av --collect-all huggingface_hub` in PyInstaller spec.
- **[No SSR]** → `adapter-static` SPA mode means SEO/indexability is lost. Mitigation: Acceptable for a desktop app (no public URL). The (dormant) `adapter-node` path remains in-repo for a future hosted offering.
- **[HuggingFace rate limits]** → First-run model download can hit HF rate limits if many users download simultaneously. Mitigation: Sidecar emits a clear error if 429 received; user retries.

## Migration Plan

1. **Refactor core** → Extract `transcribe_core.py` from `transcribe.py` (no behavior change to Flask app).
2. **Add sidecar** → Create `api/sidecar.py` with JSON-lines protocol, structured errors, SIGTERM handler, model download progress.
3. **Switch adapter** → Add `adapter-static` for Tauri builds; keep `adapter-node` available.
4. **Tauri scaffold** → `npm create tauri-app`, configure capabilities, register sidecar.
5. **Rust bridge** → `transcribe` command (spawn + read stdout + emit events), `cancel_transcribe` command (SIGTERM).
6. **Frontend adaptation** → Replace `fetch` with `invoke` + `listen`; add `src/lib/desktop/` module.
7. **PyInstaller build** → Create `scripts/build_sidecar.py` with all `--collect-all` flags; produce macOS binary.
8. **E2E test** → `tauri dev`, transcribe one YouTube URL, verify phases, cancellation, save dialog.
9. **Rollback** → The Flask server path remains fully functional — if the Tauri app has issues, the web app can be deployed to Vultr independently.

## Open Questions

- **Save dialog default filename**: Use the video title from yt-dlp metadata, or timestamp? (Leaning: video title.)
- **Drag-and-drop validation**: Accept any URL-looking string, or fetch the page title first to confirm? (Leaning: accept any http(s) URL; let yt-dlp fail gracefully.)
- **System menu items**: Add "Open Recent" or "Preferences" in v1, or defer? (Leaning: defer.)
- **Window state persistence**: Save window size/position on close and restore on open? (Leaning: defer to v1.1; Tauri 2's window-state plugin is one line.)