## Why

The Tauri desktop app persists every settings change, transcript, and batch job to its own JSON file under the OS app-data directory. Three small files is fine for a single-user local tool — but the moment we want to add any of: FTS search across transcripts, atomic writes that survive mid-write crashes, indexed lookups by id, or a tamper-resistant license store, JSON-on-disk stops scaling. SQLite gives us all of those in a single embedded dependency that costs zero ongoing ops and ships inside the binary.

This change is the foundation for the licensing work planned next — license state and signed usage counters need to live somewhere the user can't trivially edit, and SQLite is the right substrate. Doing the storage migration now while there is no licensing code yet means the licensing change can land on a clean schema instead of retrofitting one.

## What Changes

- Add `rusqlite` (bundled) + `chrono` to `src-tauri/Cargo.toml`. Remove the now-unused `humantime` dependency.
- Add a new `src-tauri/src/db.rs` module that owns schema, migrations, and all SQL helpers for settings, history, jobs, and job items.
- Replace the three per-file JSON stores (`settings.json`, `history.json`, `jobs.json`) with a single `app.db` SQLite database under the same `app_data_dir`.
- Add a one-time JSON → SQLite migration that runs on first launch: imports existing records, stamps a flag in `meta`, then deletes the legacy JSON files. Idempotent on subsequent launches.
- Rewrite every existing Tauri storage command (`load_settings`, `save_settings`, `load_history`, `save_transcript`, `delete_transcript`, `clear_history`, `load_jobs`, `save_job`, `delete_job`, `retry_job_item`) as a thin wrapper over a `db::*` function. The command surface and the JSON shapes returned to the frontend are unchanged — no frontend work is required.
- Switch from full-file reads/writes to per-row INSERT/UPDATE/DELETE, with WAL mode for crash-safe writes.
- Move record-count caps (500 transcripts, 200 jobs) from in-memory `Vec::truncate` into SQL `DELETE … WHERE id NOT IN (SELECT … LIMIT N)` after every write.
- Add a unit test module in `db.rs` covering schema creation, settings/history/jobs round-trips, retry semantics, and error paths.

## Capabilities

### New Capabilities

- `local-data-store`: Persistent storage for app settings, completed transcripts, and batch run records. SQLite-backed, with a one-time JSON migration for existing installs. Same query surface as the previous JSON files so no frontend change is required.

### Modified Capabilities

None. The Tauri command surface (`load_settings`, `save_history`, etc.) and the returned shapes are unchanged, so no spec-level behavior changes for any existing capability.

## Impact

- **Code**:
  - `src-tauri/src/lib.rs`: storage commands shrink from ~250 lines to ~50 (thin wrappers); all schema and SQL logic moves to `db.rs`.
  - `src-tauri/src/db.rs`: new file, ~700 lines (schema, migrations, CRUD helpers, tests).
- **Dependencies**: `rusqlite = "0.31"` (bundled, no system SQLite required), `chrono = "0.4"`. Removes `humantime`.
- **Frontend**: zero changes. Same `invoke('load_history')` / `save_transcript` / etc. surface.
- **Disk**: First-launch `app.db` is ~50 KB empty; a typical install with 50 transcripts + 10 jobs is ~2–5 MB. WAL files (`app.db-wal`, `app.db-shm`) live alongside it.
- **Migration**: One-time import on first launch after upgrade. Old `*.json` files are deleted after a successful import. If the import fails for any reason the JSON files are left in place and the next launch retries.
- **Performance**: `load_history` and `load_jobs` change from "deserialize the whole file" to "ordered SQL scan over an indexed column" — faster on large datasets. Writes become per-row and atomic.
- **Verification**: 6 new unit tests in `db::tests` exercise the schema, round-trips for all three stores, the retry path, and the unknown-id error paths. All passing.