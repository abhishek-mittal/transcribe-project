# F06 — Settings View + Model Selection

## Priority
P1 — Requires F04 (storage) complete. Model selection is independent of F05.

---

## Current state
The Settings sidebar item navigates to `activeView === 'settings'`, which renders a single stub: "Settings coming in v1.1." The app hardcodes `model = 'tiny'` in `+page.svelte` (line 15: `const model = 'tiny'`). There is no way for the user to change the transcription model, theme preference, or any other setting.

Settings are not persisted between sessions. Dark mode state (`darkMode`) is in-memory only — it resets to `false` on every app launch.

---

## After state
The Settings view lets the user:
1. Choose the Whisper model (tiny / base / small)
2. Set the default timestamp preference (on/off)
3. Toggle dark mode (and have it persist)
4. See storage info (how many transcripts saved, rough disk size estimate)
5. Clear all history

Settings are persisted to a local file alongside `history.json`:
```
~/Library/Application Support/com.shuhari.transcribe/settings.json
```

| Area | Before | After |
|---|---|---|
| Settings tab | Stub | Functional settings panel |
| Model | Hardcoded `tiny` | User-selectable: tiny / base / small |
| Dark mode | Resets to light on restart | Persists across sessions |
| Timestamps default | Resets to `true` on restart | Persists across sessions |
| History storage | No info shown | Record count + estimated size shown |

---

## Target components

1. **`src/lib/desktop/SettingsView.svelte`** — new component. Renders all settings sections.
2. **`src-tauri/src/lib.rs`** — two new Rust commands: `load_settings` and `save_settings`.
3. **`src/routes/+page.svelte`** — load settings on mount; replace hardcoded `model` with reactive value from settings; apply persisted dark mode and timestamps preference.

---

## Settings file shape
```json
{
  "version": 1,
  "model": "tiny",
  "timestamps": true,
  "dark_mode": false,
  "max_history_records": 500
}
```

---

## SettingsView layout

### Section 1 — Transcription
**Model** heading + description:
> "Larger models are more accurate but slower and use more disk space. The model is downloaded once on first use."

Three option cards (radio-style selection):

| Option | Download size | Speed (5 min video) | Notes |
|---|---|---|---|
| **Tiny** (default) | ~75 MB | ~1–2 min | Fast, good for most content |
| **Base** | ~145 MB | ~2–4 min | Better accuracy, slower |
| **Small** | ~460 MB | ~5–10 min | Best accuracy, significantly slower |

Selected card has a highlighted border. Changing selection immediately calls `save_settings` with the new model. The next transcription uses the updated model.

**Timestamps** toggle — "Include timestamps by default" — mirrors the toggle in `UrlInputPanel`. Persisted.

---

### Section 2 — Appearance
**Theme** — two buttons: "Light" / "Dark" (or a system-follow option if desired). Persisted. Changing it immediately applies `darkMode` in `+page.svelte`.

---

### Section 3 — Storage
**History** heading.
- Shows: "X transcriptions · approximately Y MB"
- Button: "Clear all history" — confirmation dialog ("Are you sure? This cannot be undone.") → calls `delete_transcript` for all records OR a new `clear_history` Rust command.

---

## Rust commands to implement

### `load_settings`
Reads `settings.json`. If the file does not exist, returns the default settings object. Never errors.

### `save_settings`
Accepts the full settings object. Writes it to `settings.json`. Returns ok.

---

## Startup behaviour
In `+page.svelte` `onMount`, after Tauri APIs load:
1. Call `load_settings` → apply `darkMode`, `model`, `timestamps`, `max_history_records`
2. Call `load_history` → populate history list

The `model` constant becomes a reactive `let model = 'tiny'` (mutable). When settings change it, the next `invoke('run_sidecar', { model, ... })` picks up the new value automatically.

---

## Before / after user flow

**Before:**
1. User opens Settings → sees stub

**After:**
1. User opens Settings
2. Sees model cards (Tiny selected by default)
3. Clicks "Base" → selection highlights; settings saved; next transcription uses base model
4. Toggles dark mode → theme switches immediately; preference saved; next launch opens in dark mode
5. Sees "24 transcriptions · approx. 1.2 MB"
6. Clicks "Clear all history" → confirmation dialog → confirms → history cleared; count resets to 0

---

## Acceptance criteria
1. Open Settings — all three sections render correctly.
2. Select "Base" model. Start a transcription. The sidecar is invoked with `--model base`.
3. Close and reopen the app. The model selection is still "Base".
4. Toggle dark mode in Settings. The app switches theme immediately. Reopen the app — dark mode is still active.
5. Toggle the timestamps default off. Navigate to Transcribe tab — the timestamps toggle is pre-set to off.
6. Storage section shows a correct record count (matches what `load_history` returns).
7. "Clear all history" shows a confirmation dialog before deleting.
8. After clearing, `load_history` returns an empty array and History tab shows its empty state.
9. Changing the model in Settings does not affect a transcription already in progress.

---

## Note for the coding agent
The Python sidecar already accepts `--model <name>` as a CLI argument (see `api/sidecar.py`). Passing `--model base` or `--model small` will cause it to download the appropriate faster-whisper model to `~/Library/Caches/transcribe-app/models/<name>/` on first use. No sidecar changes are needed — only the Rust invocation argument needs to pass the selected model name.
