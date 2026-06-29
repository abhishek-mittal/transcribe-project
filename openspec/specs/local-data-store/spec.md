# local-data-store Specification

## Purpose
TBD - created by archiving change sqlite-local-store. Update Purpose after archive.
## Requirements
### Requirement: Local SQLite store under app-data directory
The Tauri Rust layer MUST persist all app data (settings, transcripts, batch jobs) in a single SQLite database file located at `<app_data_dir>/app.db`, where `app_data_dir` is the OS-appropriate Tauri app-data directory for the identifier `com.shuhari.transcribe`.

#### Scenario: Default install on macOS
- **WHEN** the app launches on macOS for the first time after this change ships
- **THEN** the file `~/Library/Application Support/com.shuhari.transcribe/app.db` is created (if it does not already exist), and is the sole location of persisted state. No `settings.json`, `history.json`, or `jobs.json` files are created or written.

#### Scenario: Default install on Linux
- **WHEN** the app launches on Linux for the first time
- **THEN** the file `~/.local/share/com.shuhari.transcribe/app.db` is created and used as the sole persistence location.

#### Scenario: Default install on Windows
- **WHEN** the app launches on Windows for the first time
- **THEN** the file under `%APPDATA%\com.shuhari.transcribe\app.db` is created and used as the sole persistence location.

### Requirement: One-time JSON import on first launch
The Tauri Rust layer MUST, on the first launch after this change ships, detect the legacy `settings.json`, `history.json`, and `jobs.json` files in `app_data_dir` and import their contents into the corresponding SQLite tables. After a successful import, the legacy JSON files MUST be deleted, and a `migrated_from_json = 1` flag MUST be written to the `meta` table.

#### Scenario: Existing user upgrades the app
- **WHEN** a user with previously-saved `settings.json`, `history.json`, and `jobs.json` files launches the new binary
- **THEN** the records from those files are inserted into `settings`, `history`, and `jobs` / `job_items`; the JSON files are removed; and the `load_history` / `load_jobs` / `load_settings` commands return the same data they would have returned from the JSON files.

#### Scenario: Brand-new install
- **WHEN** a user installs the new binary with no prior app data
- **THEN** no migration runs (no JSON files exist) and the database starts empty.

#### Scenario: Subsequent launches
- **WHEN** the user launches the app a second time
- **THEN** the migration is a no-op (detected by the `migrated_from_json` flag) and the JSON files are not re-checked.

#### Scenario: Migration leaves data intact on failure
- **WHEN** migration fails partway (e.g., one of the JSON files is malformed)
- **THEN** the legacy JSON files are NOT deleted, the `migrated_from_json` flag is NOT stamped, and the next launch retries the import.

### Requirement: Preserved Tauri command surface
The Tauri commands `load_settings`, `save_settings`, `load_history`, `save_transcript`, `delete_transcript`, `clear_history`, `load_jobs`, `save_job`, `delete_job`, and `retry_job_item` MUST continue to exist with the same names and the same `Result<T, String>` return shapes as before this change. The frontend MUST NOT require any changes.

#### Scenario: Frontend invoke calls
- **WHEN** the frontend calls `invoke('load_history', {})`
- **THEN** it receives the same array of `TranscriptRecord` objects (id, url, title, language, plain, timestamped, srt, model, word_count, created_at) as it did before this change, in the same newest-first order.

#### Scenario: Round-trip of new fields
- **WHEN** the frontend saves a job item that includes the `download_percent`, `downloaded_bytes`, and `total_bytes` fields added in earlier changes
- **THEN** a subsequent `load_jobs` returns those same values intact, with no data loss or reordering.

### Requirement: Bounded retention enforced in SQL
The Rust layer MUST keep at most 500 history records and at most 200 batch-job records, deleting the oldest excess records immediately after every insert. Enforcement MUST happen in SQL at the storage layer, not in Rust code that loads the full table into memory.

#### Scenario: Trim after insert
- **WHEN** the 501st history record is saved
- **THEN** the oldest record (lowest `created_at`) is deleted as part of the same operation, and a subsequent `load_history` returns at most 500 records.

#### Scenario: Trim after job save
- **WHEN** the 201st job is saved
- **THEN** the oldest job and all of its `job_items` (via `ON DELETE CASCADE`) are deleted as part of the same operation.

### Requirement: Crash-safe writes
Every write to the database MUST use SQLite's WAL journal mode and a `synchronous = NORMAL` pragma so that the database remains consistent across application crashes.

#### Scenario: Crash mid-write
- **WHEN** the application process is killed while a `save_transcript` command is mid-flight
- **THEN** on next launch, the database is in a consistent state — either the new record is present or it is not, but no partial row or corrupt index is left behind.

### Requirement: Foreign-key cascade for jobs
Deleting a job MUST automatically delete all of its `job_items` rows. This MUST be enforced by the database via a `FOREIGN KEY … ON DELETE CASCADE` constraint with `PRAGMA foreign_keys = ON`.

#### Scenario: Delete job removes its items
- **WHEN** `delete_job(job_id)` is called for a job that has 3 items
- **THEN** the job row is deleted and all 3 `job_items` rows for that `job_id` are deleted by the cascade. No orphaned `job_items` rows remain.

### Requirement: Idempotent schema creation
Calling the migration function on an existing database MUST be a no-op — no tables are dropped, no data is lost, and the schema version recorded in `meta` is unchanged.

#### Scenario: Restart launches the migration
- **WHEN** the app launches a second time and `run_migrations` is called
- **THEN** every `CREATE TABLE` / `CREATE INDEX` statement uses `IF NOT EXISTS`, no data is dropped, and the existing records remain queryable.

### Requirement: Storage commands return errors as strings
The Rust storage commands MUST return errors as `String` (via `Result<T, String>`), matching the Tauri convention used by the rest of the codebase. The frontend already treats a string error as a soft failure ("treat as empty defaults") and MUST continue to do so without changes.

#### Scenario: Corrupt database
- **WHEN** the `app.db` file exists but fails to open (e.g., truncated by a power loss)
- **THEN** the storage commands return an `Err(String)` to the frontend, and the frontend's existing fallback behavior (treat as empty) keeps the UI functional.

### Requirement: Unit-test coverage for pure-SQL helpers
The `db` module MUST include unit tests covering: schema creation (5 tables, 3 user indexes), settings round-trip preserving unrelated fields, history save/load/delete, job save/load/retry/delete round-trip including the new `download_percent` / `downloaded_bytes` / `total_bytes` fields, retry of an unknown item-id returning an error, and history trim query execution.

#### Scenario: cargo test passes
- **WHEN** `cargo test --lib` is run
- **THEN** all tests in `db::tests` pass with no failures or ignored tests.

