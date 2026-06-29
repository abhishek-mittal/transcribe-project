## ADDED Requirements

### Requirement: window.onerror handler is registered on mount
The `<ErrorBoundary>` component SHALL register a `window.onerror` handler during `onMount` that captures the error message, source, line, column, and Error object, then triggers the error-display and exit flow.

#### Scenario: Synchronous uncaught error is caught
- **WHEN** a synchronous JavaScript error propagates to `window.onerror`
- **THEN** the handler captures the message and error object, calls `log_error`, displays the overlay, and schedules `exit(1)`

#### Scenario: Handler returns true to suppress default browser error dialog
- **WHEN** the `window.onerror` handler fires
- **THEN** it returns `true` to prevent the WebView from showing its own error UI

### Requirement: window.onunhandledrejection handler is registered on mount
The `<ErrorBoundary>` component SHALL register a `window.onunhandledrejection` handler during `onMount` that captures unhandled Promise rejections and triggers the same error-display and exit flow.

#### Scenario: Unhandled Promise rejection is caught
- **WHEN** a Promise rejects and no `.catch()` or `try/await/catch` handles it
- **THEN** `window.onunhandledrejection` fires, the boundary logs the rejection reason, shows the overlay, and exits

### Requirement: Handlers are removed on component destroy
The `<ErrorBoundary>` component SHALL remove the `window.onerror` and `window.onunhandledrejection` handlers in `onDestroy` to prevent leaks if the component is ever unmounted during development.

#### Scenario: Handlers cleaned up on destroy
- **WHEN** the `<ErrorBoundary>` component is destroyed (e.g., during HMR in dev mode)
- **THEN** both window handlers are set back to null or the previous value
