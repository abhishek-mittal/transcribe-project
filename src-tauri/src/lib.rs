// Tauri main entry point for the transcribe-project desktop app.
//
// Wires up:
//   - Plugins: shell (sidecar spawn), dialog (native save), fs (write text)
//   - Commands: run_sidecar (spawn sidecar + stream events), cancel_transcribe
//   - Single-flight concurrency: Mutex<Option<CommandChild>>
//
// See openspec/changes/tauri-desktop-app/design.md D6/D7 for cancellation
// and concurrency semantics.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

mod db;

/// Resource-relative path to the PyInstaller `--onedir` sidecar bundle dir.
///
/// Tauri preserves the directory structure declared in `tauri.conf.json`'s
/// `bundle.resources`, so the entry `"binaries/transcribe-sidecar-…"` is
/// resolvable at `<Resource>/binaries/transcribe-sidecar-…` (both in dev,
/// under `target/debug/binaries/…`, and in the packaged `.app`). The leading
/// `binaries/` is therefore part of the resource path and must be kept here.
///
/// The directory carries the Tauri target-triple suffix (see
/// `scripts/build_sidecar.py`), but the inner executable does not —
/// PyInstaller names it after `--name transcribe-sidecar`. So the bundle is
/// laid out as `<SIDECAR_RESOURCE_DIR>/<SIDECAR_BINARY_NAME>`.
const SIDECAR_RESOURCE_DIR: &str = "binaries/transcribe-sidecar-aarch64-apple-darwin";

/// File name of the inner sidecar executable inside `SIDECAR_RESOURCE_DIR`.
/// This is PyInstaller's `--name`, with no target-triple suffix.
const SIDECAR_BINARY_NAME: &str = "transcribe-sidecar";

/// Resolve the absolute path to the inner sidecar executable.
///
/// The sidecar is bundled as a Tauri `resource` (see tauri.conf.json
/// `bundle.resources`). Each invocation just exec()s the inner binary
/// directly instead of paying PyInstaller's onefile extraction cost on
/// every spawn.
fn sidecar_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .resolve(SIDECAR_RESOURCE_DIR, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("sidecar resource not found: {e}"))?;
    let binary = dir.join(SIDECAR_BINARY_NAME);
    if !binary.exists() {
        return Err(format!(
            "sidecar binary missing at {}",
            binary.display()
        ));
    }
    Ok(binary)
}

#[derive(Default)]
struct RunningSidecar(Mutex<Option<CommandChild>>);

// Storage limits. Kept as `pub` so db.rs can use them in trim queries.
const MAX_HISTORY_RECORDS: usize = 500;
const MAX_JOB_RECORDS: usize = 200;

// ─── Persisted record shapes ──────────────────────────────────────────────
//
// These mirror the JSON shapes the frontend has been sending and receiving.
// SQLite stores them in tables defined in `db.rs`; these structs are the
// serialised form returned to the frontend.

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptRecord {
    id: String,
    url: String,
    #[serde(default)]
    title: Option<String>,
    language: String,
    plain: String,
    #[serde(default)]
    timestamped: Option<String>,
    srt: String,
    model: String,
    word_count: u32,
    created_at: String,
}

/// Fields supplied by the frontend when saving a transcript. `id` and
/// `created_at` are generated server-side.
#[derive(Debug, Deserialize)]
struct NewTranscriptRecord {
    url: String,
    #[serde(default)]
    title: Option<String>,
    language: String,
    plain: String,
    #[serde(default)]
    timestamped: Option<String>,
    srt: String,
    model: String,
    word_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    version: u32,
    model: String,
    timestamps: bool,
    dark_mode: bool,
    max_history_records: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            model: "tiny".to_string(),
            timestamps: true,
            dark_mode: false,
            max_history_records: MAX_HISTORY_RECORDS as u32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobItemRecord {
    id: String,
    url: String,
    title: String,
    thumbnail: String,
    duration_secs: u32,
    status: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_message: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    plain: Option<String>,
    #[serde(default)]
    timestamped: Option<String>,
    #[serde(default)]
    srt: Option<String>,
    #[serde(default)]
    word_count: Option<u32>,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    elapsed_ms: Option<u64>,
    /// Last reported download percentage (0.0–100.0). None if the item
    /// never reported a progress tick (e.g. failed before download started).
    #[serde(default)]
    download_percent: Option<f32>,
    #[serde(default)]
    downloaded_bytes: Option<u64>,
    #[serde(default)]
    total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobRecord {
    id: String,
    model: String,
    timestamps: bool,
    created_at: String,
    completed_at: String,
    elapsed_ms: u64,
    total_items: u32,
    success_count: u32,
    failure_count: u32,
    cancelled_count: u32,
    total_words: u32,
    total_audio_secs: u32,
    items: Vec<JobItemRecord>,
}

// ─── F13: pipeline-queue records (jobs persisted from the moment they
// start, not just on completion — see db.rs SCHEMA_VERSION doc comment) ───

/// One item within an `ActiveJobRecord`. `phase` replaces the old `status`
/// field name to make clear this is the new lifecycle
/// (waiting/downloading/downloaded/transcribing/done/failed/cancelled),
/// distinct from `JobItemRecord.status` used by the legacy `jobs` history
/// shape returned to `load_jobs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveJobItemRecord {
    id: String,
    url: String,
    title: String,
    thumbnail: String,
    duration_secs: u32,
    phase: String,
    #[serde(default)]
    download_path: Option<String>,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_message: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    plain: Option<String>,
    #[serde(default)]
    timestamped: Option<String>,
    #[serde(default)]
    srt: Option<String>,
    #[serde(default)]
    word_count: Option<u32>,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

/// A job persisted the moment `start_job` is called — `completed_at` is
/// `None` until `finalize_job` runs. `is_active` mirrors the DB column so
/// `load_active_job` can hand the frontend a ready-to-render shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveJobRecord {
    id: String,
    model: String,
    timestamps: bool,
    created_at: String,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    elapsed_ms: Option<u64>,
    total_items: u32,
    #[serde(default)]
    success_count: u32,
    #[serde(default)]
    failure_count: u32,
    #[serde(default)]
    cancelled_count: u32,
    #[serde(default)]
    total_words: u32,
    #[serde(default)]
    total_audio_secs: u32,
    #[serde(default)]
    is_active: bool,
    items: Vec<ActiveJobItemRecord>,
}

/// Payload for `update_item_result` — the sidecar's final transcription
/// result for one item.
#[derive(Debug, Deserialize)]
struct ItemResultPayload {
    language: String,
    plain: String,
    #[serde(default)]
    timestamped: Option<String>,
    srt: String,
    word_count: u32,
}

/// Payload for `finalize_job` — stats computed by the frontend once every
/// item has reached a terminal phase.
#[derive(Debug, Deserialize)]
struct JobStatsPayload {
    elapsed_ms: u64,
    success_count: u32,
    failure_count: u32,
    cancelled_count: u32,
    total_words: u32,
    total_audio_secs: u32,
}

/// Tracks in-flight `download_item` child processes, keyed by `item_id`.
/// Distinct from `RunningSidecar` (single-flight, used by the legacy
/// `run_sidecar` path and by `transcribe_item`) because up to 5 downloads
/// run concurrently — see F13 spec "Chunk advancement logic".
#[derive(Default)]
struct RunningDownloads(Mutex<std::collections::HashMap<String, CommandChild>>);

// ─── Tauri commands (thin wrappers over db.rs) ────────────────────────────
//
// Every command opens the SQLite connection (cheap — opens a file in WAL
// mode), optionally runs the one-time JSON→SQLite migration, and delegates
// to the equivalent `db::*` function. Errors are stringified and returned
// to the frontend as `Result<T, String>` per Tauri's convention.

/// Open the SQLite connection and ensure the schema + migration have run.
/// Used by every storage command below.
fn with_db(app: &AppHandle) -> Result<(std::path::PathBuf, rusqlite::Connection), String> {
    let path = db::db_path(app)?;
    let conn = db::open(&path)?;
    db::run_migrations(&conn)?;
    db::migrate_from_json_if_needed(app, &path, &conn)?;
    Ok((path, conn))
}

#[tauri::command]
async fn save_transcript(
    app: AppHandle,
    record: NewTranscriptRecord,
) -> Result<String, String> {
    let (_path, conn) = with_db(&app)?;
    db::save_transcript(&conn, record)
}

#[tauri::command]
async fn load_history(app: AppHandle) -> Result<Vec<TranscriptRecord>, String> {
    let (_path, conn) = with_db(&app)?;
    db::load_history(&conn)
}

#[tauri::command]
async fn delete_transcript(app: AppHandle, id: String) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::delete_transcript(&conn, &id)
}

#[tauri::command]
async fn clear_history(app: AppHandle) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::clear_history(&conn)
}

#[tauri::command]
async fn load_settings(app: AppHandle) -> Result<Settings, String> {
    let (_path, conn) = with_db(&app)?;
    db::load_settings(&conn)
}

#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::save_settings(&conn, &settings)
}

#[tauri::command]
async fn save_job(app: AppHandle, job: JobRecord) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::save_job(&conn, job)
}

#[tauri::command]
async fn load_jobs(app: AppHandle) -> Result<Vec<JobRecord>, String> {
    let (_path, conn) = with_db(&app)?;
    db::load_jobs(&conn)
}

#[tauri::command]
async fn delete_job(app: AppHandle, job_id: String) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::delete_job(&conn, &job_id)
}

#[tauri::command]
async fn retry_job_item(
    app: AppHandle,
    job_id: String,
    item_id: String,
) -> Result<JobItemRecord, String> {
    let (_path, conn) = with_db(&app)?;
    db::retry_job_item(&conn, &job_id, &item_id)
}

// ─── F13: pipeline queue commands ─────────────────────────────────────────

/// Directory holding per-job temporary audio: `<app_data_dir>/downloads/`.
/// `<job_id>/` subdirectories are created by `start_job` and removed by
/// `finalize_job` / orphan cleanup on startup.
fn downloads_root(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    Ok(dir.join("downloads"))
}

fn job_downloads_dir(app: &AppHandle, job_id: &str) -> Result<std::path::PathBuf, String> {
    Ok(downloads_root(app)?.join(job_id))
}

/// Delete `<downloads>/<job_id>/` recursively. Called on job finalization
/// and on Discard from the resume banner. Not an error if already absent.
async fn delete_job_downloads(app: &AppHandle, job_id: &str) -> Result<(), String> {
    let dir = job_downloads_dir(app, job_id)?;
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir)
            .await
            .map_err(|e| format!("delete_job_downloads: {e}"))?;
    }
    Ok(())
}

/// Delete any `downloads/<job_id>/` directory left behind by a crash —
/// i.e. one with no matching `is_active = 1` job row in the DB. Called once
/// from the frontend's `onMount`, after `load_active_job` has already
/// decided whether to show the resume banner (so a job that's still
/// legitimately active never has its in-flight downloads swept).
#[tauri::command]
async fn cleanup_orphan_downloads(app: AppHandle) -> Result<(), String> {
    let active_ids: std::collections::HashSet<String> = {
        let (_path, conn) = with_db(&app)?;
        db::list_active_job_ids(&conn)?.into_iter().collect()
    };

    let root = downloads_root(&app)?;
    if !root.exists() {
        return Ok(());
    }
    let mut entries = tokio::fs::read_dir(&root)
        .await
        .map_err(|e| format!("cleanup_orphan_downloads: read_dir: {e}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| format!("cleanup_orphan_downloads: next_entry: {e}"))?
    {
        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !active_ids.contains(&name) {
            eprintln!("cleanup_orphan_downloads: removing orphaned dir {name}");
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
    Ok(())
}

/// Write a job + all items to the DB the moment a batch job starts (before
/// any download begins) and create its downloads directory. Replaces the
/// old pattern of only calling `save_job` at the very end.
#[tauri::command]
async fn start_job(app: AppHandle, job: ActiveJobRecord) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::create_job_active(&conn, &job)?;
    let dir = job_downloads_dir(&app, &job.id)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("start_job: failed to create downloads dir: {e}"))?;
    Ok(())
}

/// Thin wrapper over `db::update_item_phase`. Called fire-and-forget from
/// the frontend on every phase transition — must stay fast.
#[tauri::command]
async fn update_item_phase(
    app: AppHandle,
    item_id: String,
    phase: String,
    download_path: Option<String>,
) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::update_item_phase(&conn, &item_id, &phase, download_path.as_deref())
}

#[tauri::command]
async fn update_item_result(
    app: AppHandle,
    item_id: String,
    result: ItemResultPayload,
) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::update_item_result(&conn, &item_id, &result)
}

#[tauri::command]
async fn update_item_error(
    app: AppHandle,
    item_id: String,
    error_code: String,
    error_message: String,
) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::update_item_error(&conn, &item_id, &error_code, &error_message)
}

/// Mark a job complete and clean up its downloaded audio. Called once the
/// frontend's pipeline runner sees every item reach a terminal phase.
#[tauri::command]
async fn finalize_job(
    app: AppHandle,
    job_id: String,
    stats: JobStatsPayload,
) -> Result<(), String> {
    {
        let (_path, conn) = with_db(&app)?;
        db::finalize_job(&conn, &job_id, &stats)?;
    }
    delete_job_downloads(&app, &job_id).await
}

/// Return the single in-progress job (if any) for the resume banner.
#[tauri::command]
async fn load_active_job(app: AppHandle) -> Result<Option<ActiveJobRecord>, String> {
    let (_path, conn) = with_db(&app)?;
    db::load_active_job(&conn)
}

/// Spawn the Python sidecar with arguments and stream newline-delimited JSON
/// events from its stdout to the frontend as `transcribe-progress` Tauri events.
///
/// The sidecar reads its request from CLI arguments (not stdin) because
/// tauri-plugin-shell v2 doesn't expose a `.stdin()` builder on its
/// `Command` type. Arguments are passed as
/// `--url <url> --model <model> --timestamps <bool>`.
///
/// Enforces single-flight: if a previous transcription is running, it is
/// killed before spawning the new one. Resolves once the sidecar exits.
#[tauri::command]
async fn run_sidecar(
    app: AppHandle,
    state: State<'_, RunningSidecar>,
    url: String,
    model: String,
    timestamps: bool,
) -> Result<(), String> {
    // Single-flight: stop any previous sidecar before starting a new one.
    {
        let mut guard = state.0.lock().unwrap();
        if let Some(prev) = guard.take() {
            eprintln!(
                "run_sidecar: cancelling previous sidecar (pid={})",
                prev.pid()
            );
            let _ = prev.kill();
        }
    }

    let binary = sidecar_path(&app)?;
    let (mut rx, child) = app
        .shell()
        .command(binary)
        .args([
            "--url",
            &url,
            "--model",
            &model,
            "--timestamps",
            if timestamps { "true" } else { "false" },
        ])
        .spawn()
        .map_err(|e| format!("failed to spawn sidecar: {e}"))?;

    let pid = child.pid();
    eprintln!("run_sidecar: sidecar spawned pid={}", pid);

    // Stash the child so cancel_transcribe can reach it.
    state.0.lock().unwrap().replace(child);

    // Open a log file for sidecar stderr (append, create if missing).
    let log_path = app
        .path()
        .app_log_dir()
        .ok()
        .map(|d| { let _ = fs::create_dir_all(&d); d.join("sidecar.log") });
    let mut log_file = log_path.as_ref().and_then(|p| {
        std::fs::OpenOptions::new().create(true).append(true).open(p).ok()
    });

    // Synthetic "starting" event so the UI shows something during the
    // ~30s PyInstaller cold-start extraction before any real output.
    let _ = app.emit(
        "transcribe-progress",
        serde_json::json!({
            "event": "phase",
            "phase": "starting",
        }),
    );

    // Process sidecar events inline so the command stays pending until the
    // sidecar terminates.
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<serde_json::Value>(line) {
                        Ok(value) => {
                            let _ = app.emit("transcribe-progress", value);
                        }
                        Err(_) => {
                            eprintln!("sidecar: non-JSON line: {}", line);
                        }
                    }
                }
            }
            CommandEvent::Stderr(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                eprintln!("sidecar[stderr]: {}", text);
                if let Some(ref mut f) = log_file {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", text.trim_end());
                }
            }
            CommandEvent::Terminated(payload) => {
                eprintln!(
                    "sidecar: terminated code={:?} signal={:?}",
                    payload.code, payload.signal
                );
                let _ = app.emit(
                    "transcribe-progress",
                    serde_json::json!({
                        "event": "terminated",
                        "code": payload.code,
                        "signal": payload.signal,
                    }),
                );
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Percent-encode a URL for use as a query parameter value.
fn pct_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b => {
                use std::fmt::Write;
                let _ = write!(out, "%{:02X}", b);
            }
        }
    }
    out
}

fn is_yt_video(url: &str) -> bool {
    (url.contains("youtube.com/watch?v=")
        || url.contains("youtu.be/")
        || url.contains("youtube.com/shorts/"))
        && !url.contains("list=")
}

fn is_yt_playlist(url: &str) -> bool {
    url.contains("youtube.com/playlist?list=")
        || (url.contains("youtube.com/watch") && url.contains("list="))
}

/// YouTube `/results?search_query=...` pages. Routed through the sidecar
/// because the oEmbed API doesn't know how to expand a search-results page
/// into a list of videos — that work is done in `probe_url` via yt-dlp's
/// `ytsearch[N]:<query>` extractor.
fn is_yt_search_results(url: &str) -> bool {
    (url.contains("youtube.com/results?") || url.contains("m.youtube.com/results?"))
        && url.contains("search_query=")
}

/// Fast probe via YouTube oEmbed API — no sidecar spawn, no startup delay.
async fn probe_yt_oembed(url: &str) -> serde_json::Value {
    let oembed_url = format!(
        "https://www.youtube.com/oembed?url={}&format=json",
        pct_encode(url)
    );
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return serde_json::json!({"type": "error", "code": "INTERNAL", "message": e.to_string()})
        }
    };
    match client.get(&oembed_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(data) => serde_json::json!({
                    "type": "video",
                    "url": url,
                    "title": data["title"].as_str().unwrap_or(""),
                    "thumbnail": data["thumbnail_url"].as_str().unwrap_or(""),
                    "duration": null,
                    "uploader": data["author_name"].as_str().unwrap_or(""),
                }),
                Err(e) => serde_json::json!({"type": "error", "code": "INTERNAL", "message": e.to_string()}),
            }
        }
        Ok(resp) => serde_json::json!({"type": "error", "code": "UNSUPPORTED_PLATFORM", "message": format!("oEmbed {}", resp.status())}),
        Err(e) => serde_json::json!({"type": "error", "code": "INTERNAL", "message": e.to_string()}),
    }
}

/// Probe via sidecar (for playlists and non-YouTube URLs).
///
/// `extra_args` is appended after `--mode probe --url <url>` — used by
/// `probe_url_page` to pass `--page-start`/`--page-end` without duplicating
/// the spawn/timeout/parse plumbing below.
///
/// F14 progressive picker: the sidecar now emits `entry`/`status`/`done`/
/// `error` events to stdout in addition to the final result line. Lines
/// with an `event` field are forwarded to the frontend as `probe-activity`
/// Tauri events; lines with a `type` field are kept as the result to
/// return. This preserves the existing probe_url return contract while
/// letting the UI render entries as yt-dlp resolves them.
async fn probe_via_sidecar(app: &AppHandle, url: &str, extra_args: &[&str]) -> serde_json::Value {
    // Resolve the SAME onedir bundle that `run_sidecar` uses (via the resource
    // path) instead of `app.shell().sidecar()`. The `.sidecar()` resolver
    // depends on `bundle.externalBin` (which is unset here); in dev it resolves
    // to a stale, self-extracting PyInstaller *onefile* at
    // `target/debug/transcribe-sidecar` that takes ~30s+ to unpack on every
    // spawn — blowing the 60s probe timeout — and runs out-of-date code.
    // Using `sidecar_path` keeps probe and transcription on the identical fast
    // onedir binary.
    let binary = match sidecar_path(app) {
        Ok(p) => p,
        Err(e) => return serde_json::json!({"type": "error", "code": "INTERNAL", "message": format!("sidecar not found: {e}")}),
    };
    let mut args: Vec<&str> = vec!["--mode", "probe", "--url", url];
    args.extend_from_slice(extra_args);
    let (mut rx, child) = match app
        .shell()
        .command(binary)
        .args(args)
        .spawn()
    {
        Ok(v) => v,
        Err(e) => return serde_json::json!({"type": "error", "code": "INTERNAL", "message": format!("spawn failed: {e}")}),
    };
    eprintln!("probe_via_sidecar: pid={}", child.pid());
    // Sidecar probe timeout: covers PyInstaller cold-start (~3-5s on first
    // run, ~1s after) plus yt-dlp's network round-trips. YouTube search
    // results with N=20 entries typically take 2-4s on residential links;
    // we leave generous headroom for slow connections without hanging the
    // UI forever.
    let app_clone = app.clone();
    let result = tokio::time::timeout(Duration::from_secs(60), async move {
        // Final result line (has a `type` field, not an `event` field) is
        // returned; intermediate events (`entry`/`status`/`done`/`error`)
        // are forwarded as `probe-activity` Tauri events for the UI to
        // consume live.
        let mut final_result: Option<serde_json::Value> = None;
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        let t = line.trim();
                        if t.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<serde_json::Value>(t) {
                            Ok(value) => {
                                // Lines with `event` are live activity —
                                // stream them to the frontend so the picker
                                // can render entries as yt-dlp resolves
                                // them. Lines with `type` are the final
                                // result blob (no `event` field); keep
                                // those for the return value.
                                if value.get("event").is_some() {
                                    let _ = app_clone.emit("probe-activity", &value);
                                } else if value.get("type").is_some() {
                                    final_result = Some(value);
                                } else {
                                    eprintln!("probe: unknown JSON line: {t}");
                                }
                            }
                            Err(_) => eprintln!("probe: non-JSON line: {t}"),
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => eprintln!("probe[stderr]: {}", String::from_utf8_lossy(&bytes)),
                CommandEvent::Terminated(p) => { eprintln!("probe: terminated {:?}", p.code); break; }
                _ => {}
            }
        }
        final_result.unwrap_or_else(|| serde_json::json!({"type": "error", "code": "INTERNAL", "message": "probe exited with no output"}))
    }).await;
    let _ = child;
    result.unwrap_or_else(|_| serde_json::json!({"type": "error", "code": "INTERNAL", "message": "probe timed out"}))
}

/// Probe a URL: YouTube videos use the fast oEmbed path; playlists and
/// non-YouTube URLs fall back to the Python sidecar.
#[tauri::command]
async fn probe_url(app: AppHandle, url: String) -> Result<serde_json::Value, String> {
    eprintln!("probe_url: url={url}");
    let result = if is_yt_video(&url) {
        eprintln!("probe_url: taking oEmbed fast path");
        probe_yt_oembed(&url).await
    } else if is_yt_search_results(&url) {
        eprintln!("probe_url: taking sidecar path (search results)");
        probe_via_sidecar(&app, &url, &[]).await
    } else if is_yt_playlist(&url) {
        eprintln!("probe_url: taking sidecar path (playlist)");
        probe_via_sidecar(&app, &url, &[]).await
    } else {
        eprintln!("probe_url: taking sidecar path (unknown platform)");
        probe_via_sidecar(&app, &url, &[]).await
    };
    eprintln!("probe_url: result type={}", result.get("type").and_then(|t| t.as_str()).unwrap_or("?"));
    Ok(result)
}

/// Fetch one additional page of playlist/channel entries ("Load more" in
/// the picker). `page_start`/`page_end` are 1-indexed, matching yt-dlp's
/// own `playliststart`/`playlistend` convention.
#[tauri::command]
async fn probe_url_page(
    app: AppHandle,
    url: String,
    page_start: u32,
    page_end: u32,
) -> Result<serde_json::Value, String> {
    eprintln!("probe_url_page: url={url} page_start={page_start} page_end={page_end}");
    let page_start_s = page_start.to_string();
    let page_end_s = page_end.to_string();
    let result = probe_via_sidecar(
        &app,
        &url,
        &["--page-start", &page_start_s, "--page-end", &page_end_s],
    )
    .await;
    Ok(result)
}

// ─── F14: probe cache (SQLite-backed, TTL 15 min) ─────────────────────────

/// Return the cached probe result for `url` if one exists and is fresh.
/// Returns `Ok(None)` for cache miss / expired entry / corrupt row.
/// On a hit, the frontend renders the picker immediately with no network
/// call, and shows "Loaded from cache · Xm ago" in the activity strip.
#[tauri::command]
async fn get_cached_probe(
    app: AppHandle,
    url: String,
) -> Result<Option<serde_json::Value>, String> {
    let (_path, conn) = with_db(&app)?;
    Ok(db::get_cached_probe(&conn, &url)?.map(|(value, _age)| value))
}

/// Write the probe result to the cache. Called by the frontend once the
/// `probe_url` invoke resolves successfully. `url` is the original input
/// (e.g. `https://www.youtube.com/@MillieAdrian/shorts`), `result` is the
/// final blob the sidecar returned — stored verbatim so cache reads
/// require zero transformation on the frontend hot path.
#[tauri::command]
async fn cache_probe(
    app: AppHandle,
    url: String,
    result: serde_json::Value,
) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::cache_probe(&conn, &url, &result)
}

/// Drop the cache entry for `url`. Called on "Transcribe X videos" click
/// so the next paste triggers a fresh probe (catches newly added videos).
#[tauri::command]
async fn invalidate_probe(app: AppHandle, url: String) -> Result<(), String> {
    let (_path, conn) = with_db(&app)?;
    db::invalidate_probe(&conn, &url)
}

/// Path to the Instagram cookies file the Python sidecar reads — must match
/// `_ig_cookies_file_path()` in `api/transcribe_core.py` exactly (same
/// `app_data_dir`, same filename) since Rust writes it here and Python is
/// the only thing that ever reads it.
fn instagram_cookies_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    Ok(dir.join("instagram_cookies.txt"))
}

/// Parse a raw `Cookie:` request-header value (as copied from DevTools'
/// Network tab — `name1=value1; name2=value2; ...`) into Netscape cookie
/// file lines for `.instagram.com`. Unlike `document.cookie` in the page
/// console, this header includes `HttpOnly` cookies (e.g. `sessionid`),
/// which is the whole reason this entry point exists instead of asking
/// users to run JS in the console.
///
/// Expiry is unknown from a raw header (no `Expires`/`Max-Age` is present in
/// the *request* header, only in the *response* `Set-Cookie`), so every
/// cookie is written with a far-future expiry. yt-dlp doesn't care about an
/// honest expiry — Instagram's own server-side session validation is what
/// actually decides if the cookie still works.
fn parse_cookie_header_to_netscape(raw: &str) -> Result<String, String> {
    let mut lines = vec![
        "# Netscape HTTP Cookie File".to_string(),
        "# This is a generated file! Do not edit.".to_string(),
    ];
    let far_future_expiry = 9_999_999_999u64;
    let mut count = 0;
    for pair in raw.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            continue;
        }
        lines.push(format!(
            ".instagram.com\tTRUE\t/\tTRUE\t{far_future_expiry}\t{name}\t{value}"
        ));
        count += 1;
    }
    if count == 0 {
        return Err("no cookies found — paste the full 'Cookie:' request header value, e.g. 'sessionid=...; csrftoken=...'".to_string());
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

/// Save a pasted Instagram `Cookie:` header value, converted to the
/// Netscape format `_inject_ig_browser_cookies` already knows how to read
/// (it checks this exact file path first, before any live-browser
/// fallback). Overwrites any prior manually-saved cookies.
#[tauri::command]
async fn save_instagram_cookies(app: AppHandle, cookie_header: String) -> Result<(), String> {
    let netscape = parse_cookie_header_to_netscape(&cookie_header)?;
    let path = instagram_cookies_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("save_instagram_cookies: {e}"))?;
    }
    std::fs::write(&path, netscape).map_err(|e| format!("save_instagram_cookies: {e}"))
}

/// Whether a manually-saved Instagram cookies file currently exists. Drives
/// the Settings UI's "cookies saved" indicator without exposing the cookie
/// values themselves back to the frontend.
#[tauri::command]
async fn has_instagram_cookies(app: AppHandle) -> Result<bool, String> {
    Ok(instagram_cookies_path(&app)?.exists())
}

/// Delete the manually-saved Instagram cookies file, if any. Falls back to
/// the live-browser-session probe on the next Instagram request.
#[tauri::command]
async fn clear_instagram_cookies(app: AppHandle) -> Result<(), String> {
    let path = instagram_cookies_path(&app)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("clear_instagram_cookies: {e}"))?;
    }
    Ok(())
}

/// Append a structured crash log entry to the sidecar log file.
///
/// Called by the frontend's ErrorBoundary on any uncaught JS error so that
/// crash details are persisted before the process exits. Never panics —
/// I/O failures are printed to stderr and silently swallowed so a logging
/// failure never causes a secondary crash.
#[tauri::command]
async fn log_error(app: AppHandle, message: String) -> Result<(), String> {
    use std::io::Write;
    use std::time::SystemTime;

    let log_path = app
        .path()
        .app_log_dir()
        .ok()
        .map(|d| {
            let _ = fs::create_dir_all(&d);
            d.join("sidecar.log")
        });

    let Some(path) = log_path else {
        eprintln!("log_error: could not resolve log dir");
        return Ok(());
    };

    // Format timestamp as a simple UTC string without pulling in chrono.
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let timestamp = format_unix_ts(secs);

    let line = format!("[CRASH {}] {}\n", timestamp, message);

    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(line.as_bytes()) {
                eprintln!("log_error: write failed: {e}");
            }
        }
        Err(e) => eprintln!("log_error: open failed ({path:?}): {e}"),
    }

    Ok(())
}

/// Minimal UTC timestamp formatter (YYYY-MM-DDTHH:MM:SSZ) without chrono.
fn format_unix_ts(secs: u64) -> String {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400; // days since 1970-01-01
    // Compute year/month/day via the proleptic Gregorian algorithm.
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm: civil_from_days (Howard Hinnant, public domain)
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
}

/// Write a structured crash entry to `<app_data_dir>/crash-log.json`.
///
/// The file is a JSON array of entries so it is easy to read/parse
/// by agents and developers:
///   [{"timestamp":"...","message":"..."}]
///
/// Appends to existing entries (up to 50 kept; older ones are trimmed).
/// Never panics — I/O failures are printed to stderr and silently swallowed.
#[tauri::command]
async fn write_crash_log(app: AppHandle, message: String) -> Result<(), String> {
    use std::io::Write;

    let log_path = match app.path().app_data_dir() {
        Ok(dir) => {
            let _ = fs::create_dir_all(&dir);
            dir.join("crash-log.json")
        }
        Err(e) => {
            eprintln!("write_crash_log: cannot resolve app_data_dir: {e}");
            return Ok(());
        }
    };

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let timestamp = format_unix_ts(secs);

    // Read existing entries (tolerate missing / corrupt file).
    let mut entries: Vec<serde_json::Value> = std::fs::read_to_string(&log_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    entries.push(serde_json::json!({ "timestamp": timestamp, "message": message }));

    // Keep at most 50 entries.
    if entries.len() > 50 {
        entries.drain(0..entries.len() - 50);
    }

    match serde_json::to_string_pretty(&entries) {
        Ok(json) => {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&log_path)
            {
                if let Err(e) = f.write_all(json.as_bytes()) {
                    eprintln!("write_crash_log: write failed: {e}");
                }
            }
        }
        Err(e) => eprintln!("write_crash_log: serialize failed: {e}"),
    }

    Ok(())
}

/// Force-quit the entire Tauri process. Called by the ErrorBoundary after
/// logging a crash so the app never stays open and unresponsive.
#[tauri::command]
fn force_quit() {
    std::process::exit(1);
}

/// Send SIGKILL to the running sidecar.
#[tauri::command]
async fn cancel_transcribe(state: State<'_, RunningSidecar>) -> Result<(), String> {
    // Take the child out of the Mutex BEFORE awaiting, so we don't hold a
    // non-Send MutexGuard across an await point (which Tauri forbids).
    let child = state.0.lock().unwrap().take();
    if let Some(child) = child {
        let pid = child.pid();
        eprintln!("cancel_transcribe: killing sidecar pid={}", pid);
        let _ = child.kill();
        // Give it a moment to clean up; ignore result.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}

/// Spawn a `--mode download` sidecar for one item. Up to 5 of these run
/// concurrently (the F13 pipeline's chunked download phase), so the child
/// is tracked in `RunningDownloads` keyed by `item_id` rather than the
/// single-slot `RunningSidecar` used by `run_sidecar`/`transcribe_item`.
///
/// Streams events as `"download-progress"` Tauri events — a separate
/// channel from `"transcribe-progress"` so the frontend's two listeners
/// (download runner vs. transcription runner) don't have to filter each
/// other's events out of a shared stream.
#[tauri::command]
async fn download_item(
    app: AppHandle,
    state: State<'_, RunningDownloads>,
    job_id: String,
    item_id: String,
    url: String,
) -> Result<(), String> {
    let binary = sidecar_path(&app)?;
    let out_dir = job_downloads_dir(&app, &job_id)?;
    let out_dir_str = out_dir.to_string_lossy().into_owned();

    let (mut rx, child) = app
        .shell()
        .command(binary)
        .args([
            "--mode",
            "download",
            "--url",
            &url,
            "--out-dir",
            &out_dir_str,
            "--item-id",
            &item_id,
        ])
        .spawn()
        .map_err(|e| format!("download_item: failed to spawn sidecar: {e}"))?;

    let pid = child.pid();
    eprintln!("download_item: item={item_id} pid={pid}");
    state.0.lock().unwrap().insert(item_id.clone(), child);

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<serde_json::Value>(line) {
                        Ok(value) => {
                            // Persist "downloaded" + the file path directly
                            // here, in addition to the frontend's own
                            // fire-and-forget update_item_phase call for the
                            // same transition (redundant but harmless) — this
                            // survives even if the renderer never gets to
                            // make that call (e.g. it crashes right after
                            // this event arrives), which matters for resume.
                            if value.get("event").and_then(|v| v.as_str()) == Some("download-done") {
                                if let Some(path) = value.get("path").and_then(|v| v.as_str()) {
                                    if let Ok((_p, conn)) = with_db(&app) {
                                        let _ = db::update_item_phase(&conn, &item_id, "downloaded", Some(path));
                                    }
                                }
                            }
                            let _ = app.emit("download-progress", value);
                        }
                        Err(_) => {
                            eprintln!("download_item[item={item_id}]: non-JSON line: {line}");
                        }
                    }
                }
            }
            CommandEvent::Stderr(bytes) => {
                eprintln!(
                    "download_item[item={item_id}][stderr]: {}",
                    String::from_utf8_lossy(&bytes)
                );
            }
            CommandEvent::Terminated(payload) => {
                eprintln!(
                    "download_item[item={item_id}]: terminated code={:?} signal={:?}",
                    payload.code, payload.signal
                );
                let _ = app.emit(
                    "download-progress",
                    serde_json::json!({
                        "event": "terminated",
                        "item_id": item_id,
                        "code": payload.code,
                        "signal": payload.signal,
                    }),
                );
                break;
            }
            _ => {}
        }
    }

    state.0.lock().unwrap().remove(&item_id);
    Ok(())
}

/// Kill the download child for `item_id` (leaving the other up-to-4 active
/// downloads in its chunk running) and delete its partial file. The
/// extension is unknown ahead of time (yt-dlp may produce .mp3/.m4a/.webm/
/// .opus depending on source), so this globs `<item_id>.*` rather than
/// assuming one.
#[tauri::command]
async fn cancel_download(
    app: AppHandle,
    state: State<'_, RunningDownloads>,
    job_id: String,
    item_id: String,
) -> Result<(), String> {
    let child = state.0.lock().unwrap().remove(&item_id);
    if let Some(child) = child {
        let pid = child.pid();
        eprintln!("cancel_download: killing item={item_id} pid={pid}");
        let _ = child.kill();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let dir = job_downloads_dir(&app, &job_id)?;
    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        let prefix = format!("{item_id}.");
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
    Ok(())
}

/// Spawn a `--mode transcribe` sidecar for one already-downloaded item.
/// Only one transcription ever runs at a time, so this reuses the same
/// single-slot `RunningSidecar` state as the legacy `run_sidecar` path
/// (mutually exclusive in practice: the pipeline runner never calls both
/// for the same job, and a single-video job IS a 1-item pipeline run).
///
/// Streams events as `"transcribe-progress"` — the same channel name
/// `run_sidecar` already uses, now carrying an added `item_id` field so
/// the frontend can route pipeline events to the right queue row while
/// still working for the legacy single-shot path (which has no item_id).
#[tauri::command]
async fn transcribe_item(
    app: AppHandle,
    state: State<'_, RunningSidecar>,
    item_id: String,
    audio_path: String,
    model: String,
    timestamps: bool,
) -> Result<(), String> {
    {
        let mut guard = state.0.lock().unwrap();
        if let Some(prev) = guard.take() {
            eprintln!("transcribe_item: cancelling previous sidecar (pid={})", prev.pid());
            let _ = prev.kill();
        }
    }

    let binary = sidecar_path(&app)?;
    let (mut rx, child) = app
        .shell()
        .command(binary)
        .args([
            "--mode",
            "transcribe",
            "--audio-path",
            &audio_path,
            "--item-id",
            &item_id,
            "--model",
            &model,
            "--timestamps",
            if timestamps { "true" } else { "false" },
        ])
        .spawn()
        .map_err(|e| format!("transcribe_item: failed to spawn sidecar: {e}"))?;

    let pid = child.pid();
    eprintln!("transcribe_item: item={item_id} pid={pid}");
    state.0.lock().unwrap().replace(child);

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<serde_json::Value>(line) {
                        Ok(value) => {
                            let _ = app.emit("transcribe-progress", value);
                        }
                        Err(_) => {
                            eprintln!("transcribe_item[item={item_id}]: non-JSON line: {line}");
                        }
                    }
                }
            }
            CommandEvent::Stderr(bytes) => {
                eprintln!(
                    "transcribe_item[item={item_id}][stderr]: {}",
                    String::from_utf8_lossy(&bytes)
                );
            }
            CommandEvent::Terminated(payload) => {
                eprintln!(
                    "transcribe_item[item={item_id}]: terminated code={:?} signal={:?}",
                    payload.code, payload.signal
                );
                let _ = app.emit(
                    "transcribe-progress",
                    serde_json::json!({
                        "event": "terminated",
                        "item_id": item_id,
                        "code": payload.code,
                        "signal": payload.signal,
                    }),
                );
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(RunningSidecar::default())
        .manage(RunningDownloads::default())
        .invoke_handler(tauri::generate_handler![
            run_sidecar,
            cancel_transcribe,
            probe_url,
            probe_url_page,
            get_cached_probe,
            cache_probe,
            invalidate_probe,
            save_instagram_cookies,
            has_instagram_cookies,
            clear_instagram_cookies,
            log_error,
            write_crash_log,
            force_quit,
            save_transcript,
            load_history,
            delete_transcript,
            clear_history,
            load_settings,
            save_settings,
            save_job,
            load_jobs,
            delete_job,
            retry_job_item,
            start_job,
            update_item_phase,
            update_item_result,
            update_item_error,
            finalize_job,
            load_active_job,
            cleanup_orphan_downloads,
            download_item,
            cancel_download,
            transcribe_item
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod cookie_header_tests {
    use super::parse_cookie_header_to_netscape;

    #[test]
    fn parses_multiple_pairs_into_netscape_lines() {
        let out = parse_cookie_header_to_netscape("sessionid=abc123; csrftoken=xyz789")
            .expect("should parse");
        assert!(out.contains(".instagram.com\tTRUE\t/\tTRUE\t9999999999\tsessionid\tabc123"));
        assert!(out.contains(".instagram.com\tTRUE\t/\tTRUE\t9999999999\tcsrftoken\txyz789"));
        assert!(out.starts_with("# Netscape HTTP Cookie File"));
    }

    #[test]
    fn handles_extra_whitespace_and_trailing_semicolon() {
        let out = parse_cookie_header_to_netscape("  sessionid = abc123 ;  csrftoken=xyz789;  ")
            .expect("should parse");
        assert!(out.contains("sessionid\tabc123"));
        assert!(out.contains("csrftoken\txyz789"));
    }

    #[test]
    fn rejects_empty_input() {
        let err = parse_cookie_header_to_netscape("").unwrap_err();
        assert!(err.contains("no cookies found"));
    }

    #[test]
    fn rejects_input_with_no_valid_pairs() {
        let err = parse_cookie_header_to_netscape("; ; garbage-no-equals ;").unwrap_err();
        assert!(err.contains("no cookies found"));
    }

    #[test]
    fn cookie_value_containing_equals_sign_is_preserved() {
        // Some cookie values (e.g. base64/JSON-ish) legitimately contain '='.
        // split_once keeps everything after the FIRST '=' as the value.
        let out = parse_cookie_header_to_netscape("token=a=b=c").expect("should parse");
        assert!(out.contains("token\ta=b=c"));
    }
}