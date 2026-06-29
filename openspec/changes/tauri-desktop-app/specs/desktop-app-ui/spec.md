## ADDED Requirements

### Requirement: Desktop-mode-aware UI
The system SHALL detect when it is running inside Tauri (vs. a browser) and render a desktop-native UI layer accordingly.

#### Scenario: App running in Tauri
- **WHEN** the SvelteKit app loads inside the Tauri webview
- **THEN** the UI SHALL render the desktop UI layer (`src/lib/desktop/` components): drag-and-drop URL zone, keyboard shortcut hints, Save Transcript button with native dialog, system menu items, and the desktop-mode-aware layout (e.g., wider spacing, larger click targets, native scrollbars)

#### Scenario: App running in a browser (development)
- **WHEN** the SvelteKit app loads in a regular browser (e.g., during `vite dev`)
- **THEN** the UI SHALL fall back to the original web-only layout without desktop-specific affordances, and SHALL NOT throw errors when Tauri APIs are unavailable

### Requirement: Drag-and-drop URL input
The system SHALL accept a video URL dropped onto the URL input area via HTML5 drag-and-drop, populating the input field with the dropped text.

#### Scenario: URL dragged onto the drop zone
- **WHEN** the user drags a text/URL from any source (browser address bar, another app, a text file) and drops it onto the URL input
- **THEN** the URL SHALL be extracted from the drag payload and populated into the input field, ready for transcription

#### Scenario: Non-URL text dropped
- **WHEN** the user drops non-URL text onto the drop zone
- **THEN** the text SHALL be rejected with a brief visual shake animation and no change to the input field

### Requirement: Keyboard shortcuts
The system SHALL support the following global keyboard shortcuts when the app is focused:
- `Cmd+Enter` (macOS) / `Ctrl+Enter` (other) — trigger transcription if the URL field is non-empty and the app is idle
- `Cmd+.` (macOS) / `Ctrl+.` (other) — cancel the in-flight transcription
- `Cmd+S` (macOS) / `Ctrl+S` (other) — open the Save Transcript dialog (only enabled when a result is available)

#### Scenario: Cmd+Enter triggers transcription
- **WHEN** the user has a URL in the input and presses `Cmd+Enter`
- **THEN** the transcription SHALL start as if the Transcribe button was clicked

#### Scenario: Cmd+. cancels
- **WHEN** the user presses `Cmd+.` during an in-flight transcription
- **THEN** the transcription SHALL be cancelled and the UI SHALL return to the idle state

#### Scenario: Cmd+S is disabled when no result
- **WHEN** no transcription result is available and the user presses `Cmd+S`
- **THEN** no save dialog SHALL appear

### Requirement: Native file save dialog
The system SHALL provide a Save Transcript button that opens the native macOS save dialog via `tauri-plugin-dialog`, allowing the user to save the active transcript (plain, timestamped, or SRT) to disk.

#### Scenario: Save dialog opens
- **WHEN** the user clicks the Save Transcript button
- **THEN** the native macOS save dialog SHALL open with a default filename derived from the source URL (e.g., the video slug or timestamp)

#### Scenario: User selects a path and saves
- **WHEN** the user selects a path in the save dialog and clicks Save
- **THEN** the active tab's content SHALL be written to the selected file with the appropriate extension (`.txt` for plain, `.txt` for timestamped, `.srt` for SRT)

#### Scenario: User cancels the dialog
- **WHEN** the user cancels the save dialog
- **THEN** no file SHALL be written and the UI SHALL remain unchanged

### Requirement: System menu integration
The system SHALL register native system menu items via Tauri's menu API, accessible from the macOS menu bar.

#### Scenario: System menu items present
- **WHEN** the app is running on macOS
- **THEN** the system menu bar SHALL show: App menu (About, Quit), File menu (New Transcription, Save Transcript), Edit menu (standard)

#### Scenario: Save Transcript menu item
- **WHEN** the user clicks File → Save Transcript in the system menu
- **THEN** the same native save dialog SHALL open as when clicking the Save Transcript button

### Requirement: Title bar handling
The system SHALL set the Tauri window title to reflect the current state of the app.

#### Scenario: Idle title
- **WHEN** the app is in the idle state
- **THEN** the window title SHALL be "Transcribe"

#### Scenario: Transcribing title
- **WHEN** a transcription is in flight
- **THEN** the window title SHALL be "Transcribe — Transcribing…" (or the current phase, e.g., "Downloading…")

#### Scenario: Done title
- **WHEN** a transcription completes
- **THEN** the window title SHALL briefly show "Transcribe — Done" for 3 seconds, then revert to "Transcribe"

### Requirement: Desktop-style layout
The system SHALL use a desktop-optimized layout when running in Tauri: wider minimum width (≥ 800px), larger click targets, native-style scrollbars, no mobile-first responsive breakpoints.

#### Scenario: Window resized
- **WHEN** the user resizes the window down to 800px wide
- **THEN** all controls SHALL remain accessible and the layout SHALL NOT collapse to a mobile-style stacked layout