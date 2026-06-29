## 1. Dependencies

- [x] 1.1 Add `rusqlite = { version = "0.31", features = ["bundled"] }` to `src-tauri/Cargo.toml`
- [x] 1.2 Add `chrono = "0.4"` to `src-tauri/Cargo.toml` (used for the migration's date formatting)
- [x] 1.3 Remove the now-unused `humantime` dependency from `src-tauri/Cargo.toml`
- [x] 1.4 Verify `cargo check` resolves cleanly with the new dependencies

## 2. Database module (`src-tauri/src/db.rs`)

- [x] 2.1 Create `src-tauri/src/db.rs` with module-level imports (`chrono`, `rusqlite`, `serde`, `tauri`)
- [x] 2.2 Implement `db_path(app)` — resolves `<app_data_dir>/app.db`, creating the parent directory if missing
- [x] 2.3 Implement `open(path)` — opens the connection and applies pragmas (`journal_mode = WAL`, `synchronous = NORMAL`, `foreign_keys = ON`, `busy_timeout = 5000`)
- [x] 2.4 Implement `run_migrations(conn)` — `CREATE TABLE IF NOT EXISTS` for `settings`, `history`, `jobs`, `job_items`, `meta` + `CREATE INDEX IF NOT EXISTS` for the three user indexes, and stamp `schema_version` in `meta`
- [x] 2.5 Implement `load_settings(conn)` / `save_settings(conn, settings)` — single-row upsert into the `settings` table
- [x] 2.6 Implement `save_transcript(conn, record)` — insert + `trim_history` to cap at `MAX_HISTORY_RECORDS`; generates uuid + rfc3339 timestamp server-side
- [x] 2.7 Implement `load_history(conn)` — ordered scan by `created_at DESC, id DESC`
- [x] 2.8 Implement `delete_transcript(conn, id)` and `clear_history(conn)`
- [x] 2.9 Implement `save_job(conn, job)` inside an `unchecked_transaction` — upsert parent + delete-then-insert items, then `trim_jobs` to cap at `MAX_JOB_RECORDS`
- [x] 2.10 Implement `load_jobs(conn)` — load jobs + per-job items in stable `idx` order
- [x] 2.11 Implement `delete_job(conn, job_id)` and `retry_job_item(conn, job_id, item_id)` — retry resets status + error/timing fields, preserves stable id/url
- [x] 2.12 Implement `migrate_from_json_if_needed(app, db_path, conn)` — detects existing JSON files via the `migrated_from_json` flag, imports any present files, then deletes them

## 3. Wire-up in `src-tauri/src/lib.rs`

- [x] 3.1 Add `mod db;` to the top of `lib.rs`
- [x] 3.2 Remove `const HISTORY_FILE`, `const SETTINGS_FILE`, `const JOBS_FILE` and their helper functions (`history_path`, `read_history_store`, `write_history_store`, `settings_path`, `jobs_path`, `read_job_store`, `write_job_store`)
- [x] 3.3 Remove the `HistoryStore` and `JobStore` structs (no longer needed at this layer — they live as private migration types in `db.rs`)
- [x] 3.4 Remove the `now_iso8601` helper (replaced by inline `chrono::Utc::now()` in `db::save_transcript`)
- [x] 3.5 Add `with_db(app)` helper that resolves `db_path`, opens the connection, runs migrations, and runs the JSON migration
- [x] 3.6 Rewrite the 10 storage commands as thin wrappers over `db::*` — same command names, same `Result<T, String>` shapes, no changes to `invoke_handler!`
- [x] 3.7 Keep `MAX_HISTORY_RECORDS` and `MAX_JOB_RECORDS` as `pub` constants so `db.rs` can use them

## 4. Verification

- [x] 4.1 `cargo check` passes with zero warnings
- [x] 4.2 `cargo build` succeeds (library links cleanly)
- [x] 4.3 `cargo test --lib` — 6/6 tests pass: schema creation, settings round-trip, history save/load/delete, history trim, job save/load/retry/delete, retry unknown item errors
- [x] 4.4 `npx vite build` succeeds (frontend unaffected, no source changes outside `src-tauri/`)
- [x] 4.5 Manual smoke: confirm the JSON → SQLite migration runs on first launch by placing sample JSON files in a temp app-data dir and observing imports in the logs