## ADDED Requirements

### Requirement: Python sidecar entry point
The system SHALL provide a Python entry point (`api/sidecar.py`) that reads a single JSON request from stdin, processes the transcription, and writes newline-delimited JSON events to stdout.

#### Scenario: Valid request received
- **WHEN** the sidecar receives `{"url": "https://youtube.com/watch?v=...", "model": "tiny", "timestamps": true}` on stdin
- **THEN** it SHALL emit phase events (`downloading-model` if needed, `downloading`, `transcribing`), progress events with segment text, a result event with the full transcript, and a `done` event

#### Scenario: Invalid URL
- **WHEN** the sidecar receives a request with an empty URL, a URL longer than 2048 characters, or a URL that does not start with `http://` or `https://`
- **THEN** it SHALL emit an `error` event with code `INVALID_URL` and exit with a non-zero status code

#### Scenario: Unsupported model
- **WHEN** the sidecar receives a request with `model` other than `"tiny"`
- **THEN** it SHALL emit an `error` event with code `INVALID_URL` (or a new `UNSUPPORTED_MODEL` code) and exit

### Requirement: JSON-lines event protocol
The sidecar SHALL communicate with the Tauri layer using a JSON-lines protocol over stdio. Each line on stdout SHALL be a valid JSON object with an `event` field.

#### Scenario: Phase event
- **WHEN** the sidecar transitions between phases
- **THEN** it SHALL emit `{"event": "phase", "phase": "downloading"}` or `{"event": "phase", "phase": "transcribing"}`

#### Scenario: Model download phase with progress
- **WHEN** the Whisper model is being downloaded for the first time
- **THEN** the sidecar SHALL emit `{"event": "phase", "phase": "downloading-model", "progress": <float 0.0-1.0>}` periodically (every ~500ms or on each file chunk) until download completes

#### Scenario: Progress event
- **WHEN** a transcription segment is produced by faster-whisper
- **THEN** the sidecar SHALL emit `{"event": "progress", "phase": "transcribing", "segment": <int>, "text": "<segment text>"}`

#### Scenario: Result event
- **WHEN** transcription completes successfully
- **THEN** the sidecar SHALL emit `{"event": "result", "language": "<lang>", "plain": "...", "timestamped": "...", "srt": "..."}` followed by `{"event": "done"}`

#### Scenario: Error event with structured code
- **WHEN** any error occurs during download or transcription
- **THEN** the sidecar SHALL emit `{"event": "error", "code": "<ERROR_CODE>", "message": "<description>"}` and exit

### Requirement: Structured error codes
The sidecar SHALL classify errors into structured codes so the UI can show actionable messages. The code SHALL be one of:
- `INVALID_URL` — empty, too long, or wrong scheme
- `NETWORK` — DNS or connectivity failure
- `BOT_CHALLENGE` — yt-dlp error message contains "Sign in to confirm" or similar bot-detection phrases
- `UNSUPPORTED_PLATFORM` — yt-dlp has no extractor for the URL
- `FFMPEG_MISSING` — `shutil.which("ffmpeg")` returns None
- `MODEL_LOAD_FAILED` — faster-whisper or huggingface_hub raised during model load
- `INTERNAL` — any other unexpected exception

#### Scenario: Bot challenge classified
- **WHEN** yt-dlp raises a `DownloadError` whose message contains "Sign in to confirm you're not a bot"
- **THEN** the sidecar SHALL emit `{"event": "error", "code": "BOT_CHALLENGE", "message": "YouTube is blocking this video. Try a different one or sign in to YouTube in your browser."}`

#### Scenario: FFmpeg missing
- **WHEN** the sidecar starts and `shutil.which("ffmpeg")` returns None
- **THEN** the sidecar SHALL emit `{"event": "error", "code": "FFMPEG_MISSING", "message": "FFmpeg is required. Install with `brew install ffmpeg` (macOS) or see https://ffmpeg.org/download.html"}` and exit immediately

### Requirement: Core logic extraction
The system SHALL extract `download_audio`, `transcribe_audio`, formatting helpers, and model management from `api/transcribe.py` into `api/transcribe_core.py` as a pure module with no Flask dependency. The Flask app SHALL import from `transcribe_core.py` to maintain backward compatibility.

#### Scenario: Flask app still works
- **WHEN** the Flask API is run via `scripts/dev_api.py`
- **THEN** it SHALL import `download_audio` and `transcribe_audio` from `transcribe_core.py` and function identically to before the refactor

#### Scenario: Sidecar imports core module
- **WHEN** the sidecar processes a transcription request
- **THEN** it SHALL import `download_audio` and `transcribe_audio` from `transcribe_core.py` without importing Flask

### Requirement: No server-side anti-blocking workarounds in sidecar
The sidecar SHALL NOT apply cookies, PO-token plugins, player-client fallbacks, or proxy configuration when running in desktop mode, as requests originate from the user's residential IP.

#### Scenario: YouTube URL processed without workarounds
- **WHEN** the sidecar downloads audio from a YouTube URL
- **THEN** yt-dlp SHALL be invoked with default options (no `cookiefile`, no `extractor_args`, no `proxy`) and rely on the user's residential IP for access

### Requirement: Model download progress
The sidecar SHALL use `huggingface_hub.snapshot_download` to pre-download the model before loading, emitting `downloading-model` phase events with progress updates so the UI can show a progress bar.

#### Scenario: First-run model download
- **WHEN** the sidecar starts and no cached model exists
- **THEN** it SHALL call `huggingface_hub.snapshot_download` with a progress callback that emits `{"event": "phase", "phase": "downloading-model", "progress": <float>}` periodically, then load the model from the local path

#### Scenario: Cached model load
- **WHEN** the sidecar starts and the model is already cached locally
- **THEN** it SHALL skip the snapshot_download call and load the model directly (no `downloading-model` events emitted)

### Requirement: Cancellation via SIGTERM
The sidecar SHALL handle SIGTERM by cleaning up the temp directory and exiting with status code 130.

#### Scenario: User cancels mid-download
- **WHEN** the sidecar is downloading audio and receives SIGTERM
- **THEN** it SHALL delete the temp directory, emit no further events, and exit with status 130 within 2 seconds

#### Scenario: User cancels mid-transcription
- **WHEN** the sidecar is transcribing and receives SIGTERM
- **THEN** it SHALL stop the faster-whisper iterator, delete the temp directory, emit no further events, and exit with status 130 within 2 seconds

### Requirement: Temp file cleanup
The sidecar SHALL create audio files in a temporary directory and delete them after transcription completes, regardless of success, failure, or cancellation.

#### Scenario: Successful transcription cleanup
- **WHEN** transcription completes successfully
- **THEN** the temporary directory and downloaded audio file SHALL be deleted before the `done` event is emitted

#### Scenario: Error cleanup
- **WHEN** an error occurs during download or transcription
- **THEN** the temporary directory SHALL be deleted before the `error` event is emitted and the process exits

#### Scenario: SIGTERM cleanup
- **WHEN** the sidecar receives SIGTERM
- **THEN** the temporary directory SHALL be deleted in the signal handler before exit