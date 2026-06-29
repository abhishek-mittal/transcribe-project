# F04 — Transcript Storage (History Persistence)

## Priority
P0 — Required for History and Settings to function. Must ship before F05 and F06.

---

## Current state
No storage exists. Every transcript exists only in memory during the session. When the user closes the app or starts a new transcription, the previous transcript is lost. The History tab shows a static stub: "Recent transcriptions will appear here." There is no way to retrieve a past transcript.

The Rust layer (`lib.rs`) currently manages only the running sidecar process. The `tauri-plugin-fs` is already registered — it can read and write files. No database, no history file, no Rust command for saving records.

---

## After state
Every completed transcription is automatically saved to a local JSON store on the user's machine at:
```
~/Library/Application Support/com.shuhari.transcribe/history.json
```

The store is an ordered array of transcript records (newest first). The app exposes two Rust commands to the frontend: one to save a record, one to load all records.

| What | Before | After |
|---|---|---|
| Completed transcript | Lost on session end | Saved automatically to history.json |
| History tab | Static stub | Live list of past transcripts |
| App restart | No transcript data | Past transcripts load from disk |
| User deletes a record | Not possible | Record removed from history.json |

---

## Target components

1. **`src-tauri/src/lib.rs`** — two new Rust commands: `save_transcript` and `load_history`, plus a `delete_transcript` command
2. **`src-tauri/Cargo.toml`** — no new dependencies needed (serde, serde_json, tauri-plugin-fs all already present)
3. **`src-tauri/capabilities/default.json`** — no permission change needed (fs:default already present)
4. **`src/routes/+page.svelte`** — call `save_transcript` when a result arrives (after the `result` event is handled)

---

## Data shape

Each record saved to `history.json`:
```json
{
  "id": "uuid-v4-string",
  "url": "https://youtube.com/watch?v=...",
  "title": "optional-human-readable-title",
  "language": "en",
  "plain": "Full plain text transcript...",
  "timestamped": "[00:01] Full text with timestamps...",
  "srt": "1\n00:00:01,000 --> 00:00:03,000\nFirst subtitle\n\n...",
  "model": "tiny",
  "word_count": 1240,
  "created_at": "2026-06-27T14:30:00Z"
}
```

`history.json` top-level shape:
```json
{
  "version": 1,
  "records": [ ...record objects, newest first... ]
}
```

---

## Rust commands to implement

### `save_transcript`
Accepts a full transcript record (all fields above except `id` and `created_at` which are generated server-side). Reads `history.json`, prepends the new record, writes it back. Returns the new record's `id`.

### `load_history`
Reads `history.json` and returns the `records` array. If the file does not exist, returns an empty array (do not error).

### `delete_transcript`
Accepts a record `id`. Reads `history.json`, removes the matching record, writes it back. Returns ok.

---

## Storage path
Use `tauri::path::AppDataDir` (the standard Tauri helper) to resolve `~/Library/Application Support/com.shuhari.transcribe/`. The file is named `history.json`. Create the directory if it does not exist.

---

## Auto-save behaviour
In `+page.svelte`, inside the `handleSidecarEvent` function, in the `event === 'result'` branch — after setting `result` — call `invoke('save_transcript', { record: { url, language, plain, timestamped, srt, model, word_count } })`. Fire-and-forget (do not block the UI on the save completing).

---

## Acceptance criteria
1. Complete a transcription. Close and reopen the app.
2. The saved transcript is present when `load_history` is called on startup.
3. Complete five transcriptions. All five appear in the history store, newest first.
4. Delete a record via `delete_transcript`. It no longer appears in subsequent `load_history` calls.
5. `history.json` exists at the correct path and is valid JSON.
6. If `history.json` is missing or corrupted, `load_history` returns an empty array without crashing the app.
7. The URL and language fields saved in the record match what was actually transcribed.

---

## Note
Keep the history.json file size manageable. Trim to a maximum of 500 records (discard oldest) on each save. This limit can be made user-configurable later (F06).
