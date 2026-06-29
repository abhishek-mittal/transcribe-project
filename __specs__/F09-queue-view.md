# F09 — Queue View (Live Batch Progress)

## Priority
P1 — Depends on F07 (single video) and F08 (batch job start). The queue view handles both single-video jobs (1-item queue) and batch jobs (N items).

---

## Current state

No queue concept exists. The app processes one video at a time via `run_sidecar`. Phase state (`downloading`, `transcribing`, `done`) is a single global variable. There is no way to queue multiple videos or track parallel progress.

The Rust `run_sidecar` command enforces single-flight via `Mutex<Option<CommandChild>>` — any new call kills the previous. This mechanism is preserved for the queue runner: only one sidecar runs at a time, managed by the queue processor in `+page.svelte`.

---

## After state

A Queue tab (with a live badge dot when active) shows all videos in the current job as a scrollable list. Each row shows the video's thumbnail, title, and current status. Completed rows are clickable — the right pane slides open with the transcript. Failed rows show a plain-English error and a Retry button. The job runs sequentially — one video at a time, top to bottom.

| Area | Before | After |
|---|---|---|
| Sidebar | Transcribe · History · Settings | Transcribe · Queue (·) · History · Settings |
| Queue tab | Does not exist | Live list of current job's videos |
| Left pane in Queue | Shows URL input (irrelevant) | Hidden — full width for the list + transcript |
| Right pane in Queue | Always TranscriptPanel | Slides in when user clicks a Done row |
| Single-video job | Phase shown in left pane status row | Also appears in Queue as a 1-row list |

---

## Target components

1. **`src/lib/desktop/QueueView.svelte`** — new component. Receives `jobItems[]` as a prop, renders the list, emits `retryItem` and `cancelItem` events.
2. **`src/lib/desktop/SidebarNav.svelte`** — add `queue` nav item between Transcribe and History. Show a pulsing dot badge when `queueActive` prop is true. Hide the item (or grey it out) when no job is running and the queue is empty.
3. **`src/routes/+page.svelte`** — queue state management: `jobItems[]`, sequential runner loop, wiring QueueView and TranscriptPanel together.
4. **`src-tauri/src/lib.rs`** — new Rust command: `save_job` (saves a completed job record for History, F10). Additive — no existing commands change.

---

## Queue state model (in +page.svelte)

```typescript
type JobItemStatus =
  | 'waiting'
  | 'downloading'
  | 'transcribing'
  | 'done'
  | 'failed'
  | 'cancelled';

type JobItem = {
  id: string;                // uuid, generated at job creation
  url: string;
  title: string;
  thumbnail: string;
  duration: number;          // seconds, from probe
  status: JobItemStatus;
  error: string | null;      // plain-English error message if failed
  errorCode: string | null;  // raw code (NETWORK, BOT_CHALLENGE, etc.)
  result: TranscriptResult | null;  // { language, plain, timestamped, srt }
  startedAt: string | null;  // ISO8601
  completedAt: string | null;
  wordCount: number | null;
};

type Job = {
  id: string;
  model: string;
  timestamps: boolean;
  createdAt: string;
  items: JobItem[];
};
```

`let currentJob: Job | null = null` in `+page.svelte`. Persisted to History when the job completes (F10).

---

## Sequential queue runner

The runner is a simple async loop in `+page.svelte`. It processes `currentJob.items` one by one in order:

```
for each item where status === 'waiting':
  1. Set item.status = 'downloading', item.startedAt = now
  2. invoke('run_sidecar', { url: item.url, model, timestamps })
     → listen for 'transcribe-progress' events
     → map phase events to item.status ('downloading' / 'transcribing')
     → on 'result' event: set item.status = 'done', item.result = payload, item.completedAt = now
       → invoke('save_transcript', { record: { ...item.result, url, title, model, word_count } })
     → on 'error' event: set item.status = 'failed', item.error = errorMessageFor(code), item.errorCode = code
  3. Continue to next item
```

On `cancel_job`: set all `waiting` items to `cancelled`, call `invoke('cancel_transcribe')` for the active item.

On `retry_item(id)`: reset the item to `waiting` (clear error, result, timestamps), then re-run the runner from that item.

The runner uses the existing `run_sidecar` single-flight command — calling it again auto-kills any previous sidecar. No new Rust concurrency logic needed.

**Reactivity:** `currentJob` is a Svelte reactive object. Mutations trigger re-renders automatically. Use `currentJob = { ...currentJob }` or Svelte's `$:` blocks to propagate updates to `QueueView`.

---

## QueueView component

### Props
```typescript
export let items: JobItem[] = [];
export let selectedItemId: string | null = null;
```

### Emits
```typescript
dispatch('selectItem', { id: string });
dispatch('retryItem', { id: string });
dispatch('cancelItem', { id: string });
dispatch('cancelJob');
```

### Layout
Full width (left pane hidden in Queue view). Right pane slides in when a Done item is selected.

```
┌─────────────────────────────────────────────────────────────────┐
│  topbar: "Queue · 3 of 10 complete"          [Cancel job]       │
├──────────────────────────────────────────────┬──────────────────│
│  # │ Thumb   │ Title                 │ Status │ (right pane)    │
│────┼─────────┼───────────────────────┼────────│                 │
│  1 │ [56×32] │ Video title here      │ ✓ Done │ TranscriptPanel │
│  2 │ [56×32] │ Another video         │ ✓ Done │ when item       │
│  3 │ [56×32] │ Currently processing  │ ▓░ 60% │ selected        │
│  4 │ [56×32] │ Queued next           │ ○ Wait │                 │
│  5 │ [56×32] │ Failed — bot block    │ ✗ [↺]  │                 │
│  6 │ [56×32] │ Queued                │ ○ Wait │                 │
└──────────────────────────────────────────────┴──────────────────┘
```

### Row anatomy
- **#**: 1-based position number (28px col, `var(--text-3)`)
- **Thumb**: 56×32px image, `object-fit: cover`, border-radius 4px. Grey placeholder on error.
- **Title**: Single line, ellipsis. 13px, 500 weight. `var(--text)`.
- **Status**: Right-aligned, contextual per state (see below). 120px col.

### Status column per state

| State | Display |
|---|---|
| waiting | `○ Waiting` in `var(--text-3)` |
| downloading | `↓ Downloading…` with subtle pulse |
| transcribing | Inline progress bar (thin, accent colour) + segment count ticking up `"✦ 42 segments"` |
| done | `✓ Done · EN · 1.2k words` in green-ish |
| failed | `✗ Error` in red + `[↺ Retry]` icon button |
| cancelled | `— Cancelled` in `var(--text-3)` |

### Row interactions
- **Done row**: Click anywhere → selects the row (highlighted), right pane slides in with TranscriptPanel loaded with that item's result.
- **Failed row**: Retry button (↺) → emits `retryItem`. Hovering shows the plain-English error in a tooltip or inline beneath the title.
- **Waiting row**: Hover shows `[×]` cancel icon on the right to remove from queue.
- **Active row**: Non-clickable, shows live status only.

### Right pane — transcript slide-in
When a Done row is selected:
- Right pane slides in from the right (translate-X transition, 200ms).
- Shows `TranscriptPanel` with the item's result pre-loaded, the video title at the top, and all existing tab/copy/save actions working.
- Deselecting (clicking elsewhere, pressing Escape) slides the right pane back out.
- If no item is selected: right pane is hidden and the list occupies full width.

### Topbar
```
"Queue · 3 of 10 complete"   [Cancel job]
```
Once all items are done/failed/cancelled:
```
"Queue · ✓ All done · 10 of 10"   [View in History]
```
"View in History" navigates to `activeView = 'history'` and scrolls to the job just completed.

### Empty state (no job running)
```
No active job.
Start one from the Transcribe tab.
```

---

## Sidebar — Queue nav item

Add `queue` between `transcribe` and `history` in `SidebarNav.svelte`:

```svelte
{ id: 'queue', label: 'Queue', icon: 'queue' }
```

**Badge dot**: When `queueActive` is true (job is running), show a small pulsing amber dot on the nav item. Not a number badge — just a dot indicating activity. This is different from the History badge which shows a count.

**Visibility**: The Queue item is always visible in the sidebar (not hidden when idle). Navigating to it when idle shows the empty state.

**Icon**: A simple horizontal lines-with-dots icon (representing a list/queue).

---

## Left pane in Queue view

When `activeView === 'queue'`, the left pane (`UrlInputPanel`) is hidden. The `desktop-content` area gives full width to `QueueView`. This matches the design decision (Option A from the brainstorm).

In `+page.svelte`, the `desktop-content` layout switches from two-pane to single-pane when `activeView === 'queue'` or `activeView === 'history'`.

---

## Job completion → History handoff

When the last item in `currentJob.items` reaches a terminal state (done, failed, or cancelled):

1. Calculate job-level stats: `totalDuration`, `totalWords`, `successCount`, `failureCount`, `elapsedMs` (job `completedAt - createdAt`).
2. Call `invoke('save_job', { job: currentJob })` — saves the full job record to History (F10).
3. Set `queueActive = false` (removes the sidebar dot badge).
4. Topbar updates to "✓ All done · N of N".

`currentJob` remains in memory until the user navigates away or starts a new job. Starting a new job replaces `currentJob`.

---

## Rust — `save_job` command

```rust
#[tauri::command]
async fn save_job(app: AppHandle, job: JobRecord) -> Result<(), String>
```

Saves to `jobs.json` at `~/Library/Application Support/com.shuhari.transcribe/jobs.json`. Same pattern as `history.json` — a `{ version, records }` wrapper, newest first, max 200 job records (jobs are larger than transcript records). Each `JobRecord` matches the `Job` type above, with items including their results (or null for failed items).

Register in `invoke_handler`.

---

## Before / after user flow

**Single video:**
1. User pastes single URL → preview card appears (F07) → clicks Transcribe
2. App creates a 1-item job, switches to Queue, starts processing
3. Queue shows 1 row: `1 | [thumb] | Title | ▓░ Transcribing…`
4. Completes → `✓ Done · EN · 1.2k words` → user clicks row → transcript appears

**Batch (10 videos):**
1. User picks 10 videos from picker (F08) → clicks "Transcribe 10 videos"
2. Queue opens with 10 rows — first row active, rest waiting
3. Rows complete one by one, done rows become clickable
4. Failed row shows error + Retry button
5. All done → topbar: "✓ All done · 9 of 10" (1 failed) → "View in History"

---

## Acceptance criteria

1. Start a single-video job — Queue tab activates with a dot badge; 1-row list appears with correct title and thumbnail.
2. Active row shows real-time status: "↓ Downloading…" then "✦ N segments" ticking up during transcription.
3. Completed row shows `✓ Done · EN · 1.2k words`. Clicking it opens the transcript in the right pane.
4. Copy and Save buttons in the right pane work correctly on the queued transcript.
5. Failed row shows the plain-English error message. Retry button re-queues the video and processes it.
6. Cancel job button stops the active sidecar and marks remaining waiting items as cancelled.
7. Cancelling a single waiting row removes it from the list without affecting the active item.
8. "View in History" after job completion navigates to History and the completed job is at the top.
9. Starting a new job while viewing the completed queue replaces the queue state.
10. The sidebar dot badge disappears when no job is running.
11. `save_transcript` is called for every successfully completed video — each transcript appears in the existing individual History view (F05 compatibility).
12. `save_job` is called once when the entire job completes — the job record appears in the Job History view (F10).

---

## Note for the coding agent

The existing `run_sidecar` and `cancel_transcribe` Rust commands are unchanged. The queue runner in `+page.svelte` calls them sequentially in a loop — there is no new concurrency in Rust.

The `transcribe-progress` Tauri event is the same event the single-video flow already uses. The queue runner subscribes to it per-item, mapping phase/result/error events to the current `JobItem`.

Svelte's reactivity handles list re-rendering automatically when `currentJob.items` is mutated. To trigger a re-render of a specific item, reassign the array: `currentJob.items = [...currentJob.items]`.

For the right-pane slide-in, use a Svelte `{#if selectedItem}` block wrapping `TranscriptPanel` with a `transition:fly={{ x: 300, duration: 200 }}` animation. The list area uses CSS `flex: 1` and shrinks to accommodate the right pane when it appears.
