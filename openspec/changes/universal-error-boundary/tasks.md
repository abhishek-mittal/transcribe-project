## 1. Rust — log_error command

- [x] 1.1 Add `log_error(app: AppHandle, message: String) -> Result<(), String>` command to `src-tauri/src/lib.rs` that resolves `app_log_dir()`, creates the directory if missing, and appends a `[CRASH <ISO-timestamp>Z] <message>` line to `sidecar.log`
- [x] 1.2 Register `log_error` in `tauri::generate_handler![]` alongside the existing commands
- [x] 1.3 Verify `cargo build` succeeds with no new warnings

## 2. Svelte — ErrorBoundary component

- [x] 2.1 Create `src/lib/desktop/ErrorBoundary.svelte` with `onMount`/`onDestroy` hooks that register and unregister `window.onerror` and `window.onunhandledrejection`
- [x] 2.2 Add internal `hasError`, `errorMessage` state; on any caught error call `invoke('log_error', { message })`, set `hasError = true`, and schedule `import('@tauri-apps/plugin-process').then(m => m.exit(1))` after 2000 ms with `window.close()` as fallback
- [x] 2.3 Render the error overlay (`hasError` branch): fullscreen dark panel, "Something went wrong — the app will close." heading, one-line error message, no interactive elements required
- [x] 2.4 Render the normal slot (`!hasError` branch): `<slot />`

## 3. SvelteKit — layout wrapper

- [x] 3.1 Create `src/routes/+layout.svelte` that imports `ErrorBoundary` and wraps `<slot />` inside it
- [x] 3.2 Confirm the existing `+layout.js` (SSR/prerender flags) still applies with the new layout file and no HMR errors appear in dev

## 4. Verification

- [ ] 4.1 Run `npm run dev` (or `npm run tauri dev`) and click the Queue tab — confirm the error overlay appears and the process exits within 2 seconds
- [ ] 4.2 Confirm a `[CRASH ...]` line appears in `~/Library/Logs/com.shuhari.transcribe/sidecar.log`
- [ ] 4.3 Verify that normal app usage (URL input, transcription, history) is unaffected — the boundary must be transparent when no error occurs
