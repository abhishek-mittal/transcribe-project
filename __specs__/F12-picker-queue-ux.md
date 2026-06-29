# F12 — Picker + Queue UX: Load More, Transcribed State, Queue Fix, Active Row

## Priority
P1 — Depends on FIX-04 (entries work), FIX-06 (thumbnails), F11 (polish layer). These improvements target the two UX gaps visible in the app screenshots after FIX-04 was shipped.

---

## Current state

Two screenshots of the running app reveal four UX problems:

**Screenshot 1 — Picker:**
- Shows "Playlist: Modern Millie - Shorts", 20 rows, all checked
- **Problem A:** Count is always exactly 20. No way to load more. Channels may have 100+ videos but the probe caps at 20 with `playlistend: 20`.
- **Problem B:** Videos already transcribed (present in `history.json`) show identically to un-transcribed ones. User can select and re-queue them by accident.

**Screenshot 2 — Queue:**
- Shows "Queue · 0 of 1 complete", a single row with title `https://www.youtube.com/@MillieAdrian/shorts`, empty thumbnail, status "↓ Downloading..."
- **Problem C (root cause):** When a playlist probe result is showing and the user clicks the "Transcribe" button (not the picker's "Transcribe N videos →" button), `handleTranscribeViaQueue` fires. At that point `probeResult` is `null` (it's cleared when a playlist result comes in) and the code falls back to `{ url: trimmed, title: trimmed }` — creating a single-item job with the raw channel URL as both URL and title. The picker's selected entries are never used.
- **Problem D:** Even when the correct batch path fires, the active queue row has only subtle styling (`background: rgba(0,0,0,0.02)`) — essentially invisible. There is no "Processing video N of M" counter, no visual anchor for which item is currently running.

---

## After state — changes by component

### A. Load more in picker

| Location | Before | After |
|---|---|---|
| Picker footer | Nothing — list just ends at row 20 | "Load 20 more" button appears after the last row |
| Picker header | "20 videos" | "20 of 47 videos" (shows total from probe if available) |
| Probe result | `entries: [20 items]` | `entries: [20 items], total_count: 47` (new field) |

**How it works:**
1. The sidecar's `probe_url` already has `playlistend: 20`. Add a parallel `count_only` path that fetches the total playlist length cheaply (yt-dlp returns `playlist_count` in the root `info` object — no per-entry fetch needed).
2. The probe response gains `total_count: int | null` — populated when known, `null` for search results or channels where count isn't available without a full scan.
3. `VideoPicker` receives `totalCount` prop. If `totalCount > entries.length`, a "Load 20 more" button renders below the last row.
4. Clicking "Load 20 more" calls a new `probe_url_page` sidecar command with `playliststart` and `playlistend` offset. New entries are appended to the existing list (not replacing).
5. Once all entries are loaded (`entries.length >= totalCount`), the button disappears.

**Load more button text:**
- Shows remaining count: "Load 20 more (27 remaining)" or just "Load more" if total is unknown
- Replaces itself with a spinner while fetching
- On error: shows "Failed to load more. Retry?" in amber

---

### B. Already-transcribed state in picker

| Location | Before | After |
|---|---|---|
| Picker row — transcribed video | Normal row, fully selectable | Greyed row, checkbox disabled, "✓ Transcribed" badge on right |
| Picker header count | "3 of 20 selected" | "3 of 17 selectable (3 already transcribed)" |
| Select All | Selects all 20 | Selects only the 17 un-transcribed ones |

**How it works:**
1. `+page.svelte` already loads `historyRecords` from `load_history` on mount. Each record has `url: string`.
2. Extract a `Set<string>` of already-transcribed URLs from `historyRecords`. Pass it as `transcribedUrls` prop to `UrlInputPanel`, which passes it to `VideoPicker`.
3. `VideoPicker` receives `transcribedUrls: Set<string>` prop. For each entry, compute `isTranscribed = transcribedUrls.has(entry.url)`.
4. Transcribed rows:
   - Checkbox is `disabled`
   - Row has `pointer-events: none` and `opacity: 0.45`
   - Right side shows: `✓ Transcribed` in green, 11px
   - Cannot be toggled — clicking does nothing
5. `selectedIds` is initialised excluding transcribed entries (only un-transcribed start selected).
6. Select All toggles only the non-transcribed entries.
7. Count label: `"3 of 17 selected"` where 17 = total minus transcribed count.

**Visual:**
```
☑  [thumb]  Video title here                          ✓ Transcribed
   (greyed row, disabled checkbox, muted title)
```

---

### C. Queue job creation fix (root cause)

**Root cause:** In `UrlInputPanel.svelte`, when the probe returns `playlist` or `search` type, the code sets `probeState = 'idle'` and `probeResult = null` (lines 86–87). This is correct — the picker is showing, not the single-video preview. But the "Transcribe" button in picker mode still calls `handleTranscribe()` → dispatches `transcribe` event → `+page.svelte` calls `handleTranscribeViaQueue()` → sees `probeResult === null` → falls back to `{ url: trimmed, title: trimmed }` as a single item.

**Fix:**

In `UrlInputPanel.svelte`, the "Transcribe N videos →" button in picker mode should dispatch `transcribePicker` — not `transcribe`. The handler `handleTranscribePicker` currently has an empty body:

```
/** Called when the user clicks "Transcribe X videos" in picker mode */
function handleTranscribePicker() {
  // Primary path is via VideoPicker startJob event.
}
```

The **correct path** is already wired: `VideoPicker` dispatches `startJob` → `UrlInputPanel`'s `handlePickerStartJob` relays it → `+page.svelte`'s `handleStartJob` receives `e.detail.selected` → `startBatchJob(selected)`.

The bug is that **clicking the "Transcribe N videos →" button does not trigger the picker's `startJob` dispatch** — it calls `handleTranscribePicker` which does nothing. The `VideoPicker` only dispatches `startJob` when the user clicks the button *inside* the picker component.

**Two-part fix:**

1. In `UrlInputPanel.svelte`: change the "Transcribe N videos →" button's `on:click` to call `handlePickerStartJob` with the picker's currently selected entries. The `VideoPicker` component needs to expose its selected entries via a `bind:` prop or a store, OR the button in `UrlInputPanel` should trigger the picker's own submit action.

   Simplest approach: Add `let selectedPickerEntries = []` in `UrlInputPanel`. Have `VideoPicker` emit `selectionChange` with the full selected array (not just count). Then `UrlInputPanel`'s "Transcribe N videos →" button calls `handlePickerStartJob(selectedPickerEntries)`.

   Currently `VideoPicker` only dispatches `selectionChange` with `{ count }`. Change it to `{ count, selected }` — passing the actual selected entries array.

2. Separately: guard `handleTranscribeViaQueue` in `+page.svelte` against firing when `activeView === 'picker'`. If somehow the transcribe event fires while in picker mode, it should be a no-op (do not fall back to single-item with the raw URL).

**After fix:**
- User pastes channel URL → picker appears with 20 rows, all checked
- User deselects 3 videos → "17 of 20 selected"
- User clicks "Transcribe 17 videos →"
- Queue appears with 17 rows, each showing individual video title + thumbnail
- "Queue · 0 of 17 complete"

---

### D. Active queue row — processing clarity

| Location | Before | After |
|---|---|---|
| Active row background | `rgba(0,0,0,0.02)` — invisible | Accent tint: `var(--accent)` at 8% opacity |
| Active row left border | None | 3px solid `var(--accent)` |
| Queue header during batch | "Queue · 3 of 20 complete" | "Queue · 3 of 20 complete  ·  Processing video 4 of 20" |
| Active row title | Normal weight | Bold (600) |
| Active row scrolls into view | No | Yes — on status change to `starting`, scroll the active row into view |

**Processing counter:**
- Add to `QueueView.svelte` header: when `!allTerminal`, show which item is currently active:
  ```
  Queue · 3 of 20 complete  ·  ▶ Video 4 of 20
  ```
  This uses the index of the item whose status is `starting | downloading | transcribing` within `items`.

**Active row scroll into view:**
- In `QueueView.svelte`, use a `$: activeIndex = items.findIndex(i => ['starting','downloading','transcribing'].includes(i.status))`. When `activeIndex` changes, call `.scrollIntoView({ behavior: 'smooth', block: 'nearest' })` on the active row element.

---

## E. Sequential processing — how the queue actually runs

The queue runner is **strictly sequential** — one video at a time, one sidecar process. This is intentional: faster-whisper is CPU-bound and parallel runs would thrash the machine.

The loop: for each `waiting` item → mark `downloading` → `await run_sidecar(url)` → sidecar streams events → item reaches `done`/`failed`/`terminated` → next item starts.

**This means:**
- Only ever one row is `active` at a time
- "▶ Video N of M" in the header is simply the index of the currently-active item
- Total time = sum of individual video times (no parallelism)

**Implications for the UI:**
- Waiting rows show `○ Waiting` — they haven't started yet
- The active row shows live download/transcription progress
- Done rows accumulate above the active row as the job progresses
- The queue finishes when the loop exits (all items terminal)

Do not add parallelism. Do not change the loop structure.

---

## F. Cancel single item — already implemented, needs UI clarity

`cancelQueueItem(itemId)` already exists and handles both cases correctly:

- **Waiting item:** marks `cancelled` immediately. No sidecar call needed — the item hasn't started.
- **Active item:** marks `cancelled` locally, calls `cancel_transcribe` to kill the sidecar process, then the loop's `await run_sidecar` resolves (via the `terminated` event), and the loop advances to the next `waiting` item automatically.

**Do not re-implement this logic.** It works correctly.

**What needs to change:** the cancel button's visibility on the active row. Currently it only appears on row hover and is very small (11×11px, `opacity: 0` until hover). On the active row, this should be:

- Always visible (not hover-only)
- Labelled "Skip →" instead of just `×` — so the user knows it skips to the next video, not cancels the whole job
- Positioned clearly at the right edge of the active row's status column

For **waiting rows**, the `×` remove button is correct as-is (hover-only, small) — those items haven't started so there's no urgency.

**Queue-level Cancel job** button (top right) remains unchanged — it marks all remaining `waiting` items as cancelled and kills the active sidecar. This cancels the entire job.

| Action | Button | Behaviour |
|---|---|---|
| Skip current video | "Skip →" on active row (always visible) | Kills sidecar, marks item cancelled, loop continues to next |
| Remove from queue | `×` on waiting row (hover-only) | Marks item cancelled, no sidecar call |
| Cancel entire job | "Cancel job" top-right button | Marks all waiting cancelled, kills sidecar, loop exits |

---

## Target components

1. **`api/sidecar.py`** — `probe_url` function: add `total_count` field to playlist/search response. Add new sidecar command `probe_url_page` that accepts `url`, `page_start`, `page_end` and returns additional entries.
2. **`src/lib/desktop/VideoPicker.svelte`** — add `transcribedUrls` prop, `totalCount` prop, transcribed row state, Load More button, fix `selectionChange` event to include `selected` array.
3. **`src/lib/desktop/UrlInputPanel.svelte`** — pass `transcribedUrls` and `totalCount` to picker, fix "Transcribe N videos →" button to use picker's selected entries, add `loadMoreEntries` handler.
4. **`src/routes/+page.svelte`** — pass `transcribedUrls` down, guard `handleTranscribeViaQueue` in picker mode, handle `loadMore` event.
5. **`src/lib/desktop/QueueView.svelte`** — stronger active row styling, "Processing video N of M" in header, active row scroll into view.
6. **`src-tauri/src/lib.rs`** — expose new `probe_url_page` Tauri command (or extend `probe_url` to accept optional offset params).

---

## Before / after flows

### Paste a channel URL — load more

**Before:**
1. Paste `https://www.youtube.com/@MillieAdrian/shorts`
2. Picker shows 20 rows — no indication there are more
3. User doesn't know the channel has 80 more videos

**After:**
1. Paste `https://www.youtube.com/@MillieAdrian/shorts`
2. Picker shows "20 of 100 videos" in header
3. After the last row: "Load 20 more (80 remaining)" button
4. User clicks → rows 21–40 append to the list, button updates to "Load 20 more (60 remaining)"
5. User continues until all loaded or stops when satisfied

---

### Paste channel URL — transcribed videos marked

**Before:**
1. Paste channel URL
2. Picker shows 20 rows, all selectable
3. User selects all and queues — 5 of the videos were already transcribed
4. Those 5 re-run, wasting time

**After:**
1. Paste channel URL
2. Picker loads. 3 of 20 videos are already in history
3. Those 3 rows are greyed out, checkbox disabled, "✓ Transcribed" badge showing
4. Only the other 17 start selected
5. Header: "17 of 17 selectable (3 already transcribed)"
6. User clicks "Transcribe 17 videos →"
7. Queue has exactly 17 rows

---

### Paste channel URL — correct queue rows

**Before:**
1. Paste `https://www.youtube.com/@MillieAdrian/shorts`
2. Picker shows correctly
3. User clicks "Transcribe 20 videos →"
4. Queue shows 1 row: `https://www.youtube.com/@MillieAdrian/shorts` — the raw URL
5. No thumbnail. Status: "↓ Downloading..."

**After:**
1. Same paste and picker
2. User clicks "Transcribe 20 videos →"
3. Queue shows 20 rows, each with video thumbnail + individual title
4. Header: "Queue · 0 of 20 complete"
5. First row goes active — strong accent border, tinted background, "▶ Video 1 of 20" in header

---

### Active queue row clarity

**Before:**
- Processing row looks almost identical to waiting rows
- No header indicator of which video is running
- If list is long, the active row may be off-screen with no visual cue

**After:**
- Active row: 3px accent left border + accent-tinted background + bold title
- Header shows: "▶ Video 4 of 20" — always visible regardless of scroll position
- When a new video starts processing, the list smoothly scrolls to bring it into view

---

## Acceptance criteria

1. After probing a YouTube channel URL, the picker header shows "20 of N videos" where N is the total playlist count (if available from yt-dlp's `playlist_count` field).
2. A "Load 20 more" button appears below the last picker row when `entries.length < totalCount`. Clicking it fetches and appends the next 20 entries without replacing existing ones. Button disappears when fully loaded.
3. Videos already present in `load_history` results are rendered as greyed, non-selectable rows in the picker with a "✓ Transcribed" badge.
4. The Select All checkbox does not select transcribed rows.
5. Clicking "Transcribe N videos →" creates a queue with exactly N individual video rows — one per selected picker entry — each showing the video's own title and thumbnail, not the source URL.
6. The queue row count matches the picker selection count exactly.
7. No queue row ever shows a raw channel or playlist URL as its title.
8. The active queue row has a visible 3px left border in the accent colour and a tinted background that clearly distinguishes it from waiting rows.
9. The queue header shows "▶ Video N of M" while any item is processing.
10. When a new video starts processing, the list scrolls to bring the active row into view.
11. All of the above work with playlist, channel, and search result URLs.
12. Only one queue row is ever active at a time — the queue is sequential, not parallel.
13. The active row shows a "Skip →" button that is always visible (not hover-only). Clicking it kills the current sidecar, marks the item cancelled, and the queue runner automatically starts the next waiting item.
14. Waiting rows keep their existing hover-only `×` remove button — no change needed there.
15. "Cancel job" (top right) continues to cancel all remaining waiting items and kill the active sidecar — behaviour unchanged.

---

## Note for the coding agent

**On the queue fix (Problem C):** The simplest and most reliable fix is to change `VideoPicker`'s `selectionChange` event from `{ count: number }` to `{ count: number, selected: Array<entry> }`. Then `UrlInputPanel`'s "Transcribe N videos →" button calls `handlePickerStartJob(selectedPickerEntries)` directly. No new props or stores needed.

**On `total_count` from sidecar:** yt-dlp populates `info.get('playlist_count')` on the root playlist object when using `extract_flat='in_playlist'`. This is a cheap field — it comes back in the same `process=True` call already used by FIX-04. Just add it to the existing probe response dict: `"total_count": info.get("playlist_count")`. No additional network call needed.

**On `probe_url_page` for load more:** Accept `page_start: int` and `page_end: int` as optional params in a new sidecar command (or extend `probe_url` with optional offset fields). Pass them as `playliststart` and `playlistend` in yt-dlp opts. The Rust layer needs a new `probe_url_page` Tauri command that spawns the sidecar with these params.

**On transcribed URL matching:** Match by `entry.url` against `historyRecord.url`. These are the individual video URLs (e.g. `https://www.youtube.com/watch?v=ABC123`), not the channel URL — so the match is exact and reliable. Do not match on title.

**On the active row accent styling:** The current `active-row` class sets `background: rgba(0,0,0,0.02)`. Change it to use CSS variables:
```css
.queue-row.active-row {
  background: color-mix(in srgb, var(--accent) 8%, transparent);
  border-left: 3px solid var(--accent);
  padding-left: 11px; /* compensate for 3px border so content doesn't shift */
}
```

Design references: no design PNG for this spec — implement based on the before/after descriptions and existing component style patterns.
