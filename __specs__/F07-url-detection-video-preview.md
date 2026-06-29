# F07 — URL Type Detection + Single Video Preview

## Priority
P0 — Foundation for all batch work. F08 and F09 depend on the detection layer built here.

---

## Current state

`UrlInputPanel` accepts a URL string and immediately passes it to `run_sidecar` on click. There is no metadata fetch before transcription starts. The user has no confirmation of what they're about to transcribe — no title, no thumbnail, no duration. Single video and playlist URLs are treated identically.

`+page.svelte` hardcodes `model = 'tiny'` (now reactive via Settings, but the Transcribe tab shows no indication of the active model).

The Python sidecar (`api/sidecar.py`) currently only handles single-URL transcription. There is no Python endpoint for URL type detection or metadata extraction.

---

## After state

When the user pastes a URL and either presses Enter or waits 800ms after stopping typing, the app performs a fast metadata probe. Based on the result:

- **Single video** → a preview card appears below the URL input showing thumbnail, title, channel, and duration. The "Transcribe" button becomes active. User clicks → single-video transcription runs (existing flow, no change).
- **Playlist / page / channel** → the right pane switches to the video picker (F08). The left pane shows a count: "Found 24 videos".

| Area | Before | After |
|---|---|---|
| URL input | Paste → click Transcribe immediately | Paste → metadata probe → preview or picker |
| Single video | No preview, blind transcription | Thumbnail + title + duration shown before start |
| Playlist URL | Treated as single video, fails or transcribes first entry | Triggers video picker (F08) |
| Model indicator | Not shown on Transcribe tab | Active model shown as a small badge below the Transcribe button |
| Left pane during probe | No feedback | "Checking URL…" status with spinner |

---

## Target components

1. **`api/sidecar.py`** — new entry point: `probe` mode. When called with `--mode probe --url <url>`, returns a single JSON object to stdout describing the URL type and metadata. Does not download audio or transcribe.
2. **`src-tauri/src/lib.rs`** — new Rust command: `probe_url`. Spawns the sidecar in probe mode, reads stdout, returns the JSON result to the frontend. Does not use the single-flight mutex (probe is lightweight and non-cancellable).
3. **`src/lib/desktop/UrlInputPanel.svelte`** — debounced probe trigger on URL change; preview card rendering; model badge.
4. **`src/routes/+page.svelte`** — handle probe result: set `probeResult`; if playlist, switch `activeView` to `'picker'` (new view added in F08).

---

## Python sidecar — probe mode

### New CLI mode
```
transcribe-sidecar --mode probe --url <url>
```

Writes exactly one JSON object to stdout, then exits with code 0. Never writes multiple lines. No streaming events.

### Probe logic
1. Call `yt_dlp.YoutubeDL(quiet=True).extract_info(url, download=False, process=True)` — this is the fast metadata-only path, no audio download.
2. Inspect the result:
   - If `result.get('_type') == 'playlist'` or `result.get('entries')` is present → type is `playlist`.
   - Otherwise → type is `video`.
3. For `video`: extract `title`, `thumbnail` (highest-res square or 16:9 URL), `duration` (seconds), `uploader` (channel name), `webpage_url`.
4. For `playlist`: extract `title`, `uploader`, `webpage_url`, and `entries` as a flat list of `{ id, title, thumbnail, duration, webpage_url }`. Use `extract_flat='in_playlist'` to avoid downloading full metadata for every entry — this makes the probe fast even for 100-video playlists.
5. Emit one JSON object:

```json
{
  "type": "video",
  "url": "https://...",
  "title": "Video title here",
  "thumbnail": "https://i.ytimg.com/...",
  "duration": 312,
  "uploader": "Channel Name"
}
```

Or for playlists:

```json
{
  "type": "playlist",
  "url": "https://...",
  "title": "Playlist name",
  "uploader": "Channel Name",
  "count": 24,
  "entries": [
    { "id": "abc123", "title": "Video 1", "thumbnail": "https://...", "duration": 180, "url": "https://youtube.com/watch?v=abc123" },
    ...
  ]
}
```

### Error output
If the probe fails (network error, unsupported URL, bot challenge), emit:
```json
{ "type": "error", "code": "NETWORK", "message": "..." }
```
Use the same error codes as the existing sidecar (`NETWORK`, `BOT_CHALLENGE`, `UNSUPPORTED_PLATFORM`, `INVALID_URL`, `INTERNAL`).

### Performance target
Probe for a single YouTube video: under 3 seconds on a normal residential connection. Playlist probe (flat extract): under 5 seconds for up to 100 entries.

### Best practices
- Use `yt_dlp.YoutubeDL` with `quiet=True`, `no_warnings=True`, `skip_download=True` — never download audio in probe mode.
- Use `extract_flat='in_playlist'` when probing playlists to avoid fetching full per-video metadata.
- Apply the same `desktop_mode=True` logic as `download_audio` — no cookies, no proxy, no player-client override.
- Wrap the entire probe in a try/except and always emit exactly one JSON line, even on failure.
- Add `socket_timeout: 8` to yt-dlp options to prevent hanging on slow connections.

---

## Rust — `probe_url` command

```rust
#[tauri::command]
async fn probe_url(app: AppHandle, url: String) -> Result<serde_json::Value, String>
```

- Spawns `transcribe-sidecar --mode probe --url <url>`.
- Reads the first non-empty stdout line.
- Parses it as JSON and returns it to the frontend.
- Timeout: if no stdout within 12 seconds, kill the process and return `{ "type": "error", "code": "INTERNAL", "message": "probe timed out" }`.
- Does NOT use `RunningSidecar` mutex — probe is a separate lightweight process.
- Does NOT emit Tauri events — just returns the value directly to the caller.

Register `probe_url` in `invoke_handler` alongside existing commands.

---

## Frontend — UrlInputPanel probe flow

### Debounce
On every change to the URL input, start a 800ms debounce timer. On expiry, if the URL starts with `http://` or `https://`, call `invoke('probe_url', { url })`.

Cancel any pending probe if the user clears the URL or starts a new one before the debounce fires.

### States
```
idle       → user hasn't pasted anything
probing    → debounce fired, waiting for probe_url result
preview    → single video, show preview card
error      → probe returned error, show inline error
```

### Preview card (single video)
Appears below the URL input inside `UrlInputPanel`. Contains:
- Thumbnail image (16:9, full width of the panel, border-radius 8px). If thumbnail URL fails to load, show a grey placeholder.
- Title (13.5px, 500 weight, max 2 lines, ellipsis overflow).
- Channel name + duration (12px, `var(--text-3)`). Duration formatted as `MM:SS` or `H:MM:SS`.
- If probe fails: no preview card, show inline error below the URL input in the existing error style.

### Model badge
Below the Transcribe button in the action bar, show a small line: `Model: Tiny` (or Base, Small — reads from the current `model` prop). Tapping it navigates to Settings. This is read-only on the Transcribe tab — changing model happens in Settings only.

### Playlist detection
If probe returns `type === 'playlist'`, `UrlInputPanel` emits a `playlist` event upward with the probe result payload. `+page.svelte` handles it by storing the entries and switching `activeView` to `'picker'`.

---

## Before / after user flow

**Single video — before:**
1. Paste YouTube URL
2. Click Transcribe
3. App blindly starts downloading

**Single video — after:**
1. Paste YouTube URL
2. Wait ~1s — preview card appears: thumbnail + "Never Gonna Give You Up · Rick Astley · 3:32"
3. Click Transcribe
4. Same transcription flow as before

**Playlist — before:**
1. Paste playlist URL
2. Click Transcribe
3. Sidecar attempts to transcribe the playlist URL as a single video — unpredictable result

**Playlist — after:**
1. Paste playlist URL
2. Wait ~2s — left pane shows "Found 24 videos · My Playlist"
3. Right pane switches to video picker (F08)

---

## Data flow summary

```
User pastes URL
  → 800ms debounce
  → invoke('probe_url', { url })          [JS → Rust]
  → sidecar --mode probe --url <url>      [Rust → Python]
  → stdout: one JSON line                 [Python → Rust]
  → return JSON to frontend               [Rust → JS]
  → if video: show preview card
  → if playlist: emit 'playlist' event → +page.svelte switches to picker view (F08)
  → if error: show inline error
```

---

## Acceptance criteria

1. Paste a single YouTube URL — within 3 seconds a preview card appears with the correct title, thumbnail, and duration.
2. Paste a YouTube playlist URL — within 5 seconds the right pane switches to the video picker; the left pane shows the correct video count.
3. Paste an unsupported URL — an inline error appears: "This URL is not supported."
4. Paste a URL, then immediately change it — the first probe is cancelled; only the second probe fires.
5. Clear the URL — the preview card disappears immediately.
6. Probe completes and user clicks Transcribe — transcription starts using the same `run_sidecar` flow as before; the preview card remains visible during transcription.
7. The active model name is visible below the Transcribe button at all times.
8. Network is offline — probe returns a NETWORK error within 12 seconds (timeout); inline error appears.

---

## Note for the coding agent

The sidecar binary is built by PyInstaller and lives at `binaries/transcribe-sidecar`. The new `--mode probe` flag is an addition to the existing CLI argument parser in `api/sidecar.py`. The existing `--url / --model / --timestamps` flow for transcription is unchanged — this is purely additive. Add `--mode` as an optional arg that defaults to `'transcribe'` to preserve backward compatibility.

Thumbnail images are loaded via a standard `<img>` tag in the webview. They are remote HTTPS URLs (e.g. YouTube's `i.ytimg.com` CDN). The current CSP (`default-src 'self' ipc: http://ipc.localhost`) blocks these. Before rendering thumbnails, add `img-src 'self' https: data:` to the CSP in `src-tauri/tauri.conf.json`. This is the only required CSP change.
