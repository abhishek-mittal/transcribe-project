## ADDED Requirements

### Requirement: log_error Tauri command appends to sidecar log
The system SHALL expose a `log_error(message: String)` Tauri command that opens `~/Library/Logs/com.shuhari.transcribe/sidecar.log` in append mode (creating it and its parent directory if they do not exist) and writes one line with the format:

```
[CRASH 2026-06-27T22:33:47Z] <message>
```

where the timestamp is the current UTC time in ISO-8601 format.

#### Scenario: Log entry written on crash
- **WHEN** `log_error` is invoked with a non-empty message string
- **THEN** a single line is appended to the sidecar log file with a `[CRASH ...]` prefix and the message

#### Scenario: Log directory is created if missing
- **WHEN** `log_error` is invoked and the log directory does not exist
- **THEN** the directory is created before the file is opened, and the write succeeds

#### Scenario: Command succeeds even if message is very long
- **WHEN** `log_error` is invoked with a message longer than 10,000 characters (e.g., a full stack trace)
- **THEN** the full message is written without truncation

### Requirement: log_error is registered in the Tauri invoke handler
The `log_error` command SHALL be listed in `tauri::generate_handler![]` so it is callable from the frontend via `invoke('log_error', { message })`.

#### Scenario: Frontend can invoke log_error
- **WHEN** JavaScript calls `invoke('log_error', { message: 'test error' })`
- **THEN** the Rust handler is executed and the log entry is appended within the same process tick

### Requirement: log_error never panics or crashes the process
The `log_error` command SHALL handle all I/O errors gracefully by logging them to `eprintln!` and returning `Ok(())`, so a logging failure never causes a secondary crash.

#### Scenario: Write fails silently
- **WHEN** the log file cannot be written (e.g., disk full or permissions error)
- **THEN** the error is printed to stderr and `log_error` returns `Ok(())` without panicking
