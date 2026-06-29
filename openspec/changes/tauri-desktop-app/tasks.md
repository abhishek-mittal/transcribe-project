## 1. Core Logic Extraction

- [x] 1.1 Create `api/transcribe_core.py` — extract `download_audio`, `transcribe_audio`, `format_timestamp`, `format_srt_timestamp`, `_get_model`, `_resolve_model_source`, `_MODEL_CACHE`, `MODELS_DIR`, `VALID_MODELS`, `DEFAULT_MODEL`, and host helpers from `api/transcribe.py`
- [x] 1.2 Update `api/transcribe.py` to import all extracted symbols from `transcribe_core.py` (Flask app remains functional for reference)
- [x] 1.3 Verify Flask dev server (`scripts/dev_api.py`) still works end-to-end after the refactor

## 2. Python Sidecar

- [x] 2.1 Create `api/sidecar.py` — reads JSON request from stdin, validates URL (non-empty, ≤2048 chars, http/https scheme) and model (`tiny` only)
- [x] 2.2 Implement the JSON-lines event protocol: emit `phase` events (`downloading-model` with progress, `downloading`, `transcribing`), `progress` events (segment text), `result` event (full transcript), `error` events with structured codes, and `done` event
- [x] 2.3 Implement structured error codes: `INVALID_URL`, `NETWORK`, `BOT_CHALLENGE`, `UNSUPPORTED_PLATFORM`, `FFMPEG_MISSING`, `MODEL_LOAD_FAILED`, `INTERNAL` — with regex matching on yt-dlp error messages to classify `BOT_CHALLENGE` and `UNSUPPORTED_PLATFORM`
- [x] 2.4 Add FFmpeg detection: check `shutil.which("ffmpeg")` at startup; emit `FFMPEG_MISSING` and exit if absent
- [x] 2.5 Implement two-step model loading: call `huggingface_hub.snapshot_download` with progress callback emitting `downloading-model` events, then load model from returned path
- [x] 2.6 Wire sidecar to call `download_audio` and `transcribe_audio` from `transcribe_core.py` with default yt-dlp options (no cookies, no PO-token, no proxy, no player-client fallback)
- [x] 2.7 Install SIGTERM handler: clean up temp directory in handler, exit with status 130
- [ ] 2.8 Test sidecar standalone: pipe a real YouTube URL via stdin, verify all events stream correctly including model download progress

## 3. Tauri Scaffolding

- [x] 3.1 Install Tauri CLI: `npm install -D @tauri-apps/cli@^2 @tauri-apps/api@^2 tauri-plugin-dialog tauri-plugin-fs`; verify Rust toolchain is installed
- [x] 3.2 Run `npx tauri init` in the repo root — configure app name "Transcribe", frontend dist dir to SvelteKit static build output, dev URL to `http://localhost:5173`
- [x] 3.3 Configure `src-tauri/tauri.conf.json` — window minimum 800×600, title "Transcribe", bundle identifier `com.shuhari.transcribe`
- [x] 3.4 Create `src-tauri/capabilities/default.json` — whitelist `core:default`, `core:event:default`, `shell:allow-execute` (scoped to sidecar), `dialog:allow-save`, `dialog:allow-open`, `fs:allow-write-file`
- [x] 3.5 Register sidecar binary in `tauri.conf.json` under `bundle.externalBin` with platform-specific naming

## 4. SvelteKit Static Build

- [x] 4.1 Install `@sveltejs/adapter-static`
- [x] 4.2 Configure `svelte.config.js` to use `adapter-static` with `fallback: 'index.html'` (SPA mode); keep `adapter-node` config commented for reference
- [ ] 4.3 Verify `npm run build` produces static files that Tauri can load

## 5. Rust Sidecar Bridge

- [x] 5.1 Implement `transcribe` command in `src-tauri/src/main.rs` — spawn sidecar binary via `tauri-plugin-shell`'s `Command::new_sidecar("transcribe-sidecar")`, write JSON request to stdin, read stdout line-by-line
- [x] 5.2 For each stdout line, parse as JSON and emit Tauri event (`app_handle.emit("transcribe-progress", payload)`)
- [x] 5.3 Implement `cancel_transcribe` command — stores running child PID in `Mutex<Option<Child>>`, sends SIGTERM, force-kills after 2s timeout
- [x] 5.4 Enforce single-flight concurrency: if `transcribe` is called while a child is running, SIGTERM the previous one, await exit (2s timeout), then spawn new
- [x] 5.5 Register both commands in the Tauri builder (`invoke_handler`)

## 6. Frontend Adaptation

- [x] 6.1 Add Tauri API imports to `src/routes/+page.svelte`: `import { invoke } from '@tauri-apps/api/core'` and `import { listen } from '@tauri-apps/api/event'`
- [x] 6.2 Replace the `fetch('/api/transcribe/stream')` call with `invoke('transcribe', { url, model, timestamps })`
- [x] 6.3 Replace the `ReadableStream` reader loop with `listen('transcribe-progress', ...)` event handler that updates `phase`, `streamSegments`, and `result`
- [x] 6.4 Add handling for `downloading-model` phase with progress bar UI
- [x] 6.5 Add structured error code → user-friendly message mapping (BOT_CHALLENGE, FFMPEG_MISSING, NETWORK, etc.)
- [x] 6.6 Add Stop button + `Cmd+.` shortcut that calls `invoke('cancel-transcribe')`
- [x] 6.7 Add Tauri environment detection: check `window.__TAURI_INTERNALS__` and disable Transcribe button in browser mode
- [x] 6.8 Wire `Cmd+Enter` shortcut to trigger transcription and `Cmd+S` to open save dialog

## 7. Desktop UI Layer

- [x] 7.1 Create `src/lib/desktop/` directory with components: `DesktopTitleBar.svelte`, `UrlDropZone.svelte`, `SaveTranscriptButton.svelte`, `KeyboardShortcuts.svelte`, `SystemMenu.svelte`
- [x] 7.2 Implement `UrlDropZone.svelte` — wraps URL input with HTML5 drag-and-drop handlers (`dragover`, `drop`), validates dropped text is an http(s) URL, populates input on valid drop
- [x] 7.3 Implement `SaveTranscriptButton.svelte` — uses `@tauri-apps/plugin-dialog`'s `save()` to open native save dialog with default filename derived from URL
- [x] 7.4 Implement `KeyboardShortcuts.svelte` — registers global keydown listeners: `Cmd+Enter` → invoke transcribe, `Cmd+.` → invoke cancel, `Cmd+S` → open save dialog
- [x] 7.5 Implement `SystemMenu.svelte` — uses Tauri's `Menu` API to register File menu items (New Transcription, Save Transcript, Quit) on mount
- [x] 7.6 Implement `DesktopTitleBar.svelte` — subscribes to phase changes and calls `getCurrentWindow().setTitle()` with the appropriate title
- [x] 7.7 Compose desktop components into `+page.svelte`: wrap URL input in `UrlDropZone`, add `SaveTranscriptButton` next to Copy, mount `KeyboardShortcuts` and `SystemMenu` in `onMount`

## 8. PyInstaller Sidecar Build

- [x] 8.1 Create `scripts/build_sidecar.py` — PyInstaller spec bundling `api/sidecar.py` + `transcribe_core.py`
- [x] 8.2 Add `--collect-all` flags: `yt_dlp`, `faster_whisper`, `av`, `huggingface_hub`, `tokenizers` (needed for hidden imports)
- [x] 8.3 Add `--onefile` and `--name transcribe-sidecar` flags
- [ ] 8.4 Build the sidecar binary for current macOS arch and copy to `src-tauri/binaries/transcribe-sidecar-aarch64-apple-darwin` (or `x86_64-apple-darwin`)
- [ ] 8.5 Test the PyInstaller binary standalone: `echo '{"url":"...","model":"tiny","timestamps":true}' | ./transcribe-sidecar-aarch64-apple-darwin`

## 9. Build & E2E Test

- [x] 9.1 Run `npx tauri build` on macOS and verify a `.app` bundle is produced in `src-tauri/target/release/bundle/macos/`
- [x] 9.2 Open the built `.app`, paste a YouTube URL, run a full end-to-end transcription test (verify phases stream, transcript displays in all three tabs)
- [ ] 9.3 Test cancellation: start a long transcription, click Stop, verify UI returns to idle within 2 seconds
- [ ] 9.4 Test single-flight: start a transcription, paste a new URL and click Transcribe, verify previous is cancelled
- [ ] 9.5 Test drag-and-drop: drag a YouTube URL onto the drop zone, verify input is populated
- [ ] 9.6 Test save dialog: complete a transcription, click Save Transcript, verify native dialog opens and file is written
- [ ] 9.7 Test keyboard shortcuts: `Cmd+Enter` triggers, `Cmd+.` cancels, `Cmd+S` saves
- [ ] 9.8 Test first-run model download: delete cached model, run transcription, confirm `downloading-model` phase shows progress and model downloads successfully
- [ ] 9.9 Test FFmpeg missing: rename `ffmpeg` on PATH temporarily, run app, verify friendly error message

## 10. Cleanup & Documentation

- [x] 10.1 Add `npm run tauri:dev` and `npm run tauri:build` scripts to `package.json`
- [x] 10.2 Update root `README.md` with desktop app build/run instructions, FFmpeg install requirement, and Gatekeeper workaround (`xattr -dr com.apple.quarantine`)
- [x] 10.3 Add a note in `deploy/` README that the Vultr deployment is now a secondary/reference path
- [x] 10.4 Document v1 limitations: macOS-only, unsigned, FFmpeg required, no auto-update