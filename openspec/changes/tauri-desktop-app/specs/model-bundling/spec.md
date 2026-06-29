## ADDED Requirements

### Requirement: Lazy model download on first run
The system SHALL download the `tiny` Whisper model (~75 MB) via `huggingface_hub.snapshot_download` on the first transcription request if the model is not already cached locally, and cache it in a user-data directory for subsequent runs.

#### Scenario: First transcription triggers model download
- **WHEN** the user initiates transcription for the first time and no cached model exists
- **THEN** the sidecar SHALL call `huggingface_hub.snapshot_download` with progress callbacks, emit `downloading-model` phase events with progress, cache the model locally, and proceed to transcription

#### Scenario: Subsequent transcriptions use cached model
- **WHEN** the user initiates transcription and the `tiny` model is already cached
- **THEN** the sidecar SHALL load the cached model directly without downloading or emitting `downloading-model` events

### Requirement: Model cache location
The system SHALL store downloaded model weights in a platform-appropriate user-data directory.

#### Scenario: Model cached on macOS
- **WHEN** the model is downloaded on macOS
- **THEN** it SHALL be stored under `~/Library/Caches/transcribe-app/models/tiny/`

### Requirement: Two-step model resolution
The sidecar SHALL use a two-step model resolution flow:
1. Check if the model exists in the user-data cache; if so, load it directly.
2. If not, call `huggingface_hub.snapshot_download` with progress callbacks, then load from the now-cached path.

#### Scenario: Model found in cache
- **WHEN** the sidecar starts and the model exists at the cached path
- **THEN** it SHALL skip snapshot_download and load directly from the local path

#### Scenario: Model not in cache
- **WHEN** the sidecar starts and no cached model exists
- **THEN** it SHALL call snapshot_download with progress callbacks, then load from the local path returned by snapshot_download

### Requirement: Single model in v1
The system SHALL support only the `tiny` model in v1. The model selection SHALL be fixed in the frontend (no user-facing model picker) to keep the app size small and the UX simple.

#### Scenario: Model is fixed to tiny
- **WHEN** the frontend sends a transcription request
- **THEN** the `model` field SHALL always be `"tiny"` and no model picker UI SHALL be shown

#### Scenario: Invalid model rejected
- **WHEN** the sidecar receives a request with a model other than `"tiny"`
- **THEN** it SHALL emit an error event indicating that only the `tiny` model is supported in v1