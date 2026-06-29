# F13 — Pipeline Queue Architecture: Chunked Downloads + Sequential Transcription + DB Resume

## Priority
P1 — Depends on F12. Full architectural change. Do not start until F12 is shipped and working.

---

## Principle: SQLite is the only state store

No JSON files. No `queue_state.json`. No `jobs.json`.  
The existing SQLite DB at `~/Library/Application Support/com.shuhari.transcribe/app.db` is the single source of truth for everything — in-progress jobs, item phases, download paths, resume state.

The current `jobs` table only stores completed jobs (written by `finalizeJob` at the end). This spec changes that: jobs are written to DB the moment they start, and item rows are updated in-place as phases change. Resume is just a DB read on startup.

---

## Problem with the current architecture

Current `runQueueLoop` in `+page.svelte`:

```
for each item:
    await run_sidecar(url)   ← download + transcribe, blocking, one at a time
```

For 50 videos: CPU idle during every download, network idle during every transcription. Total = Σ(download + transcribe) with no overlap.

**Target:**

```
Download phase  — 5 at a time, rolling chunks
Transcription   — 1 at a time, starts as each download completes
Both run concurrently — download chunk N+1 while transcribing from chunk N
```

---

## Schema changes (migration to version 2)

### `jobs` table — make `completed_at` nullable

```sql
-- Migration: schema_version 1 → 2

-- SQLite does not support ALTER COLUMN, so recreate the table.
-- All existing completed jobs keep their data.

CREATE TABLE IF NOT EXISTS jobs_v2 (
    id                TEXT    PRIMARY KEY,
    model             TEXT    NOT NULL,
    timestamps        INTEGER NOT NULL,
    created_at        TEXT    NOT NULL,
    completed_at      TEXT,               -- NULL while job is in progress
    elapsed_ms        INTEGER,            -- NULL while in progress
    total_items       INTEGER NOT NULL,
    success_count     INTEGER NOT NULL DEFAULT 0,
    failure_count     INTEGER NOT NULL DEFAULT 0,
    cancelled_count   INTEGER NOT NULL DEFAULT 0,
    total_words       INTEGER NOT NULL DEFAULT 0,
    total_audio_secs  INTEGER NOT NULL DEFAULT 0,
    is_active         INTEGER NOT NULL DEFAULT 0   -- 1 while job is running
);

INSERT INTO jobs_v2 SELECT
    id, model, timestamps, created_at, completed_at, elapsed_ms,
    total_items, success_count, failure_count, cancelled_count,
    total_words, total_audio_secs, 0
FROM jobs;

DROP TABLE jobs;
ALTER TABLE jobs_v2 RENAME TO jobs;

CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_jobs_active ON jobs (is_active) WHERE is_active = 1;
```

### `job_items` table — add `phase` and `download_path` columns

```sql
ALTER TABLE job_items ADD COLUMN phase TEXT NOT NULL DEFAULT 'waiting';
-- phase values: waiting | downloading | downloaded | transcribing | done | failed | cancelled

ALTER TABLE job_items ADD COLUMN download_path TEXT;
-- absolute path to downloaded audio file; NULL until download completes
```

### `meta` table — bump schema version

```sql
INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', '2');
```

**Migration implementation in `db.rs`:**
- Increment `SCHEMA_VERSION` constant to `2`
- Read current version from `meta` table on `run_migrations`
- If version < 2: execute the migration SQL above
- Migration is idempotent — `CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`

---

## New DB functions in `db.rs`

### `create_job_active(conn, job)` — write a job at start time

```rust
pub fn create_job_active(conn: &Connection, job: &ActiveJobRecord) -> Result<(), String>
```

- Inserts into `jobs` with `completed_at = NULL`, `is_active = 1`
- Inserts all items into `job_items` with `phase = 'waiting'`
- Called the moment `startBatchJob` fires — before any download starts
- `ActiveJobRecord` shape: id, model, timestamps, created_at, total_items, items[]

### `update_item_phase(conn, item_id, phase, download_path?)` — per-item phase update

```rust
pub fn update_item_phase(
    conn: &Connection,
    item_id: &str,
    phase: &str,
    download_path: Option<&str>,
) -> Result<(), String>
```

- `UPDATE job_items SET phase = ?1, download_path = ?2 WHERE id = ?3`
- Called on every phase transition: waiting→downloading, downloading→downloaded, downloaded→transcribing, transcribing→done/failed/cancelled
- Fast single-row UPDATE — called fire-and-forget from frontend

### `update_item_result(conn, item_id, language, plain, timestamped, srt, word_count)` — on transcription done

```rust
pub fn update_item_result(conn: &Connection, item_id: &str, result: &ItemResult) -> Result<(), String>
```

- Updates `job_items` SET phase='done', language, plain, timestamped, srt, word_count, completed_at
- Replaces the current `save_transcript` call for batch items (history table still gets written separately)

### `update_item_error(conn, item_id, phase, error_code, error_message)` — on failure

```rust
pub fn update_item_error(conn: &Connection, item_id: &str, ...) -> Result<(), String>
```

- Updates `job_items` SET phase='failed', error_code, error_message, completed_at

### `finalize_job(conn, job_id, stats)` — mark job complete

```rust
pub fn finalize_job(conn: &Connection, job_id: &str, stats: &JobStats) -> Result<(), String>
```

- Updates `jobs` SET completed_at, elapsed_ms, success_count, failure_count, cancelled_count, total_words, total_audio_secs, is_active=0
- Existing `save_job` function is retired — `create_job_active` + `finalize_job` replace it

### `load_active_job(conn)` — resume on startup

```rust
pub fn load_active_job(conn: &Connection) -> Result<Option<ActiveJobRecord>, String>
```

- `SELECT * FROM jobs WHERE is_active = 1 LIMIT 1`
- Joins `job_items` for that job_id
- Returns None if no active job

### `delete_job_downloads(app, job_id)` — cleanup audio files

Not a DB function — Rust filesystem cleanup:

```rust
async fn delete_job_downloads(app: &AppHandle, job_id: &str) -> Result<(), String>
```

- Deletes `<app_data_dir>/downloads/<job_id>/` directory recursively
- Called on job finalization and on Discard from resume banner

---

## New Tauri commands in `lib.rs`

### `start_job` — replaces frontend calling `save_job` at the end

```rust
#[tauri::command]
async fn start_job(app, job: ActiveJobPayload) -> Result<(), String>
```

- Calls `db::create_job_active` — writes job + all items to DB immediately
- Creates `<app_data_dir>/downloads/<job_id>/` directory

### `update_item_phase`

```rust
#[tauri::command]
async fn update_item_phase(app, item_id: String, phase: String, download_path: Option<String>) -> Result<(), String>
```

- Thin wrapper over `db::update_item_phase`
- Called fire-and-forget on every phase transition

### `update_item_result`

```rust
#[tauri::command]
async fn update_item_result(app, item_id: String, result: ItemResultPayload) -> Result<(), String>
```

- Calls `db::update_item_result` + `db::save_transcript` (history table) in one transaction

### `update_item_error`

```rust
#[tauri::command]
async fn update_item_error(app, item_id: String, error_code: String, error_message: String) -> Result<(), String>
```

### `finalize_job`

```rust
#[tauri::command]
async fn finalize_job(app, job_id: String, stats: JobStatsPayload) -> Result<(), String>
```

- Calls `db::finalize_job`, then `delete_job_downloads`

### `load_active_job`

```rust
#[tauri::command]
async fn load_active_job(app) -> Result<Option<ActiveJobPayload>, String>
```

- Calls `db::load_active_job`
- Returns full job with items and their current phase + download_path

### `download_item`

```rust
#[tauri::command]
async fn download_item(app, job_id: String, item_id: String, url: String) -> Result<(), String>
```

- Spawns sidecar with `--mode download --url <url> --out-dir <downloads/<job_id>/> --item-id <item_id>`
- Stores child in `RunningDownloads: Arc<Mutex<HashMap<String, Child>>>`
- Streams events → emits as `"download-progress"` Tauri event with `item_id` in every payload
- On `download-done`: calls `db::update_item_phase(item_id, "downloaded", path)` directly in Rust

### `transcribe_item`

```rust
#[tauri::command]
async fn transcribe_item(app, item_id: String, audio_path: String, model: String, timestamps: bool) -> Result<(), String>
```

- Spawns sidecar with `--mode transcribe --audio-path <path> --item-id <item_id> --model <model> --timestamps <bool>`
- Streams events → emits as `"transcribe-progress"` (same channel as today, adds `item_id` field)
- Stores child in existing `RunningSidecar` slot (one transcription at a time)

### `cancel_download`

```rust
#[tauri::command]
async fn cancel_download(state: State<'_, RunningDownloads>, item_id: String) -> Result<(), String>
```

- Kills child for `item_id` from `RunningDownloads`
- Deletes partial file: glob `<downloads/<job_id>/<item_id>.*`

---

## Python sidecar: two new modes

**`--mode download`**

```
python -m api.sidecar --mode download --url <url> --out-dir <path> --item-id <id>
```

- Calls `download_audio(url, out_dir, ...)` only — no model, no transcription
- Emits: `phase:downloading-audio` events with `item_id` in each payload
- On success: `{"event":"download-done","item_id":"<id>","path":"<abs-path>"}`
- On failure: `{"event":"error","item_id":"<id>","code":"...","message":"..."}`

**`--mode transcribe`**

```
python -m api.sidecar --mode transcribe --audio-path <path> --item-id <id> --model <model> --timestamps true|false
```

- Reads audio from file, calls `transcribe_audio()` only — no yt-dlp
- Emits: `phase:transcribing`, `progress` segments, `result` — all with `item_id`
- On success: deletes audio file (`os.unlink(audio_path)`)
- On failure: does NOT delete file (allow retry without re-download)

**`--mode transcribe-full`** — existing behaviour, unchanged, used for single-video path.

**`item_id` in every event payload:**  
Both new modes add `"item_id": "<id>"` to every emitted event. This lets the frontend route concurrent download events to the correct queue row.

---

## Frontend queue runner: `runPipelineJob`

Replaces `runQueueLoop` in `+page.svelte`.

### Item phase (replaces `status`)

| Phase | Meaning | Row display |
|---|---|---|
| `waiting` | Not started | ○ Waiting |
| `downloading` | Download sidecar running | ↓ 62% · 1.2 MB/s |
| `downloaded` | Audio on disk, awaiting transcription slot | ✓ Downloaded · queued |
| `transcribing` | Transcription sidecar running | ✦ Transcribing · seg 42 |
| `done` | Complete | ✓ 1.2k words |
| `failed` | Error | ✗ Error label |
| `cancelled` | Skipped by user | — Cancelled |

### Algorithm

```
function runPipelineJob(job):

  1. call start_job(job)          ← write to DB immediately
  2. subscribe to "download-progress" events → route by payload.item_id
  3. subscribe to "transcribe-progress" events → route by payload.item_id

  DOWNLOAD RUNNER (async, event-driven):
    chunks = partition(job.items, size=5)
    for each chunk:
      for each item in chunk:
        call update_item_phase(item.id, "downloading")
        call download_item(job.id, item.id, item.url)
      wait until all items in chunk are done/failed/cancelled
        (tracked by counting download-done + error + terminated events)
      → advance to next chunk

  TRANSCRIPTION RUNNER (async, event-driven):
    transcribeQueue = []     ← items enter here when download completes
    transcribeActive = null

    on download-done(item_id, path):
      item.phase = "downloaded"
      item.downloadPath = path
      call update_item_phase(item_id, "downloaded", path)
      transcribeQueue.push(item_id)
      if transcribeActive == null:
        startNextTranscription()

    function startNextTranscription():
      if transcribeQueue is empty: return
      item_id = transcribeQueue.shift()
      item = job.items[item_id]
      transcribeActive = item_id
      call update_item_phase(item_id, "transcribing")
      call transcribe_item(item_id, item.downloadPath, job.model, job.timestamps)

    on transcribe-progress result(item_id, payload):
      call update_item_result(item_id, payload)   ← DB + history
      item.phase = "done"
      transcribeActive = null
      startNextTranscription()

    on transcribe-progress error(item_id, payload):
      call update_item_error(item_id, payload.code, payload.message)
      item.phase = "failed"
      transcribeActive = null
      startNextTranscription()

  JOB COMPLETION:
    when downloadRunner is done AND transcribeQueue is empty AND transcribeActive is null:
      stats = compute(success_count, failure_count, cancelled_count, total_words, ...)
      call finalize_job(job.id, stats)   ← marks is_active=0, sets completed_at
      unsubscribe all event listeners
      queueActive = false
```

### Key properties
- Download runner and transcription runner run concurrently via event listeners — no blocking loops
- `download-progress` events carry `item_id` → route to correct row
- `transcribe-progress` events carry `item_id` → route to correct row
- DB is updated on every phase transition — no in-memory-only state that can be lost
- `update_item_phase` is fire-and-forget (don't await in the hot path)

---

## Resume on startup

In `+page.svelte` `onMount`, after `load_history` and `load_settings`:

```js
const activeJob = await invokeFn('load_active_job');
if (activeJob) {
  // Classify items by their persisted phase
  const midDownload = activeJob.items.filter(i => i.phase === 'downloading');
  const downloaded  = activeJob.items.filter(i => i.phase === 'downloaded');
  const waiting     = activeJob.items.filter(i => i.phase === 'waiting');
  const resumable   = midDownload.length + downloaded.length + waiting.length;

  if (resumable > 0) {
    showResumeBanner(activeJob, resumable);
  } else {
    // All items terminal — job was interrupted after all processing, just finalize
    await invokeFn('finalize_job', { jobId: activeJob.id, stats: computeStats(activeJob) });
  }
}
```

**Resume behaviour:**
- Items with phase `downloading`: reset to `waiting` (partial file deleted by `cancel_download` cleanup on startup)
- Items with phase `downloaded`: skip download phase, push straight into `transcribeQueue`
- Items with phase `waiting`: go through normal download → transcription pipeline
- Items with phase `done`/`failed`/`cancelled`: left as-is, not re-run

**Resume banner** (inline below sidebar nav, not a modal):
```
┌─────────────────────────────────────────────┐
│  ↩ Resume unfinished job?                   │
│  32 videos still pending from earlier       │
│  [Resume]   [Discard]                       │
└─────────────────────────────────────────────┘
```

- **Resume**: restore `currentJob` from DB state, call `runPipelineJob` with resume=true
- **Discard**: call `finalize_job` with all pending items marked cancelled, call `delete_job_downloads(job_id)`

---

## Cancellation — three tiers

| Action | Button | What happens |
|---|---|---|
| Skip downloading video | "Skip →" on active download row | `cancel_download(item_id)` → item→cancelled, chunk continues with other 4 |
| Skip transcribing video | "Skip →" on active transcription row | `cancel_transcribe()` → item→cancelled, `startNextTranscription()` |
| Remove pending video | `×` on waiting/downloaded row | item→cancelled in DB, removed from transcribeQueue |
| Cancel entire job | "Cancel job" button top-right | cancel_download all active items, cancel_transcribe, mark all waiting/downloaded/downloading→cancelled, finalize_job |

---

## Download directory layout

```
~/Library/Application Support/com.shuhari.transcribe/
├── app.db                         ← SQLite: all state
└── downloads/
    └── <job_id>/                  ← created by start_job
        ├── <item_id>.mp3          ← deleted after successful transcription
        ├── <item_id>.m4a          ← (format depends on source)
        └── ...
```

**No other files.** No JSON. No flat files. `app.db` + `downloads/` is the complete persistent state.

**Cleanup rules:**
- `transcribe_item` success: sidecar deletes its own audio file
- `cancel_download`: Rust deletes partial file for that item_id
- `finalize_job`: Rust deletes entire `downloads/<job_id>/` directory
- App startup: scan `downloads/` — delete any `<job_id>/` directories where no `is_active=1` job with that `job_id` exists in DB (orphan cleanup from previous crashes)

---

## UI changes in `QueueView.svelte`

### Row states for new phases

**`downloading` row:**
```
[num] [thumb] Title                    ↓ 62% · 1.2 MB/s · ETA 8s   [Skip →]
                                       ████████░░░░░░ (progress bar)
```

**`downloaded` row (waiting for transcription slot):**
```
[num] [thumb] Title                    ✓ Downloaded · in queue
```
Style: subtle green-tinted left border (3px, `var(--green)` at 40% opacity) — "ready but waiting".

**`transcribing` row (active — accent treatment same as F12):**
```
[num] [thumb] Title  ← bold           ✦ Transcribing · seg 42       [Skip →]
3px accent left border + accent tinted background
```

**`waiting` row:**
```
[num] [thumb] Title  ← 55% opacity    ○ Waiting                     [×]
```

### Header — dual-phase indicator

While both phases running:
```
Queue · 8 of 50 done  ·  ↓ Batch 2 of 10  ·  ✦ Transcribing #12
```

Download phase complete, transcription only:
```
Queue · 28 of 50 done  ·  ✦ Transcribing video 29 of 50
```

All done:
```
Queue · ✓ All done · 50 of 50       [View in History →]
```

---

## Single-video backward compatibility

When user pastes a single URL (not from picker): unchanged path.
- `handleTranscribeViaQueue` → `startBatchJob([entry])` → `runPipelineJob` with 1 item
- 1 item, chunk size 1 → download then transcribe, same sequence as before
- Still writes to DB via `start_job` → `update_item_phase` → `finalize_job`
- History table still gets written by `update_item_result`

No separate code path needed — the pipeline handles 1 item the same as 50.

---

## Implementation order

Work in this exact order. Each step is independently testable.

**Step 1 — DB migration (schema v2)**
- Update `SCHEMA_VERSION = 2` in `db.rs`
- Add migration path in `run_migrations`: if version < 2, recreate `jobs` table (nullable `completed_at`, add `is_active`), add columns to `job_items`
- Add new DB functions: `create_job_active`, `update_item_phase`, `update_item_result`, `update_item_error`, `finalize_job`, `load_active_job`
- Remove `save_job` (or keep as deprecated wrapper — do not use in new code)
- Test: run migrations on an existing DB, verify old job data is preserved

**Step 2 — Python sidecar: new modes**
- Add `--mode download` → `run_download(url, out_dir, item_id)` in `sidecar.py`
- Add `--mode transcribe` → `run_transcribe_file(audio_path, item_id, model, timestamps)` in `sidecar.py`
- Both modes add `item_id` to every emitted event
- Keep `--mode transcribe-full` as existing default (used by single-video path)
- Test standalone: `python -m api.sidecar --mode download --url <yt-url> --out-dir /tmp/test --item-id item1`
  - Expected: events stream, file written at `/tmp/test/item1.mp3`, `download-done` emitted
- Test standalone: `python -m api.sidecar --mode transcribe --audio-path /tmp/test/item1.mp3 --item-id item1 --model tiny --timestamps true`
  - Expected: transcription events stream, `result` emitted, file deleted
- Rebuild: `cd api && pyinstaller transcribe-sidecar.spec --noconfirm`

**Step 3 — Rust: new commands**
- Add `RunningDownloads: Arc<Mutex<HashMap<String, Child>>>` to managed state
- Add `download_item`, `cancel_download`, `transcribe_item` commands
- Add `start_job`, `update_item_phase`, `update_item_result`, `update_item_error`, `finalize_job`, `load_active_job` commands
- Register all in `invoke_handler!`
- Test via `tauri dev` console: call `invoke('download_item', {jobId, itemId, url})` for one video
  - Expected: `download-progress` events with `item_id` arrive in frontend listener

**Step 4 — Frontend: `runPipelineJob`**
- Add download runner (chunk loop) and transcription runner (serial drain) to `+page.svelte`
- Subscribe to `download-progress` events — route by `payload.item_id`
- Update transcribe-progress listener to route by `payload.item_id`
- Add `update_item_phase` fire-and-forget calls on each phase transition
- Test with 2 videos, chunk_size=1: verify events, phase transitions, DB rows
- Test with 6 videos, chunk_size=5: verify 5 concurrent downloads, 1 transcription at a time

**Step 5 — QueueView: new phase display**
- Add `downloaded` row state (green-tinted, "✓ Downloaded · in queue")
- Update header to show `↓ Batch N of M` + `✦ Transcribing #N` simultaneously
- "Skip →" button on downloading and transcribing rows (always visible, not hover-only)
- `×` on waiting and downloaded rows (hover-only)

**Step 6 — Resume**
- Add `load_active_job` call in `onMount` after settings load
- Add resume banner component (inline, not modal)
- Test: start 10-video job, force-quit mid-download (`Cmd+Q`), reopen app
  - Expected: banner shows correct pending count, Resume resumes correctly

**Step 7 — Cleanup**
- Orphan cleanup on startup: `load_active_job` returns null → scan `downloads/` → delete dirs with no matching active job
- `finalize_job` Rust command: after DB update, call `delete_job_downloads`

---

## Acceptance criteria

1. Queuing 10 videos: exactly 5 download simultaneously (batch 1). As each finishes, it enters the transcription queue. While batch 1 is downloading, batch 2 does not start. When all 5 of batch 1 are done/failed/cancelled, batch 2 starts.
2. Only one video is ever transcribing at a time.
3. While 5 downloads run in parallel, the queue shows 5 rows with live progress bars, 1 row with "✦ Transcribing", and the rest as "○ Waiting".
4. "✓ Downloaded · in queue" appears on rows that are downloaded but waiting for the transcription slot.
5. Every phase transition is persisted to `app.db` immediately. Verify by opening SQLite: `sqlite3 ~/Library/Application\ Support/com.shuhari.transcribe/app.db "SELECT id, phase FROM job_items ORDER BY idx"`.
6. Force-quitting mid-job and relaunching: resume banner shows correct count. Resume correctly skips re-downloading items already in `downloaded` phase.
7. Resume discard: no audio files remain in `downloads/<job_id>/`, no active job in DB.
8. Skip → on a downloading row: that item becomes cancelled, the other 4 in its chunk continue unaffected.
9. Skip → on the transcribing row: that item becomes cancelled, the next downloaded item starts transcribing immediately.
10. Cancel job: all pending items become cancelled, `finalize_job` is called, `downloads/<job_id>/` is deleted, no resume banner on next launch.
11. Single-video path is unchanged in behaviour — uses pipeline internally but user observes no difference.
12. No JSON files written anywhere. `app.db` is the only persistent state file (plus `downloads/` for temporary audio).
13. `downloads/<job_id>/` is empty (deleted) after a job completes successfully.
14. No orphan `downloads/<job_id>/` directories remain after restart if the corresponding job is no longer active.

---

## Note for the coding agent

**Do not remove `run_sidecar` or `cancel_transcribe`** — keep them for the single-video `transcribe-full` path and for backward compatibility. Add `download_item` and `transcribe_item` as new commands alongside.

**`item_id` in every event is non-negotiable** — 5 download sidecars run concurrently. Without `item_id` in the payload the frontend cannot route events to the right row. Both new sidecar modes must include `item_id` in every JSON line they emit.

**`RunningDownloads` is a `HashMap<String, Child>`** keyed by `item_id`. `cancel_download(item_id)` kills only that child, leaving the other 4 in the chunk running.

**Chunk advancement logic**: the download runner tracks a counter per chunk: `pendingInChunk`. Each `download-done`, `error`, or `terminated` event for an item in the current chunk decrements it. When it hits 0, start the next chunk. This is simpler than any promise-based approach.

**Do not `await update_item_phase`** in the hot path — fire and forget. The Tauri IPC round-trip (~1ms) would stall the event handler if awaited on every progress tick.

**Audio file extension**: yt-dlp may produce `.mp3`, `.m4a`, `.webm`, or `.opus` depending on source. The `download-done` event carries the actual `path` — use that, don't assume extension. `cancel_download` in Rust should glob `<out_dir>/<item_id>.*` and delete whatever matches.

**DB write ordering**: `create_job_active` must complete before any `download_item` calls start — the job row must exist before item rows are updated. Await `start_job` in the frontend before starting the download runner.
