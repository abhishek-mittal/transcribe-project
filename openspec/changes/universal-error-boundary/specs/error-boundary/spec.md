## ADDED Requirements

### Requirement: Error boundary wraps all app content
The system SHALL provide a top-level `<ErrorBoundary>` Svelte component rendered in `src/routes/+layout.svelte` that wraps the entire SvelteKit page slot. When any JavaScript error propagates uncaught to the `window.onerror` or `window.onunhandledrejection` handlers registered by this component, the boundary SHALL intercept it.

#### Scenario: Uncaught error triggers boundary
- **WHEN** any uncaught JavaScript error occurs anywhere in the app (render, lifecycle, event handler, or async callback)
- **THEN** the ErrorBoundary component catches it, records it internally, and switches to error-display mode

### Requirement: Error overlay is shown immediately
When in error-display mode the system SHALL replace the normal app content with a fullscreen overlay containing:
- A "Something went wrong" heading.
- A one-line description: "An unexpected error occurred. The app will close automatically."
- The error message string (for developer visibility).
- A visible countdown or indication that the app is closing.

#### Scenario: Overlay appears on crash
- **WHEN** an uncaught error is caught by the boundary
- **THEN** the overlay is rendered within one render frame and covers the entire window

#### Scenario: Overlay is styled for dark and light modes
- **WHEN** the error overlay is displayed
- **THEN** it uses a neutral dark background with white text so it is readable regardless of the app's theme setting

### Requirement: App force-quits within 2 seconds of catching an error
After catching an error the system SHALL call the Tauri `exit(1)` API after a delay of no more than 2000 ms, terminating the process entirely.

#### Scenario: Process exits after delay
- **WHEN** an uncaught error is caught by the boundary
- **THEN** `exit(1)` is called within 2000 ms and the OS-level process terminates

#### Scenario: Exit fallback when Tauri API unavailable
- **WHEN** the Tauri process plugin import fails (e.g., running in a browser during development)
- **THEN** `window.close()` is called as a fallback and an error is printed to the browser console
