## Context

The Tauri desktop app persists three logical stores — settings, completed transcripts, and batch run records — as three separate JSON files under `app_data_dir`. Every read deserializes the whole file; every write re-serializes the whole file. This worked for the MVP because the dataset is small (≤500 transcripts, ≤200 jobs, single user) and there is no concurrent-write contention. It does not scale to the next set of features the project wants:

- **FTS search** across transcript text — would require either loading every transcript into memory or building a side index.
- **Atomic writes** — a crash mid-write leaves the file truncated, and the read path returns `HistoryStore::default()` (i.e., silently empties the store) on parse failure.
- **Tamper-resistant state** for licensing — JSON files are user-editable; SQLite + a few derived helpers give us a defensible foundation.
- **Indexed lookups by id** — currently O(n) scans inside the `Vec`.

This change migrates the storage layer to SQLite (`rusqlite`, bundled). The Tauri command surface and the JSON shapes the frontend receives are preserved exactly, so no frontend code changes.

The implementation lives in a new module `src-tauri/src/db.rs`. All SQL, schema, and migration code lives there; `lib.rs` becomes a thin layer that delegates each storage command to the corresponding `db::*` function. This keeps `lib.rs` focused on the Tauri runtime concerns (sidecar spawn, cancellation, plugins).

## Goals / Non-Goals

**Goals:**
- Move all persistent app data (settings, transcripts, batch jobs) into a single SQLite database.
- Preserve the existing Tauri command surface and the JSON shapes returned to the frontend, so the frontend requires no changes.
- Run a one-time, idempotent JSON → SQLite migration on first launch that imports any legacy `*.json` files and then deletes them.
- Keep the implementation in a dedicated module (`db.rs`) so the licensing work planned next has a clean seam to extend.
- Pass `cargo check`, `cargo build`, `cargo test --lib`, and `npx vite build` with no regressions.
- Provide unit tests for the pure-SQL helpers (schema creation, round-trips, retry semantics, error paths).

**Non-Goals:**
- No encryption at rest. The licensing work will add an encrypted license table; encryption of the existing tables is out of scope here.
- No full-text search. The licensing change will need a few small schema additions; FTS is a separate change that can land on top of this one when there's user demand.
- No online backup / sync. The data is local-only, same as before.
- No multi-process / multi-instance support. The Tauri app is single-process; WAL gives us crash-safety, not concurrency.
- No automatic repair of a corrupt DB. A `PRAGMA integrity_check` failure would surface as a command error and the UI would treat it as "no data", same behavior as today's `unwrap_or_default()` on a malformed JSON file.

## Decisions

### D1: `rusqlite` with the `bundled` feature

**Choice**: Use `rusqlite = { version = "0.31", features = ["bundled"] }`.

**Rationale**: Building SQLite from C source means the app has zero external runtime dependencies. No `brew install sqlite`, no `apt-get install libsqlite3-dev`, no Windows redistributable. The bundle cost is roughly 1 MB on top of the Tauri binary — negligible for an app that already ships a 150 MB Python sidecar.

**Alternatives considered**:
- `sqlx` over `rusqlite`: nice async story, but adds the dependency on a connection pool and async SQL macros. The Tauri commands we have today are short-lived and synchronous from the Rust side, so we don't get much from going async.
- System SQLite via the `rusqlite/system-sqlite` path: smaller binary, but requires every user's machine to have a compatible libsqlite3. Skipped.

### D2: Per-row writes inside an explicit transaction for job saves

**Choice**: `save_job` opens an `unchecked_transaction`, upserts the parent `jobs` row, deletes the old `job_items` rows for that job_id, and inserts the new ones in a single transaction. Individual settings/history commands use single-statement inserts.

**Rationale**: `save_job` replaces the whole job (including all items) atomically — the frontend sends the complete record on every save. Doing it in a transaction means a partial failure rolls everything back, so we never end up with a job pointing at stale items. `save_settings` and `save_transcript` are single-statement and don't need explicit transactions.

### D3: WAL mode + `synchronous = NORMAL`

**Choice**: `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;` on every connection.

**Rationale**: WAL gives us crash-safe writes (the alternative is "rollback journal" which means readers block writers). `synchronous = NORMAL` is the SQLite-recommended pairing with WAL: durable across application crashes, and only loses up to one checkpoint's worth of writes on OS-level power loss (acceptable for a desktop app). `foreign_keys = ON` enforces the `job_items.job_id → jobs.id ON DELETE CASCADE` constraint. `busy_timeout = 5000` lets SQLite retry for 5s if a transient lock is held.

### D4: Cap enforcement in SQL, not Rust

**Choice**: After every insert into `history` or `jobs`, run `DELETE … WHERE id NOT IN (SELECT id … ORDER BY created_at DESC LIMIT N)`.

**Rationale**: Keeps the Rust code from ever loading the whole table into memory just to enforce a cap. The query is a constant cost indexed by `created_at` and bounded by `N`. The trim runs inside the same connection, so there's no read/write race window.

**Alternatives considered**:
- `Vec::truncate` after a full read: the original approach. Was O(n) memory and required the in-memory representation to be the source of truth. The SQL approach is faster, scales to larger caps, and is the natural way to do it.

### D5: `meta` table for migration tracking

**Choice**: A small `meta` table holds `key → value` rows for `schema_version` and `migrated_from_json`.

**Rationale**: SQLite's built-in `PRAGMA user_version` could store a single integer, but a key/value table is more flexible for future migrations and is cheap. `migrate_from_json_if_needed` is a no-op on every launch after the first successful migration.

### D6: Frontend is unchanged

**Choice**: Every Tauri command name and its `Result<T, String>` shape stays exactly as before. The frontend `invoke('load_history')`, `invoke('save_transcript', ...)`, etc. continue to work without any change.

**Rationale**: This is the lowest-risk way to swap storage — the existing UI continues to validate the migration. If anything were subtly wrong with the new code path, the UI would surface it immediately.

### D7: Unit tests in the same module as the production code

**Choice**: A `#[cfg(test)] mod tests` block at the bottom of `db.rs` exercises the pure-SQL helpers.

**Rationale**: The Tauri-dependent `migrate_from_json_if_needed` is hard to test without an `AppHandle`, so it's covered by the manual launch path on a real install. Everything else (schema creation, settings/history/jobs CRUD, retry semantics, error paths) is straightforward to test in isolation. Run with `cargo test --lib`.

## Risks / Trade-offs

- **WAL file accumulation** → SQLite auto-checkpoints, but a crash mid-write can leave `app.db-wal` on disk. Mitigation: WAL is small (kilobytes for typical writes) and is deleted/merged on the next clean shutdown. If a user inspects their app-data dir and sees the file, that's expected.
- **`migrate_from_json_if_needed` running on every launch** → The `meta` flag makes it a single SQL query that returns immediately. Negligible cost (~µs). If the migration fails halfway (corrupt JSON, disk full), the next launch retries because the flag wasn't stamped.
- **Loss of human-readable on-disk format** → JSON files were easy to grep. The new `app.db` requires `sqlite3` CLI to inspect. Mitigation: `sqlite3 app.db ".schema"` works out of the box on macOS. We can also add a `dump_db` debug command later if this becomes friction.
- **No atomic schema migration path** → The current `CREATE TABLE IF NOT EXISTS` only handles initial creation. Future schema changes will need real `ALTER TABLE` migrations and a `schema_version` bump. The `meta` table is already in place for that.
- **`rusqlite::Connection` is not `Send`** → Tauri commands are `async fn` but the SQL work is synchronous and fast. If a command were to take more than ~100ms it would block the runtime. Acceptable for the current data sizes; if it becomes a problem, we move to `tokio::task::spawn_blocking`.

## Migration Plan

This change ships inside the app binary, so users receive the new code automatically on next launch:

1. First launch with the new binary: `migrate_from_json_if_needed` detects the old JSON files, imports them into the new tables, stamps `migrated_from_json = 1` in `meta`, and deletes the JSON files.
2. Every subsequent launch: the meta flag is present, migration is a no-op.
3. Rollback: if the new binary must be reverted for any reason, the rollback path needs to be considered. The JSON files are deleted on successful migration, so going back to the old binary would show empty stores. Mitigation: keep the JSON files as `*.json.bak` for one version cycle before deleting them — added in a follow-up if needed.

## Open Questions

- **Should we keep `*.json.bak` files for one release as a rollback safety net?** The proposal says "delete the JSON files after successful import." A safety net of `*.json.bak` for 30 days would protect against a regression that surfaces later. Recommended for the first release of this change; can be removed once we're confident.
- **Should the migration also be exposed as an explicit user-triggered command?** Useful for support: "Export → Import" workflows, or recovering from a deleted DB. Out of scope here; can be added if needed.
- **WAL file retention on shutdown.** SQLite auto-checkpoints, but we could force a `PRAGMA wal_checkpoint(TRUNCATE)` on app exit to keep the data dir tidy. Defer.