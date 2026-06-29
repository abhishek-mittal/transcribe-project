## Why

The Tauri desktop app becomes silently unresponsive when a runtime JavaScript error (e.g., clicking the Queue tab) crashes the SvelteKit renderer — the window stays open with no error message, no log, and no way to recover, making the failure invisible to users and developers alike.

## What Changes

- Add a Svelte `<ErrorBoundary>` wrapper at the app root that catches any uncaught render/lifecycle error.
- On error: log the full stack trace to the Tauri sidecar log file (`~/Library/Logs/com.shuhari.transcribe/sidecar.log`) via a new `log_error` Tauri command, then call `process.exit(1)` (or the Tauri `exit` API) to force-quit the app within ~2 seconds.
- Add a `window.onerror` and `window.onunhandledrejection` handler as a backstop for errors that escape Svelte's boundary (async code, event handlers outside components).
- Show a minimal "Something went wrong — the app will close" overlay for the brief period before the process exits.

## Capabilities

### New Capabilities

- `error-boundary`: Top-level Svelte error boundary component that catches render/lifecycle errors, logs them, and exits the app.
- `global-error-handlers`: `window.onerror` + `window.onunhandledrejection` backstop registered in `onMount` that logs and exits for errors outside the Svelte component tree.
- `log-error-command`: New Rust Tauri command `log_error(message: String)` that appends a structured entry to the existing sidecar log file with timestamp and severity.

### Modified Capabilities

<!-- No existing spec-level behavior changes. -->

## Impact

- **`src/routes/+layout.svelte`** (or `+page.svelte` root if no layout exists): wraps content in the `<ErrorBoundary>` component.
- **`src/lib/desktop/ErrorBoundary.svelte`**: new component.
- **`src-tauri/src/lib.rs`**: new `log_error` command added to the invoke handler.
- **No breaking API changes** — all existing commands and event channels are unchanged.
- **Dependencies**: no new npm/Cargo packages required; uses Tauri's existing `app.path().app_log_dir()` and `process::exit`.
