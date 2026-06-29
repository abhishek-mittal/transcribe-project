## Context

The Tauri desktop app renders a SvelteKit single-page app inside a WebView. When a JavaScript error propagates uncaught — during a Svelte component's render or lifecycle, inside an async event handler, or in a reactive statement — the WebView's JS engine terminates the failing task. Svelte 4 does not include a built-in error boundary, so there is no existing catch point. The window stays open but frozen with no user feedback and nothing in the logs. Developers must kill the process from Terminal/Activity Monitor with no diagnostic information.

A `+layout.svelte` does not yet exist. All app UI is rendered from `src/routes/+page.svelte`. The existing `src/lib/desktop/errors.ts` handles sidecar error-code mapping but has nothing to do with runtime JS errors.

## Goals / Non-Goals

**Goals:**
- Catch all uncaught Svelte render/lifecycle errors via a top-level error boundary wrapper.
- Catch all other unhandled JS errors and promise rejections via `window.onerror` / `window.onunhandledrejection`.
- On any such catch: append a structured entry (timestamp, error message, stack) to `~/Library/Logs/com.shuhari.transcribe/sidecar.log` via a new `log_error` Rust command.
- Force-quit the Tauri process within 2 seconds after logging so the app never stays open and unresponsive.
- Show a brief "Something went wrong — the app will close" message in the UI during those 2 seconds.

**Non-Goals:**
- Recoverable error handling or retry — the goal is fail-fast, not resilience.
- Source-map-based stack deobfuscation in the log.
- A separate log file — reuse the existing `sidecar.log` path.
- Sending crash reports to a remote service.

## Decisions

### D1 — Svelte error boundary via `<svelte:boundary>` (Svelte 5) or manual try/catch wrapper (Svelte 4)

The project uses Svelte 4 (SvelteKit with `@sveltejs/kit`). Svelte 4 has no `<svelte:boundary>` primitive. The standard pattern is to wrap child components in a parent that uses an `{#if hasError}` branch and catches errors via the `on:error` synthetic event dispatched from children — but that only works if children explicitly throw into it.

**Decision**: Use a thin `<ErrorBoundary>` Svelte component that registers `window.onerror` and `window.onunhandledrejection` in `onMount`. This catches all top-level JS errors regardless of where in the component tree they originate. The component renders a fullscreen error overlay when triggered and invokes the exit flow.

**Rationale**: Svelte 4 provides no native boundary; the window-level handlers are the idiomatic fallback and cover 100% of error paths (sync render errors bubble to the global handler in a browser/WebView).

**Alternative considered**: Wrapping `+page.svelte` imports in a try/catch — rejected because reactive statements and async callbacks can't be caught this way.

### D2 — Force-quit via Tauri `exit` plugin command

Tauri 2 exposes `@tauri-apps/plugin-process` → `exit(code)` which calls `std::process::exit(code)` on the Rust side. This is already available in the project's Tauri plugin surface.

**Decision**: Call `import('@tauri-apps/plugin-process').then(m => m.exit(1))` after a 2-second delay. As a fallback (if the import fails), call `window.close()`.

**Alternative considered**: A new `force_quit` Rust command — unnecessary since the process plugin already provides this.

### D3 — Log via new `log_error` Rust command

The existing `sidecar.log` file is already opened/appended in `run_sidecar`. A new `log_error(message: String)` command will open the same file (append mode) and write a single timestamped line. This keeps all crash-related information in one place.

**Decision**: Add `log_error` as a new `#[tauri::command]` in `src-tauri/src/lib.rs`. It reuses `app.path().app_log_dir()` to resolve the path and appends a line with format: `[CRASH YYYY-MM-DDTHH:MM:SSZ] <message>`.

**Alternative considered**: Writing directly from JS via `@tauri-apps/plugin-fs` — rejected because the log directory may need to be created and the path resolution is already handled in Rust.

### D4 — Placement: `+layout.svelte` at the route root

A new `src/routes/+layout.svelte` wraps `<slot />` (i.e., `+page.svelte`) inside `<ErrorBoundary>`. This is the canonical SvelteKit extension point and avoids modifying the large `+page.svelte`.

## Risks / Trade-offs

- **[Risk] Some async errors may not reach `window.onerror`** (e.g., errors swallowed inside Tauri invoke callbacks that have their own try/catch). → Mitigation: document that callers should not silently swallow errors; the boundary is a last resort, not a substitute for local error handling.
- **[Risk] 2-second delay before exit feels abrupt** → Mitigation: the overlay message is shown immediately; users see feedback. The delay is intentional to give the `log_error` invoke time to flush.
- **[Risk] `log_error` invoke itself could fail** (app already in bad state) → Mitigation: wrap in try/catch; proceed to exit regardless. Use `console.error` as secondary log channel.

## Migration Plan

1. Implement `log_error` Rust command and register it (no breaking changes to existing commands).
2. Create `src/lib/desktop/ErrorBoundary.svelte`.
3. Create `src/routes/+layout.svelte` that wraps content in `<ErrorBoundary>`.
4. Rebuild Tauri dev (`npm run tauri dev`).
5. Test by triggering the known crash (Queue tab click) and verifying: overlay appears, log entry written, app exits within 2 seconds.
6. No rollback complexity — these are additive changes; removing the layout file reverts the boundary.
