# F11 — UI Improvements: Probe UX, Picker Polish, Queue Row, Error States

## Priority
P1 — Depends on FIX-04, FIX-05, FIX-06. These improvements assume URLs probe correctly and thumbnails are present. Polish layer on top of the working data flow.

---

## Current state

The app's core probe → picker → queue flow works but several UI details make it feel rough:

1. **Probe loading state** is unclear — there's a spinner but no label. Users don't know if it's checking the URL or already downloading.
2. **Video picker** has no visual distinction between "all selected" and "some selected". The Select All checkbox has no indeterminate state.
3. **Queue rows** show status text only — the active row doesn't visually stand out enough from waiting rows.
4. **Error inline messages** for unsupported URLs show raw error codes, not human sentences.
5. **Duration "0"** shows as `0:00` for videos where yt-dlp returns null duration (YouTube Shorts under flat-extract). Should show `—` instead.
6. **Right pane split** on Transcribe tab always takes the same fixed width regardless of whether a preview card or picker is showing. On narrow windows this feels cramped.
7. **Queue tab badge dot** does not disappear when the job finishes — it keeps pulsing even after "All done" state.

---

## After state — changes by component

### A. UrlInputPanel — probe states

**Idle state (no URL):**
- Placeholder text: "Paste a YouTube or Instagram URL…"
- No spinner, no error

**Probing state (debounce fired, waiting for result):**
- Input border turns accent colour (subtle)
- Below input: small animated label: "Checking URL…" in `var(--text-3)`, 12px
- Transcribe button is disabled and greyed out during probe

**Single video preview — after probe:**
- Preview card appears below input with thumbnail (16:9, full left-pane width), title (2 lines max), channel name + duration on one line
- Duration `0` → renders as `—` (unknown)
- Transcribe button becomes active

**Playlist result — after probe:**
- Left pane shows: "Found N videos · [Playlist title]" in `var(--text-3)`
- Right pane switches to `VideoPicker`
- Transcribe button replaced with "Transcribe N videos →" (disabled until ≥1 selected)

**Error state — after probe:**
- `INVALID_URL` → "This URL isn't supported. Try a YouTube or Instagram link."
- `IG_PROFILE_UNSUPPORTED` → amber (warning) style: "Instagram profile pages aren't supported. Paste an individual reel URL — e.g. instagram.com/reel/ABC123"
- `NETWORK` → "Can't reach the server. Check your internet connection."
- `BOT_CHALLENGE` → "YouTube is blocking this request. Try a different video."
- All other codes → "Something went wrong. Try again."

Style: error messages use `var(--error)` text colour at 12px, appearing immediately below the URL input with no icon needed. The `IG_PROFILE_UNSUPPORTED` warning uses `var(--warning)` or amber.

---

### B. VideoPicker — selection polish

**Select All checkbox — indeterminate state:**
- All selected → checked ✓
- Some selected → indeterminate (–) — set `input.indeterminate = true` in Svelte after mount
- None selected → unchecked □

**Row hover:**
- Whole row is clickable (not just checkbox)
- Hover: subtle background `var(--surface-2)` or accent at 5% opacity
- Selected row: accent at 10% opacity background

**Duration display:**
- `duration === 0` or `null` → show `—` instead of `0:00`
- `duration > 0` → format as `M:SS` (under 1 hour) or `H:MM:SS`

**Count label:**
- "24 videos" when all selected
- "3 of 24 selected" when partial

**Picker header:**
```
□ Select all   ·   3 of 24 selected
```
The "Select all" label is a clickable label (not just the checkbox) — clicking the label text also toggles all.

**Empty state (0 entries returned):**
```
No videos found.
Try a different URL or search query.
```

**Playlist title strip** (above the list, below the header row):
```
[channel icon placeholder] Playlist Title  ·  Channel Name
```
12px, `var(--text-3)`. Truncated to one line.

---

### C. QueueView — row and status polish

**Active row (currently downloading or transcribing):**
- Left border: 3px accent colour stripe
- Background: accent at 4% opacity
- Title text: `var(--text)` (full opacity, not dimmed)
- Status column: animated — for `downloading`: a small horizontal progress fill bar (thin, 2px, accent) that fills from the actual download percent event. For `transcribing`: segment counter ticking up `"✦ 42 segs"` with a subtle pulse

**Waiting rows:**
- Dimmed: title at 60% opacity, status `○ Waiting` in `var(--text-3)`

**Done rows:**
- Check mark: green `✓`
- Status: `Done · EN · 1.2k words` — compact single line
- Clicking a done row highlights it (accent border left) and slides in right pane

**Failed rows:**
- `✗` in red
- Short error label inline: `Bot challenge`, `Network error`, `Login required`, `Unsupported`
- One line below the title (indented 8px): full plain-English explanation from error code map
- `[↺ Retry]` icon button at right edge

**Cancelled rows:**
- `—` dash in `var(--text-3)`
- Title dimmed to 50% opacity

**Topbar:**
```
Queue · 3 of 10 complete          [Cancel job]
```
When all done:
```
Queue · ✓ All done · 10 of 10    [View in History →]
```
Once the job finishes, the sidebar dot badge **stops pulsing and disappears** within 1 second. The Queue nav item remains visible (shows empty state on next visit) but loses the dot.

**Right pane slide-in:**
- Transition: `translateX(100%)` → `translateX(0)` over 200ms, ease-out
- Width: 360px fixed on desktop, or 40% of queue area — whichever is larger
- Clicking anywhere in the list area (not the right pane) closes the right pane
- Pressing `Escape` closes the right pane
- `TranscriptPanel` inside the right pane gets a close button `[×]` in its top-right corner

---

### D. Error code → human message map

Add a shared `errors.ts` file in `src/lib/` with a function `errorMessageFor(code: string): string`:

| Code | Short label (queue row) | Full message (expanded) |
|---|---|---|
| `BOT_CHALLENGE` | Bot challenge | YouTube blocked this request. Try a different video, or open it in your browser first. |
| `INSTAGRAM_LOGIN_REQUIRED` | Login required | Instagram requires login. Open Instagram in Safari and log in, then try again. Or place a cookies.txt file at ~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt |
| `IG_PROFILE_UNSUPPORTED` | Profile not supported | Instagram profile pages aren't supported. Paste an individual reel URL. |
| `NETWORK` | Network error | Can't reach the server. Check your internet connection and try again. |
| `UNSUPPORTED_PLATFORM` | Unsupported | This URL isn't supported. Try a YouTube or Instagram link. |
| `INVALID_URL` | Invalid URL | This doesn't look like a valid video URL. |
| `FFMPEG_MISSING` | FFmpeg missing | FFmpeg is not installed. Run: brew install ffmpeg |
| `MODEL_LOAD_FAILED` | Model error | Failed to load the transcription model. Check your internet connection and try again. |
| `INTERNAL` | Unexpected error | Something went wrong. Try again or restart the app. |

This map is used in: `UrlInputPanel` (probe errors), `QueueView` (failed row messages), and any other component that renders an error code.

---

### E. General polish

**Sidebar Queue badge:**
- Pulsing amber dot while job is running
- Dot disappears 1s after job reaches terminal state (all done/failed/cancelled)
- No number badge — just presence/absence of dot

**Model badge on Transcribe tab:**
- Shows active model name below Transcribe button: `Model: Tiny`
- Clicking it navigates to Settings tab
- Updates immediately when user changes model in Settings

**Transcribe button disabled states:**
- Disabled when: no URL pasted, probe in progress, probe returned error
- Active when: probe returned `video` type AND no active job is running
- Replaced by "Transcribe N videos →" button when probe returned `playlist`/`search` type

**Scroll behaviour:**
- `VideoPicker` list: `overflow-y: auto`, max-height fills available right pane space
- `QueueView` list: same — active row scrolls into view automatically when processing starts

---

## Target components

1. **`src/lib/desktop/UrlInputPanel.svelte`** — probe states, error messages, disabled states, playlist count display
2. **`src/lib/desktop/VideoPicker.svelte`** — indeterminate checkbox, hover/selected styles, duration fallback, count label, playlist title strip
3. **`src/lib/desktop/QueueView.svelte`** — active row accent, status labels, right pane slide-in, topbar done state, badge disappearance
4. **`src/lib/errors.ts`** — new file: `errorMessageFor(code)` and `errorLabelFor(code)` functions
5. **`src/lib/desktop/SidebarNav.svelte`** — badge dot disappears on job completion

---

## Before / after flows

**Paste YouTube channel URL:**

Before: spinner with no label → never resolves (FIX-04 fixes this) → even after fix, no description of what was found

After: "Checking URL…" label → "Found 20 videos · @MillieAdrian" → picker with thumbnails → "3 of 20 selected" → "Transcribe 3 videos →"

**Active queue row:**

Before: row looks same as waiting rows, hard to tell what's processing

After: accent left border + tinted background + live progress fill bar during download → segment counter during transcription

**Failed queue row:**

Before: raw error code in status column

After: "Bot challenge" short label + "YouTube blocked this request. Try a different video, or open it in your browser first." below the title + Retry button

---

## Acceptance criteria

1. During URL probe, "Checking URL…" label appears below the input within 200ms of the debounce firing.
2. After a playlist probe, left pane shows "Found N videos · [title]" and the picker shows all N rows with thumbnails.
3. Select All checkbox shows indeterminate state when some (not all) videos are selected.
4. Duration `0` or `null` shows as `—` in picker rows and queue rows.
5. Active queue row has a visible accent left border and tinted background distinguishing it from waiting rows.
6. Download progress shows as a thin fill bar on the active row (percent from download events).
7. Transcription shows as a segment counter ticking up on the active row.
8. Done rows: clicking opens transcript in right pane. Right pane slides in with 200ms ease-out animation.
9. Pressing Escape or clicking outside the right pane closes it.
10. Failed row shows short error label + full plain-English message below title + Retry button.
11. Sidebar dot badge disappears within 1s of job completing.
12. All error codes are mapped to human-readable messages — no raw error code strings are visible to the user anywhere in the UI.
13. `errors.ts` `errorMessageFor` function handles all codes in the table above plus returns a fallback for unknown codes.

---

## Note for the coding agent

`errors.ts` should be a pure TypeScript module with no Svelte or Tauri imports — it maps string codes to string messages only. Import it in any component that needs to display an error.

For the indeterminate checkbox state in Svelte, use a `bind:this={checkboxEl}` ref and set `checkboxEl.indeterminate = (selectedCount > 0 && selectedCount < totalCount)` in a reactive statement (`$:`). The `indeterminate` property cannot be set via HTML attributes — it requires direct DOM property assignment.

For the download progress bar: the `transcribe-progress` Tauri event includes `phase: 'downloading-audio'` and `percent: number`. Listen for this in `+page.svelte` and pass it to `QueueView` as `activeItemPercent: number`. The QueueView active row renders a `<div style="width: {activeItemPercent}%">` inside a thin track.

Do not change the probe debounce timing (800ms) or the probe timeout (12s) — only the visual states around them.
