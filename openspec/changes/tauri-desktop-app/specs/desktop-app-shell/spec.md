## ADDED Requirements

### Requirement: Tauri application shell
The system SHALL provide a Tauri 2.x desktop application that wraps the existing SvelteKit frontend as its webview UI, producing a distributable `.app` bundle for macOS in v1.

#### Scenario: App launches and shows the transcription UI
- **WHEN** the user opens the installed desktop application
- **THEN** the Tauri window opens displaying the SvelteKit transcription interface (URL input, transcribe button, result tabs)

#### Scenario: App window configuration
- **WHEN** the application starts
- **THEN** the window SHALL have a minimum size of 800×600, a descriptive title (e.g., "Transcribe"), and be resizable

### Requirement: SvelteKit static build for Tauri
The system SHALL build the SvelteKit frontend using `@sveltejs/adapter-static` with SPA fallback so the output is a static bundle loadable by Tauri's webview without a Node server.

#### Scenario: Frontend builds to static files
- **WHEN** `tauri build` is executed
- **THEN** the SvelteKit frontend is compiled to static HTML/JS/CSS in the Tauri expected directory and loaded by the webview

#### Scenario: Existing adapter-node path preserved
- **WHEN** the Flask server deployment path is used (dormant)
- **THEN** `adapter-node` configuration remains available in the repo for reference without conflicting with the Tauri static build

### Requirement: Tauri capabilities and permissions
The system SHALL declare a `capabilities/default.json` that whitelists only the plugins and commands needed: `core:default`, `core:event:default`, `shell:allow-execute` (scoped to the sidecar binary), `dialog:allow-save`, `dialog:allow-open`, `fs:allow-write-file` (scoped to user-chosen paths).

#### Scenario: Sidecar can be spawned
- **WHEN** the frontend invokes the `transcribe` command
- **THEN** the Rust layer spawns the sidecar binary because `shell:allow-execute` is granted with the sidecar path in scope

#### Scenario: Save dialog works
- **WHEN** the user clicks Save Transcript
- **THEN** the native save dialog opens because `dialog:allow-save` is granted

### Requirement: Sidecar binary registration
The system SHALL register the PyInstaller-bundled Python sidecar binary as a Tauri external binary (sidecar) in `tauri.conf.json`, resolving the correct platform-specific binary at runtime.

#### Scenario: Sidecar is available at runtime
- **WHEN** the Tauri app invokes the transcription command on macOS
- **THEN** the sidecar binary is located and spawned with the correct platform-specific suffix (e.g., `transcribe-sidecar-aarch64-apple-darwin`)