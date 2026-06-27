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
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

mod db;

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

    let (mut rx, child) = app
        .shell()
        .sidecar("transcribe-sidecar")
        .map_err(|e| format!("sidecar not found: {e}"))?
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
    (url.contains("youtube.com/watch?v=") || url.contains("youtu.be/"))
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
async fn probe_via_sidecar(app: &AppHandle, url: &str) -> serde_json::Value {
    let sidecar_cmd = match app
        .shell()
        .sidecar("transcribe-sidecar")
        .map(|c| c.args(["--mode", "probe", "--url", url]))
    {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"type": "error", "code": "INTERNAL", "message": format!("sidecar not found: {e}")}),
    };
    let (mut rx, child) = match sidecar_cmd.spawn() {
        Ok(v) => v,
        Err(e) => return serde_json::json!({"type": "error", "code": "INTERNAL", "message": format!("spawn failed: {e}")}),
    };
    eprintln!("probe_via_sidecar: pid={}", child.pid());
    let result = tokio::time::timeout(Duration::from_secs(45), async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        let t = line.trim();
                        if !t.is_empty() {
                            return serde_json::from_str::<serde_json::Value>(t)
                                .unwrap_or_else(|_| serde_json::json!({"type": "error", "code": "INTERNAL", "message": format!("bad JSON: {t}")}));
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => eprintln!("probe[stderr]: {}", String::from_utf8_lossy(&bytes)),
                CommandEvent::Terminated(p) => { eprintln!("probe: terminated {:?}", p.code); break; }
                _ => {}
            }
        }
        serde_json::json!({"type": "error", "code": "INTERNAL", "message": "probe exited with no output"})
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
        probe_via_sidecar(&app, &url).await
    } else if is_yt_playlist(&url) {
        eprintln!("probe_url: taking sidecar path (playlist)");
        probe_via_sidecar(&app, &url).await
    } else {
        eprintln!("probe_url: taking sidecar path (unknown platform)");
        probe_via_sidecar(&app, &url).await
    };
    eprintln!("probe_url: result type={}", result.get("type").and_then(|t| t.as_str()).unwrap_or("?"));
    Ok(result)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(RunningSidecar::default())
        .invoke_handler(tauri::generate_handler![
            run_sidecar,
            cancel_transcribe,
            probe_url,
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
            retry_job_item
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}