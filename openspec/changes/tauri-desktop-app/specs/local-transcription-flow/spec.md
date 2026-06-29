## ADDED Requirements

### Requirement: Transcription via Tauri invoke
The frontend SHALL initiate transcription by calling Tauri's `invoke('transcribe', { url, model, timestamps })` instead of `fetch('/api/transcribe/stream')`.

#### Scenario: User submits a URL
- **WHEN** the user enters a URL and clicks the Transcribe button (or presses `Cmd+Enter`)
- **THEN** the frontend SHALL call `invoke('transcribe', ...)` with the URL, model, and timestamps flag, and enter the loading state

#### Scenario: Invoke returns result
- **WHEN** the Tauri invoke resolves with a result object
- **THEN** the frontend SHALL display the transcript in the active tab (plain, timestamped, or SRT) and exit the loading state

### Requirement: Progress events via Tauri listen
The frontend SHALL subscribe to `transcribe-progress` events emitted by the Rust sidecar bridge to update the UI phase indicator and stream transcription segments in real time.

#### Scenario: Downloading phase
- **WHEN** the frontend receives a `transcribe-progress` event with `phase: "downloading"`
- **THEN** the UI SHALL show a "Downloading audio…" indicator

#### Scenario: Model download phase with progress
- **WHEN** the frontend receives a `transcribe-progress` event with `phase: "downloading-model"` and `progress: <float>`
- **THEN** the UI SHALL show "Downloading speech model (one-time)…" with a progress bar reflecting the `progress` value

#### Scenario: Transcribing phase with streaming segments
- **WHEN** the frontend receives `transcribe-progress` events with `phase: "transcribing"` and segment text
- **THEN** the UI SHALL display each segment with the typewriter animation as it arrives

### Requirement: Cancellation support
The frontend SHALL support cancelling an in-flight transcription via a `cancel-transcribe` Tauri command, triggered by a Stop button, `Cmd+.` keyboard shortcut, or system menu item.

#### Scenario: User clicks Stop button
- **WHEN** a transcription is in flight and the user clicks the Stop button
- **THEN** the frontend SHALL call `invoke('cancel-transcribe')` and return to the idle state within 2 seconds

#### Scenario: User presses Cmd+.
- **WHEN** a transcription is in flight and the user presses `Cmd+.` (or `Ctrl+.`)
- **THEN** the frontend SHALL call `invoke('cancel-transcribe')` and return to the idle state within 2 seconds

#### Scenario: Cancellation when idle
- **WHEN** no transcription is in flight and the user clicks Stop or presses `Cmd+.`
- **THEN** nothing SHALL happen (no error, no state change)

### Requirement: Single-flight concurrency policy
The frontend SHALL enforce single-flight semantics: if a transcription is already in flight when the user initiates another, the previous one SHALL be cancelled and the new one SHALL start.

#### Scenario: User clicks Transcribe twice
- **WHEN** the user clicks Transcribe while a transcription is already in flight
- **THEN** the previous transcription SHALL be cancelled (via `invoke('cancel-transcribe')`) and a new transcription SHALL start with the new URL

#### Scenario: Disabled button during transcription
- **WHEN** a transcription is in flight
- **THEN** the Transcribe button SHALL be visually disabled (but the Stop button SHALL remain enabled)

### Requirement: Structured error handling
The frontend SHALL map the sidecar's error codes to actionable, user-friendly messages.

#### Scenario: BOT_CHALLENGE error
- **WHEN** the frontend receives a `transcribe-progress` event with `code: "BOT_CHALLENGE"`
- **THEN** the UI SHALL show: "YouTube is blocking this video. Try a different one or open it in your browser first."

#### Scenario: FFMPEG_MISSING error
- **WHEN** the frontend receives a `transcribe-progress` event with `code: "FFMPEG_MISSING"`
- **THEN** the UI SHALL show: "FFmpeg is required. Install with `brew install ffmpeg` (macOS) and restart the app."

#### Scenario: NETWORK error
- **WHEN** the frontend receives a `transcribe-progress` event with `code: "NETWORK"`
- **THEN** the UI SHALL show: "Network error. Check your connection and try again."

#### Scenario: Generic error fallback
- **WHEN** the frontend receives a `transcribe-progress` event with an unknown error code
- **THEN** the UI SHALL show the raw `message` field prefixed with "Something went wrong: "

#### Scenario: Invoke rejection
- **WHEN** the Tauri `invoke('transcribe', ...)` promise rejects (e.g., Rust layer killed previous process)
- **THEN** the UI SHALL show a generic "Transcription failed. Please try again." message and reset the loading state

### Requirement: Copy and export functionality
The frontend SHALL retain the existing copy-to-clipboard and tab-switching (plain / timestamped / SRT) functionality, operating on the result returned by the sidecar.

#### Scenario: Copy transcript
- **WHEN** the user clicks the Copy button after transcription completes
- **THEN** the active tab's content (plain, timestamped, or SRT) SHALL be copied to the clipboard

#### Scenario: Switch output format tabs
- **WHEN** the user clicks between Plain Text, Timestamped, and SRT tabs
- **THEN** the displayed content SHALL switch to the corresponding format from the sidecar result

### Requirement: No remote server dependency
The frontend SHALL NOT make any HTTP requests to a remote server for transcription. All transcription requests SHALL go through the local Tauri sidecar.

#### Scenario: No fetch to /api
- **WHEN** the user initiates transcription
- **THEN** no `fetch('/api/transcribe/stream')` or similar remote call SHALL be made; only Tauri `invoke` is used

### Requirement: Tauri environment detection
The frontend SHALL detect whether it is running inside Tauri and degrade gracefully if not (browser dev mode).

#### Scenario: Running in browser
- **WHEN** the app loads in a browser (no `window.__TAURI_INTERNALS__` or `window.__TAURI__`)
- **THEN** the Transcribe button SHALL be disabled with a tooltip: "This feature requires the desktop app"

#### Scenario: Running in Tauri
- **WHEN** the app loads inside the Tauri webview
- **THEN** the Transcribe button SHALL be enabled and the desktop UI layer SHALL render