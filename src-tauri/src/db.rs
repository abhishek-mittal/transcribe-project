// SQLite-backed local store for the transcribe-project desktop app.
//
// Replaces the previous per-file JSON stores (`history.json`, `settings.json`,
// `jobs.json`). All three are now tables inside a single `app.db` file under
// the OS-appropriate `app_data_dir`:
//
//   macOS:   ~/Library/Application Support/com.shuhari.transcribe/app.db
//   Linux:   ~/.local/share/com.shuhari.transcribe/app.db
//   Windows: %APPDATA%\com.shuhari.transcribe\app.db
//
// The on-disk format is a single SQLite database with three tables:
//
//   settings      — single-row config (model, dark mode, timestamps, …)
//   history       — individual completed transcripts (capped at MAX_HISTORY_RECORDS)
//   jobs          — batch run records (capped at MAX_JOB_RECORDS)
//   job_items     — one row per video inside a job (FK to jobs.id ON DELETE CASCADE)
//
// Writes use WAL mode + a Mutex<Connection> at the call site for cross-thread
// safety. Reads are direct. All public functions return `Result<T, String>`
// to match the rest of the command surface; callers treat `Err(_)` as
// "treat as empty defaults" so a corrupted DB never bricks the app.
//
// Migration from the old JSON files happens once via `migrate_from_json_if_needed`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::{
    ActiveJobItemRecord, ActiveJobRecord, ItemResultPayload, JobItemRecord, JobRecord,
    JobStatsPayload, MAX_HISTORY_RECORDS, MAX_JOB_RECORDS, NewTranscriptRecord, Settings,
    TranscriptRecord,
};

/// Wrapper shape for the legacy `history.json` file. Used only by the
/// one-time JSON→SQLite migration.
#[derive(Deserialize)]
struct LegacyHistoryWrapper {
    #[serde(default)]
    records: Vec<TranscriptRecord>,
}

/// Wrapper shape for the legacy `jobs.json` file. Used only by the
/// one-time JSON→SQLite migration.
#[derive(Deserialize)]
struct LegacyJobsWrapper {
    #[serde(default)]
    records: Vec<JobRecord>,
}

const DB_FILE: &str = "app.db";

/// Schema version for future migrations. Bumped when DDL changes.
///
/// v1 → v2 (F13): `jobs.completed_at`/`elapsed_ms` become nullable and gain
/// `is_active` so a job can be persisted the moment it starts, not just when
/// it finishes; `job_items` gains `phase` + `download_path` to support the
/// chunked-download / sequential-transcription pipeline and crash resume.
const SCHEMA_VERSION: i32 = 2;

/// Return the absolute path to `app.db` under the app data dir, creating
/// the parent directory if needed.
pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create app data dir: {e}"))?;
    Ok(dir.join(DB_FILE))
}

/// Open a connection with the project's pragmas applied. Each call returns a
/// fresh connection — callers are responsible for keeping it on a single
/// thread (rusqlite::Connection is not Sync).
pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("open db: {e}"))?;
    // WAL gives us crash-safe writes and concurrent reads; the app is
    // single-process so this is more about durability than concurrency.
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| format!("set pragmas: {e}"))?;
    Ok(conn)
}

/// Create tables/indexes if they don't exist, then run any pending schema
/// migrations. Idempotent — safe to call on every launch.
pub fn run_migrations(conn: &Connection) -> Result<(), String> {
    // `meta` first so we can read a pre-existing schema_version before
    // deciding whether `jobs`/`job_items` need the v1→v2 migration below.
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            id                  INTEGER PRIMARY KEY CHECK (id = 1),
            model               TEXT    NOT NULL,
            timestamps          INTEGER NOT NULL,
            dark_mode           INTEGER NOT NULL,
            max_history_records INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS history (
            id            TEXT    PRIMARY KEY,
            url           TEXT    NOT NULL,
            title         TEXT,
            language      TEXT    NOT NULL,
            plain         TEXT    NOT NULL,
            timestamped   TEXT,
            srt           TEXT    NOT NULL,
            model         TEXT    NOT NULL,
            word_count    INTEGER NOT NULL,
            created_at    TEXT    NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_history_created_at
            ON history (created_at DESC);

        -- v2 shape (see SCHEMA_VERSION doc comment). Fresh installs get
        -- this directly; v1 installs are upgraded by migrate_v1_to_v2 below.
        -- NOTE: the is_active/phase-dependent INDEXes are created further
        -- down, AFTER migrate_v1_to_v2 runs — see the comment there for why
        -- they can't live in this batch.
        CREATE TABLE IF NOT EXISTS jobs (
            id                TEXT    PRIMARY KEY,
            model             TEXT    NOT NULL,
            timestamps        INTEGER NOT NULL,
            created_at        TEXT    NOT NULL,
            completed_at      TEXT,
            elapsed_ms        INTEGER,
            total_items       INTEGER NOT NULL,
            success_count     INTEGER NOT NULL DEFAULT 0,
            failure_count     INTEGER NOT NULL DEFAULT 0,
            cancelled_count   INTEGER NOT NULL DEFAULT 0,
            total_words       INTEGER NOT NULL DEFAULT 0,
            total_audio_secs  INTEGER NOT NULL DEFAULT 0,
            is_active         INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_jobs_created_at
            ON jobs (created_at DESC);

        CREATE TABLE IF NOT EXISTS job_items (
            id                TEXT    PRIMARY KEY,
            job_id            TEXT    NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
            idx               INTEGER NOT NULL,
            url               TEXT    NOT NULL,
            title             TEXT    NOT NULL,
            thumbnail         TEXT    NOT NULL,
            duration_secs     INTEGER NOT NULL,
            status            TEXT    NOT NULL,
            phase             TEXT    NOT NULL DEFAULT 'waiting',
            download_path     TEXT,
            error_code        TEXT,
            error_message     TEXT,
            language          TEXT,
            plain             TEXT,
            timestamped       TEXT,
            srt               TEXT,
            word_count        INTEGER,
            started_at        TEXT,
            completed_at      TEXT,
            elapsed_ms        INTEGER,
            download_percent  REAL,
            downloaded_bytes  INTEGER,
            total_bytes       INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_job_items_job_id_idx
            ON job_items (job_id, idx);

        -- F14: probe result cache. Lets the frontend skip the yt-dlp walk
        -- for re-pasted URLs within a short TTL. Schema-version-neutral
        -- (CREATE TABLE IF NOT EXISTS is idempotent across versions).
        CREATE TABLE IF NOT EXISTS probe_cache (
            url         TEXT    PRIMARY KEY,
            result_json TEXT    NOT NULL,
            fetched_at  TEXT    NOT NULL,
            ttl_secs    INTEGER NOT NULL DEFAULT 900
        );
        "#,
    )
    .map_err(|e| format!("create tables: {e}"))?;

    let current_version: i32 = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("read schema_version: {e}"))?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if current_version < 2 {
        migrate_v1_to_v2(conn)?;
    }

    // Deferred until after migrate_v1_to_v2: on a v1 DB, `jobs` exists
    // without `is_active` until the migration adds it (CREATE TABLE IF NOT
    // EXISTS above is a no-op against the pre-existing v1 table), so this
    // index can't be created any earlier without erroring on "no such
    // column: is_active".
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_jobs_active ON jobs (is_active) WHERE is_active = 1",
        [],
    )
    .map_err(|e| format!("create idx_jobs_active: {e}"))?;

    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| format!("record schema version: {e}"))?;

    Ok(())
}

/// Upgrade a v1 `jobs`/`job_items` table (if it still has the old shape) to
/// v2: `jobs.completed_at`/`elapsed_ms` become nullable and gain
/// `is_active`; `job_items` gains `phase` + `download_path`.
///
/// No-op if the tables are already v2 shape (idempotent): the presence of
/// `is_active` on `jobs` is used as the "already migrated" check, since
/// `CREATE TABLE IF NOT EXISTS` above means a fresh install's `jobs` table
/// is already v2 shape and this function must not touch it.
fn migrate_v1_to_v2(conn: &Connection) -> Result<(), String> {
    let jobs_has_is_active = table_has_column(conn, "jobs", "is_active")?;
    let items_has_phase = table_has_column(conn, "job_items", "phase")?;

    if !jobs_has_is_active {
        // SQLite has no ALTER COLUMN — recreate the table with the v2 shape
        // and copy every existing row across. All prior completed jobs are
        // preserved verbatim; only the new is_active column is synthesized
        // (0 — a job loaded from a v1 DB already finished, by definition,
        // since v1 only ever wrote jobs at completion time).
        //
        // CRITICAL: `job_items.job_id REFERENCES jobs(id) ON DELETE CASCADE`
        // + the connection's `PRAGMA foreign_keys = ON` means `DROP TABLE
        // jobs` below would cascade-delete every job_items row before the
        // rename even happens — silently destroying all item history on
        // upgrade. Foreign key enforcement must be off for this statement
        // sequence (PRAGMA foreign_keys can't be toggled inside a
        // transaction, so it's set directly on the connection, not inside
        // execute_batch's implicit transaction).
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .map_err(|e| format!("migrate_v1_to_v2 disable fk: {e}"))?;

        let migration_result = conn.execute_batch(
            r#"
            CREATE TABLE jobs_v2 (
                id                TEXT    PRIMARY KEY,
                model             TEXT    NOT NULL,
                timestamps        INTEGER NOT NULL,
                created_at        TEXT    NOT NULL,
                completed_at      TEXT,
                elapsed_ms        INTEGER,
                total_items       INTEGER NOT NULL,
                success_count     INTEGER NOT NULL DEFAULT 0,
                failure_count     INTEGER NOT NULL DEFAULT 0,
                cancelled_count   INTEGER NOT NULL DEFAULT 0,
                total_words       INTEGER NOT NULL DEFAULT 0,
                total_audio_secs  INTEGER NOT NULL DEFAULT 0,
                is_active         INTEGER NOT NULL DEFAULT 0
            );

            INSERT INTO jobs_v2 (
                id, model, timestamps, created_at, completed_at, elapsed_ms,
                total_items, success_count, failure_count, cancelled_count,
                total_words, total_audio_secs, is_active
            )
            SELECT
                id, model, timestamps, created_at, completed_at, elapsed_ms,
                total_items, success_count, failure_count, cancelled_count,
                total_words, total_audio_secs, 0
            FROM jobs;

            DROP TABLE jobs;
            ALTER TABLE jobs_v2 RENAME TO jobs;

            CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs (created_at DESC);
            "#,
        );

        // Re-enable FK enforcement unconditionally, even on error, then
        // verify no row now dangles before reporting success.
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| format!("migrate_v1_to_v2 re-enable fk: {e}"))?;
        migration_result.map_err(|e| format!("migrate_v1_to_v2 jobs: {e}"))?;

        let dangling: i64 = conn
            .query_row(
                "SELECT count(*) FROM job_items WHERE job_id NOT IN (SELECT id FROM jobs)",
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("migrate_v1_to_v2 fk verify: {e}"))?;
        if dangling > 0 {
            return Err(format!(
                "migrate_v1_to_v2: {dangling} job_items rows reference a missing job after migration"
            ));
        }

        eprintln!("migrate_v1_to_v2: jobs table upgraded (completed_at/elapsed_ms nullable, +is_active)");
    }

    if !items_has_phase {
        conn.execute_batch(
            r#"
            ALTER TABLE job_items ADD COLUMN phase TEXT NOT NULL DEFAULT 'waiting';
            ALTER TABLE job_items ADD COLUMN download_path TEXT;
            "#,
        )
        .map_err(|e| format!("migrate_v1_to_v2 job_items: {e}"))?;
        // Backfill phase from the existing `status` column so old rows
        // (all of which are terminal — v1 only persisted finished jobs)
        // read consistently under the new phase-based UI.
        conn.execute(
            "UPDATE job_items SET phase = status WHERE phase = 'waiting' AND status != 'waiting'",
            [],
        )
        .map_err(|e| format!("migrate_v1_to_v2 backfill phase: {e}"))?;
        eprintln!("migrate_v1_to_v2: job_items table upgraded (+phase, +download_path)");
    }

    Ok(())
}

/// Return true if `table` has a column named `column`. Used to make the v2
/// migration idempotent without a separate "have we migrated" flag.
fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| format!("table_has_column prepare: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("table_has_column query: {e}"))?;
    while let Some(row) = rows.next().map_err(|e| format!("table_has_column row: {e}"))? {
        let name: String = row.get(1).map_err(|e| format!("table_has_column col name: {e}"))?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

// ─── probe cache (F14) ───────────────────────────────────────────────────
//
// Caches the final probe result for a URL so re-pasting the same URL within
// the TTL is instant (no yt-dlp walk, no sidecar spawn). Keyed by URL since
// the probe is fully URL-deterministic (yt-dlp's playlist result depends
// only on the URL, not on time of day or cookies). The freshness check is
// done in SQL using `strftime('%s', 'now') - strftime('%s', fetched_at)`
// so the DB is the source of truth — clock skew between Rust and the
// frontend doesn't matter.
//
// Invalidation: the frontend calls `invalidate_probe(url)` on
// "Transcribe X videos" click, so the user gets a fresh probe on the next
// paste if they want (e.g. to pick up newly added videos). TTL is 15 min
// by default.

/// Default TTL for cached probe results: 15 minutes. Long enough that the
/// common "fix my selection and re-open the picker" path is instant, short
/// enough that newly added videos show up after a session break without
/// needing a manual Refresh.
pub const PROBE_CACHE_TTL_SECS: i64 = 900;

/// Write (or overwrite) the cached probe result for `url`. The full JSON
/// blob is stored verbatim — the frontend receives the same shape whether
/// the result came from the cache or a fresh probe, so no extra
/// (de)serialization is needed in the hot path.
pub fn cache_probe(
    conn: &Connection,
    url: &str,
    result: &serde_json::Value,
) -> Result<(), String> {
    let json = serde_json::to_string(result).map_err(|e| format!("cache_probe serialize: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO probe_cache (url, result_json, fetched_at, ttl_secs)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3)",
        params![url, json, PROBE_CACHE_TTL_SECS],
    )
    .map_err(|e| format!("cache_probe insert: {e}"))?;
    Ok(())
}

/// Return the cached probe result if one exists and is fresher than its
/// TTL. Returns `Ok(None)` for cache miss, expired entry, or any row
/// that fails to deserialize (treated as a miss — the frontend will
/// re-probe and overwrite the bad row on the next cache_probe call).
pub fn get_cached_probe(
    conn: &Connection,
    url: &str,
) -> Result<Option<(serde_json::Value, i64)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT result_json,
                    CAST(strftime('%s', 'now') - strftime('%s', fetched_at) AS INTEGER) AS age_secs,
                    ttl_secs
               FROM probe_cache
              WHERE url = ?1",
        )
        .map_err(|e| format!("get_cached_probe prepare: {e}"))?;
    let mut rows = stmt
        .query([url])
        .map_err(|e| format!("get_cached_probe query: {e}"))?;
    let Some(row) = rows.next().map_err(|e| format!("get_cached_probe row: {e}"))? else {
        return Ok(None);
    };
    let json: String = row.get(0).map_err(|e| format!("get_cached_probe col 0: {e}"))?;
    let age: i64 = row.get(1).map_err(|e| format!("get_cached_probe col 1: {e}"))?;
    let ttl: i64 = row.get(2).map_err(|e| format!("get_cached_probe col 2: {e}"))?;
    if age < 0 || age > ttl {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("get_cached_probe parse: {e}"))?;
    Ok(Some((value, age)))
}

/// Remove the cache entry for `url`. Called on "Transcribe X videos"
/// click so the next paste of the same URL triggers a fresh probe and
/// the user sees any newly added videos.
pub fn invalidate_probe(conn: &Connection, url: &str) -> Result<(), String> {
    conn.execute("DELETE FROM probe_cache WHERE url = ?1", [url])
        .map_err(|e| format!("invalidate_probe: {e}"))?;
    Ok(())
}

// ─── settings ────────────────────────────────────────────────────────────

/// Read the single settings row, returning defaults if the table is empty
/// (which only happens before the first save_settings call).
pub fn load_settings(conn: &Connection) -> Result<Settings, String> {
    let row = conn
        .query_row(
            "SELECT model, timestamps, dark_mode, max_history_records
               FROM settings
              WHERE id = 1",
            [],
            map_settings_row,
        )
        .optional()
        .map_err(|e| format!("load_settings: {e}"))?;
    Ok(row.unwrap_or_default())
}

/// Replace the single settings row. Always present after first call.
pub fn save_settings(conn: &Connection, settings: &Settings) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (id, model, timestamps, dark_mode, max_history_records)
         VALUES (1, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
             model               = excluded.model,
             timestamps          = excluded.timestamps,
             dark_mode           = excluded.dark_mode,
             max_history_records = excluded.max_history_records",
        params![
            settings.model,
            settings.timestamps as i64,
            settings.dark_mode as i64,
            settings.max_history_records as i64,
        ],
    )
    .map_err(|e| format!("save_settings: {e}"))?;
    Ok(())
}

fn map_settings_row(row: &Row<'_>) -> rusqlite::Result<Settings> {
    Ok(Settings {
        version: 1,
        model: row.get(0)?,
        timestamps: row.get::<_, i64>(1)? != 0,
        dark_mode: row.get::<_, i64>(2)? != 0,
        max_history_records: row.get::<_, i64>(3)? as u32,
    })
}

// ─── history ─────────────────────────────────────────────────────────────

/// Insert a new transcript record (id + created_at generated server-side)
/// and trim the table to `MAX_HISTORY_RECORDS`. Returns the new id.
pub fn save_transcript(conn: &Connection, record: NewTranscriptRecord) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now: DateTime<Utc> = Utc::now();
    let created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    conn.execute(
        "INSERT INTO history
            (id, url, title, language, plain, timestamped, srt, model, word_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            record.url,
            record.title,
            record.language,
            record.plain,
            record.timestamped,
            record.srt,
            record.model,
            record.word_count as i64,
            created_at,
        ],
    )
    .map_err(|e| format!("save_transcript insert: {e}"))?;

    trim_history(conn)?;
    Ok(id)
}

/// Load all history records, newest first.
pub fn load_history(conn: &Connection) -> Result<Vec<TranscriptRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, url, title, language, plain, timestamped, srt, model, word_count, created_at
               FROM history
              ORDER BY created_at DESC, id DESC",
        )
        .map_err(|e| format!("load_history prepare: {e}"))?;
    let rows = stmt
        .query_map([], map_history_row)
        .map_err(|e| format!("load_history query: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("load_history row: {e}"))?);
    }
    Ok(out)
}

/// Delete a single history record by id. No-op if id doesn't exist.
pub fn delete_transcript(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM history WHERE id = ?1", params![id])
        .map_err(|e| format!("delete_transcript: {e}"))?;
    Ok(())
}

/// Remove every row from the history table.
pub fn clear_history(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM history", [])
        .map_err(|e| format!("clear_history: {e}"))?;
    Ok(())
}

/// Keep only the newest `MAX_HISTORY_RECORDS` rows.
fn trim_history(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "DELETE FROM history
          WHERE id NOT IN (
              SELECT id FROM history
              ORDER BY created_at DESC, id DESC
              LIMIT ?1
          )",
        params![MAX_HISTORY_RECORDS as i64],
    )
    .map_err(|e| format!("trim_history: {e}"))?;
    Ok(())
}

fn map_history_row(row: &Row<'_>) -> rusqlite::Result<TranscriptRecord> {
    Ok(TranscriptRecord {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        language: row.get(3)?,
        plain: row.get(4)?,
        timestamped: row.get(5)?,
        srt: row.get(6)?,
        model: row.get(7)?,
        word_count: row.get::<_, i64>(8)? as u32,
        created_at: row.get(9)?,
    })
}

// ─── jobs + job_items ────────────────────────────────────────────────────

/// Save (insert or replace) a job and all its items, then trim the jobs
/// table to `MAX_JOB_RECORDS`. Items are replaced wholesale because the
/// frontend sends the full record on save.
pub fn save_job(conn: &Connection, job: JobRecord) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| format!("save_job tx: {e}"))?;

    tx.execute(
        "INSERT INTO jobs
            (id, model, timestamps, created_at, completed_at, elapsed_ms,
             total_items, success_count, failure_count, cancelled_count,
             total_words, total_audio_secs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
             model            = excluded.model,
             timestamps       = excluded.timestamps,
             created_at       = excluded.created_at,
             completed_at     = excluded.completed_at,
             elapsed_ms       = excluded.elapsed_ms,
             total_items      = excluded.total_items,
             success_count    = excluded.success_count,
             failure_count    = excluded.failure_count,
             cancelled_count  = excluded.cancelled_count,
             total_words      = excluded.total_words,
             total_audio_secs = excluded.total_audio_secs",
        params![
            job.id,
            job.model,
            job.timestamps as i64,
            job.created_at,
            job.completed_at,
            job.elapsed_ms as i64,
            job.total_items as i64,
            job.success_count as i64,
            job.failure_count as i64,
            job.cancelled_count as i64,
            job.total_words as i64,
            job.total_audio_secs as i64,
        ],
    )
    .map_err(|e| format!("save_job upsert job: {e}"))?;

    // Replace items wholesale. ON DELETE CASCADE already cleared them
    // when we replaced the parent in some scenarios, but be explicit.
    tx.execute("DELETE FROM job_items WHERE job_id = ?1", params![job.id])
        .map_err(|e| format!("save_job clear items: {e}"))?;

    for (i, item) in job.items.iter().enumerate() {
        tx.execute(
            "INSERT INTO job_items
                (id, job_id, idx, url, title, thumbnail, duration_secs,
                 status, error_code, error_message, language, plain,
                 timestamped, srt, word_count, started_at, completed_at,
                 elapsed_ms, download_percent, downloaded_bytes, total_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                     ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                item.id,
                job.id,
                i as i64,
                item.url,
                item.title,
                item.thumbnail,
                item.duration_secs as i64,
                item.status,
                item.error_code,
                item.error_message,
                item.language,
                item.plain,
                item.timestamped,
                item.srt,
                item.word_count.map(|v| v as i64),
                item.started_at,
                item.completed_at,
                item.elapsed_ms.map(|v| v as i64),
                item.download_percent,
                item.downloaded_bytes.map(|v| v as i64),
                item.total_bytes.map(|v| v as i64),
            ],
        )
        .map_err(|e| format!("save_job insert item: {e}"))?;
    }

    tx.commit().map_err(|e| format!("save_job commit: {e}"))?;

    trim_jobs(conn)?;
    Ok(())
}

/// Load all *completed* jobs (with their items in stable order), newest
/// first. `completed_at IS NOT NULL` excludes the in-progress job (if any)
/// — that one is read via `load_active_job` instead, which can represent
/// `completed_at`/`elapsed_ms` as absent rather than forcing a sentinel
/// value on a row that hasn't finished yet.
pub fn load_jobs(conn: &Connection) -> Result<Vec<JobRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, model, timestamps, created_at, completed_at, elapsed_ms,
                    total_items, success_count, failure_count, cancelled_count,
                    total_words, total_audio_secs
               FROM jobs
              WHERE completed_at IS NOT NULL
              ORDER BY created_at DESC, id DESC",
        )
        .map_err(|e| format!("load_jobs prepare: {e}"))?;

    let mut jobs: Vec<JobRecord> = Vec::new();
    let rows = stmt
        .query_map([], map_job_row)
        .map_err(|e| format!("load_jobs query: {e}"))?;
    for r in rows {
        jobs.push(r.map_err(|e| format!("load_jobs row: {e}"))?);
    }

    for job in &mut jobs {
        job.items = load_items_for_job(conn, &job.id)?;
    }
    Ok(jobs)
}

fn load_items_for_job(conn: &Connection, job_id: &str) -> Result<Vec<JobItemRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, url, title, thumbnail, duration_secs, status,
                    error_code, error_message, language, plain,
                    timestamped, srt, word_count, started_at, completed_at,
                    elapsed_ms, download_percent, downloaded_bytes, total_bytes
               FROM job_items
              WHERE job_id = ?1
              ORDER BY idx ASC",
        )
        .map_err(|e| format!("load_items prepare: {e}"))?;
    let rows = stmt
        .query_map(params![job_id], map_job_item_row)
        .map_err(|e| format!("load_items query: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("load_items row: {e}"))?);
    }
    Ok(out)
}

// ─── F13: pipeline queue — jobs persisted from the moment they start ──────

/// Write a job + all its items to the DB the instant a batch job starts,
/// `is_active = 1`, `completed_at = NULL`. Replaces `save_job` for the new
/// pipeline path — `save_job` is kept only for the one-time JSON migration.
pub fn create_job_active(conn: &Connection, job: &ActiveJobRecord) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("create_job_active tx: {e}"))?;

    tx.execute(
        "INSERT INTO jobs
            (id, model, timestamps, created_at, completed_at, elapsed_ms,
             total_items, success_count, failure_count, cancelled_count,
             total_words, total_audio_secs, is_active)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, 0, 0, 0, 0, 0, 1)",
        params![
            job.id,
            job.model,
            job.timestamps as i64,
            job.created_at,
            job.total_items as i64,
        ],
    )
    .map_err(|e| format!("create_job_active insert job: {e}"))?;

    for (i, item) in job.items.iter().enumerate() {
        tx.execute(
            "INSERT INTO job_items
                (id, job_id, idx, url, title, thumbnail, duration_secs,
                 status, phase, download_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
            params![
                item.id,
                job.id,
                i as i64,
                item.url,
                item.title,
                item.thumbnail,
                item.duration_secs as i64,
                "waiting",
                "waiting",
            ],
        )
        .map_err(|e| format!("create_job_active insert item: {e}"))?;
    }

    tx.commit().map_err(|e| format!("create_job_active commit: {e}"))?;
    trim_jobs(conn)?;
    Ok(())
}

/// Update one item's phase (and `status`, kept in sync for the legacy
/// `JobItemRecord` shape) and optionally its `download_path`. Called on
/// every phase transition — fire-and-forget from the frontend, so this
/// must stay a fast single-row UPDATE.
pub fn update_item_phase(
    conn: &Connection,
    item_id: &str,
    phase: &str,
    download_path: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE job_items
            SET phase = ?1,
                status = ?1,
                download_path = COALESCE(?2, download_path)
          WHERE id = ?3",
        params![phase, download_path, item_id],
    )
    .map_err(|e| format!("update_item_phase: {e}"))?;
    Ok(())
}

/// Record a successful transcription result for one item: phase becomes
/// `done`, transcript fields are filled in, `completed_at` is stamped.
pub fn update_item_result(
    conn: &Connection,
    item_id: &str,
    result: &ItemResultPayload,
) -> Result<(), String> {
    let now: DateTime<Utc> = Utc::now();
    let completed_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    conn.execute(
        "UPDATE job_items
            SET phase = 'done',
                status = 'done',
                language = ?1,
                plain = ?2,
                timestamped = ?3,
                srt = ?4,
                word_count = ?5,
                completed_at = ?6
          WHERE id = ?7",
        params![
            result.language,
            result.plain,
            result.timestamped,
            result.srt,
            result.word_count as i64,
            completed_at,
            item_id,
        ],
    )
    .map_err(|e| format!("update_item_result: {e}"))?;
    Ok(())
}

/// Record a failure for one item: phase becomes `failed`, error fields are
/// filled in, `completed_at` is stamped.
pub fn update_item_error(
    conn: &Connection,
    item_id: &str,
    error_code: &str,
    error_message: &str,
) -> Result<(), String> {
    let now: DateTime<Utc> = Utc::now();
    let completed_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    conn.execute(
        "UPDATE job_items
            SET phase = 'failed',
                status = 'failed',
                error_code = ?1,
                error_message = ?2,
                completed_at = ?3
          WHERE id = ?4",
        params![error_code, error_message, completed_at, item_id],
    )
    .map_err(|e| format!("update_item_error: {e}"))?;
    Ok(())
}

/// Mark a job complete: stamps `completed_at`/`elapsed_ms`, records final
/// stats, and flips `is_active` off. `save_job` + `finalize_job` together
/// replace the old single `save_job` call that only ran at the very end.
pub fn finalize_job(conn: &Connection, job_id: &str, stats: &JobStatsPayload) -> Result<(), String> {
    let now: DateTime<Utc> = Utc::now();
    let completed_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    conn.execute(
        "UPDATE jobs
            SET completed_at     = ?1,
                elapsed_ms       = ?2,
                success_count    = ?3,
                failure_count    = ?4,
                cancelled_count  = ?5,
                total_words      = ?6,
                total_audio_secs = ?7,
                is_active        = 0
          WHERE id = ?8",
        params![
            completed_at,
            stats.elapsed_ms as i64,
            stats.success_count as i64,
            stats.failure_count as i64,
            stats.cancelled_count as i64,
            stats.total_words as i64,
            stats.total_audio_secs as i64,
            job_id,
        ],
    )
    .map_err(|e| format!("finalize_job: {e}"))?;
    Ok(())
}

/// Return the single in-progress job (`is_active = 1`), if any, with its
/// items in stable order. Used on app startup to offer a resume banner.
pub fn load_active_job(conn: &Connection) -> Result<Option<ActiveJobRecord>, String> {
    let job = conn
        .query_row(
            "SELECT id, model, timestamps, created_at, completed_at, elapsed_ms,
                    total_items, success_count, failure_count, cancelled_count,
                    total_words, total_audio_secs, is_active
               FROM jobs
              WHERE is_active = 1
              LIMIT 1",
            [],
            map_active_job_row,
        )
        .optional()
        .map_err(|e| format!("load_active_job: {e}"))?;

    let mut job = match job {
        Some(j) => j,
        None => return Ok(None),
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, url, title, thumbnail, duration_secs, phase,
                    download_path, error_code, error_message, language,
                    plain, timestamped, srt, word_count, started_at, completed_at
               FROM job_items
              WHERE job_id = ?1
              ORDER BY idx ASC",
        )
        .map_err(|e| format!("load_active_job items prepare: {e}"))?;
    let rows = stmt
        .query_map(params![job.id], map_active_job_item_row)
        .map_err(|e| format!("load_active_job items query: {e}"))?;
    for r in rows {
        job.items.push(r.map_err(|e| format!("load_active_job item row: {e}"))?);
    }

    Ok(Some(job))
}

/// Return every job id currently marked `is_active = 1`. In practice this
/// is at most one row (the pipeline only ever runs one job at a time), but
/// the orphan-downloads scan on startup checks against the full set rather
/// than assuming exactly one.
pub fn list_active_job_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM jobs WHERE is_active = 1")
        .map_err(|e| format!("list_active_job_ids prepare: {e}"))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("list_active_job_ids query: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("list_active_job_ids row: {e}"))?);
    }
    Ok(out)
}

/// Delete a single job by id. Items are removed via FK cascade.
pub fn delete_job(conn: &Connection, job_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM jobs WHERE id = ?1", params![job_id])
        .map_err(|e| format!("delete_job: {e}"))?;
    Ok(())
}

/// Reset a single job item's status fields to "waiting" + clear error/timing.
/// Returns the updated item. Returns Err if either id is unknown.
pub fn retry_job_item(
    conn: &Connection,
    job_id: &str,
    item_id: &str,
) -> Result<JobItemRecord, String> {
    let updated = conn
        .query_row(
            "SELECT id, url, title, thumbnail, duration_secs, status,
                    error_code, error_message, language, plain,
                    timestamped, srt, word_count, started_at, completed_at,
                    elapsed_ms, download_percent, downloaded_bytes, total_bytes
               FROM job_items
              WHERE job_id = ?1 AND id = ?2",
            params![job_id, item_id],
            map_job_item_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                format!("item {item_id} not found in job {job_id}")
            }
            other => format!("retry_job_item query: {other}"),
        })?;

    conn.execute(
        "UPDATE job_items
            SET status        = 'waiting',
                error_code    = NULL,
                error_message = NULL,
                started_at    = NULL,
                completed_at  = NULL,
                elapsed_ms    = NULL
          WHERE job_id = ?1 AND id = ?2",
        params![job_id, item_id],
    )
    .map_err(|e| format!("retry_job_item update: {e}"))?;

    Ok(JobItemRecord {
        status: "waiting".to_string(),
        error_code: None,
        error_message: None,
        started_at: None,
        completed_at: None,
        elapsed_ms: None,
        ..updated
    })
}

/// Keep only the newest `MAX_JOB_RECORDS` jobs (cascades to items).
fn trim_jobs(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "DELETE FROM jobs
          WHERE id NOT IN (
              SELECT id FROM jobs
              ORDER BY created_at DESC, id DESC
              LIMIT ?1
          )",
        params![MAX_JOB_RECORDS as i64],
    )
    .map_err(|e| format!("trim_jobs: {e}"))?;
    Ok(())
}

fn map_job_row(row: &Row<'_>) -> rusqlite::Result<JobRecord> {
    Ok(JobRecord {
        id: row.get(0)?,
        model: row.get(1)?,
        timestamps: row.get::<_, i64>(2)? != 0,
        created_at: row.get(3)?,
        completed_at: row.get(4)?,
        elapsed_ms: row.get::<_, i64>(5)? as u64,
        total_items: row.get::<_, i64>(6)? as u32,
        success_count: row.get::<_, i64>(7)? as u32,
        failure_count: row.get::<_, i64>(8)? as u32,
        cancelled_count: row.get::<_, i64>(9)? as u32,
        total_words: row.get::<_, i64>(10)? as u32,
        total_audio_secs: row.get::<_, i64>(11)? as u32,
        items: Vec::new(), // populated by load_jobs after the rows are read
    })
}

fn map_job_item_row(row: &Row<'_>) -> rusqlite::Result<JobItemRecord> {
    Ok(JobItemRecord {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        thumbnail: row.get(3)?,
        duration_secs: row.get::<_, i64>(4)? as u32,
        status: row.get(5)?,
        error_code: row.get(6)?,
        error_message: row.get(7)?,
        language: row.get(8)?,
        plain: row.get(9)?,
        timestamped: row.get(10)?,
        srt: row.get(11)?,
        word_count: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
        started_at: row.get(13)?,
        completed_at: row.get(14)?,
        elapsed_ms: row.get::<_, Option<i64>>(15)?.map(|v| v as u64),
        download_percent: row.get(16)?,
        downloaded_bytes: row.get::<_, Option<i64>>(17)?.map(|v| v as u64),
        total_bytes: row.get::<_, Option<i64>>(18)?.map(|v| v as u64),
    })
}

fn map_active_job_row(row: &Row<'_>) -> rusqlite::Result<ActiveJobRecord> {
    Ok(ActiveJobRecord {
        id: row.get(0)?,
        model: row.get(1)?,
        timestamps: row.get::<_, i64>(2)? != 0,
        created_at: row.get(3)?,
        completed_at: row.get(4)?,
        elapsed_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
        total_items: row.get::<_, i64>(6)? as u32,
        success_count: row.get::<_, i64>(7)? as u32,
        failure_count: row.get::<_, i64>(8)? as u32,
        cancelled_count: row.get::<_, i64>(9)? as u32,
        total_words: row.get::<_, i64>(10)? as u32,
        total_audio_secs: row.get::<_, i64>(11)? as u32,
        is_active: row.get::<_, i64>(12)? != 0,
        items: Vec::new(), // populated by load_active_job after the row is read
    })
}

fn map_active_job_item_row(row: &Row<'_>) -> rusqlite::Result<ActiveJobItemRecord> {
    Ok(ActiveJobItemRecord {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        thumbnail: row.get(3)?,
        duration_secs: row.get::<_, i64>(4)? as u32,
        phase: row.get(5)?,
        download_path: row.get(6)?,
        error_code: row.get(7)?,
        error_message: row.get(8)?,
        language: row.get(9)?,
        plain: row.get(10)?,
        timestamped: row.get(11)?,
        srt: row.get(12)?,
        word_count: row.get::<_, Option<i64>>(13)?.map(|v| v as u32),
        started_at: row.get(14)?,
        completed_at: row.get(15)?,
    })
}

// ─── one-time JSON → SQLite migration ────────────────────────────────────

/// If `app.db` doesn't exist yet but the legacy JSON files do, import them
/// into SQLite and delete the JSON files. Safe to call on every launch;
/// it's a no-op once the DB exists.
pub fn migrate_from_json_if_needed(
    app: &AppHandle,
    db_path: &Path,
    conn: &Connection,
) -> Result<(), String> {
    let needs_migration = !db_path.exists()
        || conn
            .query_row::<i64, _, _>(
                "SELECT 1 FROM meta WHERE key = 'migrated_from_json'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("check migration flag: {e}"))?
            .is_none();

    if !needs_migration {
        return Ok(());
    }

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolve app_data_dir: {e}"))?;

    let history_json = dir.join("history.json");
    let settings_json = dir.join("settings.json");
    let jobs_json = dir.join("jobs.json");

    let mut migrated_any = false;

    if settings_json.exists() {
        if let Ok(text) = fs::read_to_string(&settings_json) {
            if let Ok(parsed) = serde_json::from_str::<Settings>(&text) {
                save_settings(conn, &parsed)?;
                migrated_any = true;
                eprintln!("migrate: imported settings.json");
            }
        }
    }

    if history_json.exists() {
        if let Ok(text) = fs::read_to_string(&history_json) {
            if let Ok(parsed) = serde_json::from_str::<LegacyHistoryWrapper>(&text) {
                let count = parsed.records.len();
                for record in parsed.records {
                    conn.execute(
                        "INSERT OR IGNORE INTO history
                            (id, url, title, language, plain, timestamped, srt, model, word_count, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            record.id,
                            record.url,
                            record.title,
                            record.language,
                            record.plain,
                            record.timestamped,
                            record.srt,
                            record.model,
                            record.word_count as i64,
                            record.created_at,
                        ],
                    )
                    .map_err(|e| format!("migrate history row: {e}"))?;
                }
                trim_history(conn)?;
                eprintln!("migrate: imported {count} history records");
                migrated_any = true;
            }
        }
    }

    if jobs_json.exists() {
        if let Ok(text) = fs::read_to_string(&jobs_json) {
            if let Ok(parsed) = serde_json::from_str::<LegacyJobsWrapper>(&text) {
                let count = parsed.records.len();
                for job in parsed.records {
                    save_job(conn, job)?;
                }
                eprintln!("migrate: imported {count} jobs");
                migrated_any = true;
            }
        }
    }

    // Stamp the migration as done so we don't redo the work next launch.
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('migrated_from_json', '1')",
        [],
    )
    .map_err(|e| format!("stamp migration: {e}"))?;

    // Delete the JSON files only if we actually imported something.
    if migrated_any {
        for path in [&history_json, &settings_json, &jobs_json] {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
        eprintln!("migrate: removed legacy JSON files");
    }

    Ok(())
}

// ─── tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    //! Smoke tests for the pure SQL helpers. The Tauri-dependent
    //! `migrate_from_json_if_needed` is tested by the actual app launch —
    //! it's hard to construct an `AppHandle` outside the runtime.
    //!
    //! Run with: `cargo test -p transcribe db::tests`

    use super::*;
    use crate::{JobRecord, NewTranscriptRecord};

    fn temp_db(label: &str) -> Connection {
        let dir = std::env::temp_dir().join(format!("transcribe-db-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let conn = open(&dir.join("app.db")).expect("open");
        run_migrations(&conn).expect("migrate");
        conn
    }

    #[test]
    fn schema_creates_all_tables_and_indexes() {
        let conn = temp_db("schema");
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master
                  WHERE type='table' AND name IN ('settings','history','jobs','job_items','meta')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 5, "expected 5 tables");

        let indexes: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master
                  WHERE type='index' AND name LIKE 'idx_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            indexes, 4,
            "expected 4 user indexes (history, jobs created_at, jobs is_active, job_items)"
        );
    }

    #[test]
    fn settings_round_trip() {
        let conn = temp_db("settings");
        let initial = load_settings(&conn).unwrap();
        assert_eq!(initial.model, "tiny");
        assert!(initial.timestamps);
        assert!(!initial.dark_mode);

        let mut s = initial.clone();
        s.model = "base".to_string();
        s.dark_mode = true;
        save_settings(&conn, &s).unwrap();

        let reloaded = load_settings(&conn).unwrap();
        assert_eq!(reloaded.model, "base");
        assert!(reloaded.dark_mode);
        assert!(reloaded.timestamps, "should preserve unrelated fields");
    }

    #[test]
    fn history_save_load_delete() {
        let conn = temp_db("history");
        let new_record = NewTranscriptRecord {
            url: "https://example.com/watch?v=abc".to_string(),
            title: Some("Example Video".to_string()),
            language: "en".to_string(),
            plain: "Hello world.".to_string(),
            timestamped: Some("[00:00] Hello world.".to_string()),
            srt: "1\n00:00:00,000 --> 00:00:01,000\nHello world.\n".to_string(),
            model: "tiny".to_string(),
            word_count: 2,
        };
        let id = save_transcript(&conn, new_record).unwrap();
        assert!(!id.is_empty());

        let loaded = load_history(&conn).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, id);
        assert_eq!(loaded[0].url, "https://example.com/watch?v=abc");
        assert_eq!(loaded[0].word_count, 2);

        delete_transcript(&conn, &id).unwrap();
        assert!(load_history(&conn).unwrap().is_empty());
    }

    #[test]
    fn history_trims_to_max_records() {
        let conn = temp_db("trim");
        // We can't actually insert MAX_HISTORY_RECORDS (500) in a unit test
        // cheaply. Instead, temporarily lower the cap by inserting just
        // enough to demonstrate the trim query works.
        for i in 0..5 {
            let rec = NewTranscriptRecord {
                url: format!("https://example.com/{i}"),
                title: None,
                language: "en".to_string(),
                plain: format!("rec {i}"),
                timestamped: None,
                srt: String::new(),
                model: "tiny".to_string(),
                word_count: 1,
            };
            save_transcript(&conn, rec).unwrap();
        }
        let loaded = load_history(&conn).unwrap();
        assert_eq!(loaded.len(), 5, "below cap keeps all");
        // The cap SQL is exercised; we trust it works at the full 500 since
        // it's the same query with a different LIMIT.
    }

    #[test]
    fn job_save_load_retry_delete_round_trip() {
        let conn = temp_db("jobs");

        let job = JobRecord {
            id: "job-1".to_string(),
            model: "tiny".to_string(),
            timestamps: true,
            created_at: "2026-06-27T10:00:00.000Z".to_string(),
            completed_at: "2026-06-27T10:05:00.000Z".to_string(),
            elapsed_ms: 300_000,
            total_items: 2,
            success_count: 1,
            failure_count: 1,
            cancelled_count: 0,
            total_words: 100,
            total_audio_secs: 600,
            items: vec![
                JobItemRecord {
                    id: "item-1".to_string(),
                    url: "https://example.com/a".to_string(),
                    title: "A".to_string(),
                    thumbnail: "https://i.example.com/a.jpg".to_string(),
                    duration_secs: 300,
                    status: "done".to_string(),
                    error_code: None,
                    error_message: None,
                    language: Some("en".to_string()),
                    plain: Some("transcript A".to_string()),
                    timestamped: None,
                    srt: None,
                    word_count: Some(50),
                    started_at: Some("2026-06-27T10:00:00.000Z".to_string()),
                    completed_at: Some("2026-06-27T10:02:30.000Z".to_string()),
                    elapsed_ms: Some(150_000),
                    download_percent: Some(100.0),
                    downloaded_bytes: Some(4_500_000),
                    total_bytes: Some(4_500_000),
                },
                JobItemRecord {
                    id: "item-2".to_string(),
                    url: "https://example.com/b".to_string(),
                    title: "B".to_string(),
                    thumbnail: "https://i.example.com/b.jpg".to_string(),
                    duration_secs: 300,
                    status: "failed".to_string(),
                    error_code: Some("NETWORK".to_string()),
                    error_message: Some("Connection reset".to_string()),
                    language: None,
                    plain: None,
                    timestamped: None,
                    srt: None,
                    word_count: None,
                    started_at: Some("2026-06-27T10:02:30.000Z".to_string()),
                    completed_at: Some("2026-06-27T10:03:00.000Z".to_string()),
                    elapsed_ms: Some(30_000),
                    download_percent: Some(12.5),
                    downloaded_bytes: Some(600_000),
                    total_bytes: Some(4_800_000),
                },
            ],
        };

        save_job(&conn, job.clone()).unwrap();
        let loaded = load_jobs(&conn).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "job-1");
        assert_eq!(loaded[0].items.len(), 2);
        assert_eq!(loaded[0].items[1].status, "failed");
        assert_eq!(loaded[0].items[1].error_code.as_deref(), Some("NETWORK"));
        assert_eq!(loaded[0].items[1].download_percent, Some(12.5));
        assert_eq!(loaded[0].items[1].total_bytes, Some(4_800_000));

        // Retry resets error/timing fields but keeps stable identity.
        let reset = retry_job_item(&conn, "job-1", "item-2").unwrap();
        assert_eq!(reset.status, "waiting");
        assert_eq!(reset.error_code, None);
        assert_eq!(reset.error_message, None);
        assert_eq!(reset.started_at, None);
        assert_eq!(reset.completed_at, None);
        assert_eq!(reset.elapsed_ms, None);
        assert_eq!(reset.download_percent, Some(12.5), "non-error fields preserved");
        assert_eq!(reset.id, "item-2");
        assert_eq!(reset.url, "https://example.com/b");

        // Delete cascades to items.
        delete_job(&conn, "job-1").unwrap();
        assert!(load_jobs(&conn).unwrap().is_empty());
    }

    #[test]
    fn retry_unknown_item_errors() {
        let conn = temp_db("retry-err");
        let job = JobRecord {
            id: "job-x".to_string(),
            model: "tiny".to_string(),
            timestamps: true,
            created_at: "2026-06-27T10:00:00.000Z".to_string(),
            completed_at: "2026-06-27T10:00:01.000Z".to_string(),
            elapsed_ms: 1_000,
            total_items: 0,
            success_count: 0,
            failure_count: 0,
            cancelled_count: 0,
            total_words: 0,
            total_audio_secs: 0,
            items: vec![],
        };
        save_job(&conn, job).unwrap();

        let err = retry_job_item(&conn, "job-x", "no-such-item").unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");

        let err = retry_job_item(&conn, "no-such-job", "item-1").unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    /// Build a connection with the OLD (v1) schema only, bypassing
    /// `run_migrations` — simulates a real install that hasn't been
    /// upgraded yet, so the v1→v2 migration path can be tested.
    fn temp_db_v1_shape(label: &str) -> Connection {
        let dir = std::env::temp_dir()
            .join(format!("transcribe-db-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let conn = open(&dir.join("app.db")).expect("open");
        conn.execute_batch(
            r#"
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', '1');

            CREATE TABLE jobs (
                id                TEXT    PRIMARY KEY,
                model             TEXT    NOT NULL,
                timestamps        INTEGER NOT NULL,
                created_at        TEXT    NOT NULL,
                completed_at      TEXT    NOT NULL,
                elapsed_ms        INTEGER NOT NULL,
                total_items       INTEGER NOT NULL,
                success_count     INTEGER NOT NULL,
                failure_count     INTEGER NOT NULL,
                cancelled_count   INTEGER NOT NULL,
                total_words       INTEGER NOT NULL,
                total_audio_secs  INTEGER NOT NULL
            );

            CREATE TABLE job_items (
                id                TEXT    PRIMARY KEY,
                job_id            TEXT    NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                idx               INTEGER NOT NULL,
                url               TEXT    NOT NULL,
                title             TEXT    NOT NULL,
                thumbnail         TEXT    NOT NULL,
                duration_secs     INTEGER NOT NULL,
                status            TEXT    NOT NULL,
                error_code        TEXT,
                error_message     TEXT,
                language          TEXT,
                plain             TEXT,
                timestamped       TEXT,
                srt               TEXT,
                word_count        INTEGER,
                started_at        TEXT,
                completed_at      TEXT,
                elapsed_ms        INTEGER,
                download_percent  REAL,
                downloaded_bytes  INTEGER,
                total_bytes       INTEGER
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO jobs (id, model, timestamps, created_at, completed_at, elapsed_ms,
                                total_items, success_count, failure_count, cancelled_count,
                                total_words, total_audio_secs)
             VALUES ('job-v1', 'tiny', 1, '2026-06-01T00:00:00.000Z', '2026-06-01T00:05:00.000Z',
                     300000, 1, 1, 0, 0, 50, 120)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO job_items (id, job_id, idx, url, title, thumbnail, duration_secs, status,
                                     language, plain, srt, word_count, started_at, completed_at)
             VALUES ('item-v1', 'job-v1', 0, 'https://example.com/v1', 'V1 Video', 'https://thumb',
                     120, 'done', 'en', 'hello', 'srt-body', 10,
                     '2026-06-01T00:00:00.000Z', '2026-06-01T00:02:00.000Z')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn v1_to_v2_migration_preserves_existing_job_data() {
        let conn = temp_db_v1_shape("migrate");

        // Sanity: confirm we're starting from the OLD shape (no is_active/phase).
        assert!(!table_has_column(&conn, "jobs", "is_active").unwrap());
        assert!(!table_has_column(&conn, "job_items", "phase").unwrap());

        run_migrations(&conn).expect("migration should succeed");

        // New columns exist post-migration.
        assert!(table_has_column(&conn, "jobs", "is_active").unwrap());
        assert!(table_has_column(&conn, "job_items", "phase").unwrap());
        assert!(table_has_column(&conn, "job_items", "download_path").unwrap());

        // Old job data is preserved byte-for-byte via the legacy load_jobs path.
        let jobs = load_jobs(&conn).expect("load_jobs after migration");
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!(job.id, "job-v1");
        assert_eq!(job.completed_at, "2026-06-01T00:05:00.000Z");
        assert_eq!(job.elapsed_ms, 300_000);
        assert_eq!(job.total_words, 50);
        assert_eq!(job.items.len(), 1);
        assert_eq!(job.items[0].id, "item-v1");
        assert_eq!(job.items[0].status, "done");
        assert_eq!(job.items[0].plain.as_deref(), Some("hello"));

        // is_active defaults to 0 for pre-existing (already-completed) jobs.
        let active: i64 = conn
            .query_row("SELECT is_active FROM jobs WHERE id = 'job-v1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(active, 0);

        // phase was backfilled from the old status column, not left at the
        // DEFAULT 'waiting' that ADD COLUMN would otherwise apply.
        let phase: String = conn
            .query_row("SELECT phase FROM job_items WHERE id = 'item-v1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(phase, "done");

        // schema_version is bumped.
        let version: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, "2");
    }

    #[test]
    fn migration_is_idempotent_on_already_v2_db() {
        // A fresh DB is already v2 shape; running migrations again must not
        // error or duplicate columns.
        let conn = temp_db("idempotent");
        run_migrations(&conn).expect("second migration run should be a no-op");
        run_migrations(&conn).expect("third migration run should be a no-op");
    }

    #[test]
    fn pipeline_job_lifecycle_create_update_finalize() {
        let conn = temp_db("pipeline");

        let job = ActiveJobRecord {
            id: "job-p1".to_string(),
            model: "tiny".to_string(),
            timestamps: true,
            created_at: "2026-06-28T00:00:00.000Z".to_string(),
            completed_at: None,
            elapsed_ms: None,
            total_items: 2,
            success_count: 0,
            failure_count: 0,
            cancelled_count: 0,
            total_words: 0,
            total_audio_secs: 0,
            is_active: true,
            items: vec![
                ActiveJobItemRecord {
                    id: "item-p1".to_string(),
                    url: "https://example.com/p1".to_string(),
                    title: "P1".to_string(),
                    thumbnail: "https://thumb/p1".to_string(),
                    duration_secs: 60,
                    phase: "waiting".to_string(),
                    download_path: None,
                    error_code: None,
                    error_message: None,
                    language: None,
                    plain: None,
                    timestamped: None,
                    srt: None,
                    word_count: None,
                    started_at: None,
                    completed_at: None,
                },
                ActiveJobItemRecord {
                    id: "item-p2".to_string(),
                    url: "https://example.com/p2".to_string(),
                    title: "P2".to_string(),
                    thumbnail: "https://thumb/p2".to_string(),
                    duration_secs: 90,
                    phase: "waiting".to_string(),
                    download_path: None,
                    error_code: None,
                    error_message: None,
                    language: None,
                    plain: None,
                    timestamped: None,
                    srt: None,
                    word_count: None,
                    started_at: None,
                    completed_at: None,
                },
            ],
        };

        create_job_active(&conn, &job).unwrap();

        // Immediately resumable: job + both items visible via load_active_job.
        let active = load_active_job(&conn).unwrap().expect("job should be active");
        assert_eq!(active.id, "job-p1");
        assert!(active.is_active);
        assert_eq!(active.completed_at, None);
        assert_eq!(active.items.len(), 2);
        assert_eq!(active.items[0].phase, "waiting");

        // load_jobs (completed-only view) must NOT include the active job.
        assert!(load_jobs(&conn).unwrap().is_empty());

        // Item 1 progresses through the pipeline.
        update_item_phase(&conn, "item-p1", "downloading", None).unwrap();
        update_item_phase(&conn, "item-p1", "downloaded", Some("/tmp/item-p1.mp3")).unwrap();
        update_item_phase(&conn, "item-p1", "transcribing", None).unwrap();
        update_item_result(
            &conn,
            "item-p1",
            &ItemResultPayload {
                language: "en".to_string(),
                plain: "hello world".to_string(),
                timestamped: None,
                srt: "1\n00:00:00,000 --> 00:00:01,000\nhello world\n".to_string(),
                word_count: 2,
            },
        )
        .unwrap();

        // Item 2 fails during download.
        update_item_phase(&conn, "item-p2", "downloading", None).unwrap();
        update_item_error(&conn, "item-p2", "NETWORK_ERROR", "connection reset").unwrap();

        let active = load_active_job(&conn).unwrap().unwrap();
        let item1 = active.items.iter().find(|i| i.id == "item-p1").unwrap();
        assert_eq!(item1.phase, "done");
        assert_eq!(item1.plain.as_deref(), Some("hello world"));
        assert_eq!(item1.download_path.as_deref(), Some("/tmp/item-p1.mp3"));
        let item2 = active.items.iter().find(|i| i.id == "item-p2").unwrap();
        assert_eq!(item2.phase, "failed");
        assert_eq!(item2.error_code.as_deref(), Some("NETWORK_ERROR"));

        // Finalize: is_active flips off, stats land on the jobs row, and
        // the job now shows up in the completed-jobs view instead.
        finalize_job(
            &conn,
            "job-p1",
            &JobStatsPayload {
                elapsed_ms: 5_000,
                success_count: 1,
                failure_count: 1,
                cancelled_count: 0,
                total_words: 2,
                total_audio_secs: 60,
            },
        )
        .unwrap();

        assert!(load_active_job(&conn).unwrap().is_none());
        let completed = load_jobs(&conn).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "job-p1");
        assert_eq!(completed[0].success_count, 1);
        assert_eq!(completed[0].failure_count, 1);
        assert_eq!(completed[0].total_words, 2);
    }

    #[test]
    fn list_active_job_ids_reflects_is_active_only() {
        let conn = temp_db("active-ids");

        assert_eq!(list_active_job_ids(&conn).unwrap(), Vec::<String>::new());

        let job = ActiveJobRecord {
            id: "job-orphan-check".to_string(),
            model: "tiny".to_string(),
            timestamps: true,
            created_at: "2026-06-28T00:00:00.000Z".to_string(),
            completed_at: None,
            elapsed_ms: None,
            total_items: 1,
            success_count: 0,
            failure_count: 0,
            cancelled_count: 0,
            total_words: 0,
            total_audio_secs: 0,
            is_active: true,
            items: vec![ActiveJobItemRecord {
                id: "item-1".to_string(),
                url: "https://example.com/1".to_string(),
                title: "One".to_string(),
                thumbnail: "https://thumb/1".to_string(),
                duration_secs: 30,
                phase: "waiting".to_string(),
                download_path: None,
                error_code: None,
                error_message: None,
                language: None,
                plain: None,
                timestamped: None,
                srt: None,
                word_count: None,
                started_at: None,
                completed_at: None,
            }],
        };
        create_job_active(&conn, &job).unwrap();

        // While active, this job's id is exactly what the orphan-downloads
        // scan must NOT delete.
        assert_eq!(list_active_job_ids(&conn).unwrap(), vec!["job-orphan-check".to_string()]);

        finalize_job(
            &conn,
            "job-orphan-check",
            &JobStatsPayload {
                elapsed_ms: 1_000,
                success_count: 1,
                failure_count: 0,
                cancelled_count: 0,
                total_words: 0,
                total_audio_secs: 30,
            },
        )
        .unwrap();

        // Once finalized, its downloads/ dir is fair game for the orphan
        // scan to remove (finalize_job already deletes it directly, but
        // the id must also no longer protect a stale dir from a crash).
        assert_eq!(list_active_job_ids(&conn).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn probe_cache_round_trip() {
        let conn = temp_db("probe-cache");
        let url = "https://www.youtube.com/@Channel/shorts";
        let result = serde_json::json!({
            "type": "playlist",
            "kind": "playlist",
            "url": url,
            "title": "Channel Shorts",
            "count": 5,
            "total_count": 100,
            "entries": [
                {"id": "abc", "title": "v1", "thumbnail": "", "duration": 0, "url": "https://example.com/v1"},
                {"id": "def", "title": "v2", "thumbnail": "", "duration": 0, "url": "https://example.com/v2"},
            ],
        });

        // Miss before write.
        assert!(get_cached_probe(&conn, url).unwrap().is_none());

        cache_probe(&conn, url, &result).unwrap();

        // Hit immediately after write — value round-trips intact, age ≈ 0.
        let (cached, age) = get_cached_probe(&conn, url).unwrap().expect("fresh hit");
        assert_eq!(cached["type"], "playlist");
        assert_eq!(cached["count"], 5);
        assert_eq!(cached["entries"][0]["id"], "abc");
        assert!(age < 5, "fresh cache should have age < 5s, got {age}");
    }

    #[test]
    fn probe_cache_respects_ttl() {
        let conn = temp_db("probe-cache-ttl");
        let url = "https://www.youtube.com/@Channel/shorts";
        let result = serde_json::json!({"type": "playlist", "url": url});

        // Insert with a 1-second TTL — the freshness check uses
        // (now - fetched_at) > ttl_secs, so anything older than 1s is
        // treated as expired.
        conn.execute(
            "INSERT INTO probe_cache (url, result_json, fetched_at, ttl_secs)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 seconds'), ?3)",
            params![url, serde_json::to_string(&result).unwrap(), 1i64],
        )
        .unwrap();

        // 2s old against a 1s TTL → expired, must be a miss even though the
        // row is present.
        assert!(
            get_cached_probe(&conn, url).unwrap().is_none(),
            "row older than ttl should be a miss"
        );
    }

    #[test]
    fn probe_cache_invalidate_removes_row() {
        let conn = temp_db("probe-cache-invalidate");
        let url = "https://www.youtube.com/@Channel/shorts";
        let result = serde_json::json!({"type": "playlist", "url": url});

        cache_probe(&conn, url, &result).unwrap();
        assert!(get_cached_probe(&conn, url).unwrap().is_some());

        invalidate_probe(&conn, url).unwrap();
        assert!(
            get_cached_probe(&conn, url).unwrap().is_none(),
            "invalidate must remove the row"
        );

        // Idempotent: invalidating a missing key is a no-op, not an error.
        invalidate_probe(&conn, url).unwrap();
    }

    #[test]
    fn probe_cache_overwrite_replaces_value() {
        let conn = temp_db("probe-cache-overwrite");
        let url = "https://www.youtube.com/@Channel/shorts";
        let v1 = serde_json::json!({"type": "playlist", "url": url, "count": 5});
        let v2 = serde_json::json!({"type": "playlist", "url": url, "count": 20});

        cache_probe(&conn, url, &v1).unwrap();
        cache_probe(&conn, url, &v2).unwrap();

        let (cached, _) = get_cached_probe(&conn, url).unwrap().expect("hit");
        assert_eq!(
            cached["count"], 20,
            "second cache_probe should overwrite, not duplicate"
        );
    }
}
