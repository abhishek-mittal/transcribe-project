# F10 — Job History View

## Priority
P1 — Depends on F09 (save_job command and job record schema). Replaces the current per-transcript HistoryView with a job-level log.

---

## Current state

`HistoryView.svelte` exists and shows a list of individual transcript records (one per video). It loads from `history.json` via `load_history`. This is useful for finding a specific past transcript, but it has no job-level grouping, no analytics, and no error diagnostics.

`jobs.json` does not exist yet — it is created by F09's `save_job` command.

After F09 ships, every completed job (single video or batch) will write a `JobRecord` to `jobs.json`. This spec defines the History view that surfaces those records.

---

## After state

The History tab becomes a job log. Each entry is a job (one run), not an individual transcript. The user can see at a glance how many jobs ran, when, how many videos succeeded or failed, and how long each took. Expanding a job shows per-video breakdown with individual error logs and a link to open each transcript.

The existing per-transcript search (currently in `HistoryView.svelte`) moves to a dedicated "Transcripts" sub-view accessible from within an expanded job row, or is retained as a secondary search accessible from the History header.

| Area | Before | After |
|---|---|---|
| History tab | Flat list of individual transcripts | List of jobs, each expandable |
| Job entry | Does not exist | Date, video count, success/fail, time taken, model |
| Per-video breakdown | Not grouped | Expandable inside each job row |
| Error diagnostics | Not shown | Classified error + human message per failed video |
| Analytics | None | Time taken, words transcribed, avg speed per job |
| Individual transcript access | Direct from list | Click video row inside expanded job |

---

## Target components

1. **`src/lib/desktop/JobHistoryView.svelte`** — new component. Replaces `HistoryView.svelte` in the History view branch of `+page.svelte`. Receives `jobs[]` prop. Renders job list with expand/collapse.
2. **`src/routes/+page.svelte`** — call `invoke('load_jobs')` on mount (after Tauri APIs load). Pass `jobs` to `JobHistoryView`. Handle `openTranscript` event from `JobHistoryView` to load a transcript into `TranscriptPanel`.
3. **`src-tauri/src/lib.rs`** — new commands: `load_jobs`, `delete_job`, `retry_job_item` (see below). `save_job` is defined in F09.

---

## Job record schema (Rust)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobItemRecord {
    id: String,
    url: String,
    title: String,
    thumbnail: String,
    duration_secs: u32,
    status: String,              // "done" | "failed" | "cancelled"
    error_code: Option<String>,
    error_message: Option<String>,
    language: Option<String>,
    plain: Option<String>,
    timestamped: Option<String>,
    srt: Option<String>,
    word_count: Option<u32>,
    started_at: Option<String>,
    completed_at: Option<String>,
    elapsed_ms: Option<u64>,
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
    total_audio_secs: u32,      // sum of duration_secs for done items
    items: Vec<JobItemRecord>,
}

struct JobStore {
    version: u32,
    records: Vec<JobRecord>,    // newest first, max 200
}
```

`jobs.json` lives at `~/Library/Application Support/com.shuhari.transcribe/jobs.json`.

---

## Rust commands

### `load_jobs`
Reads `jobs.json`. Returns `Vec<JobRecord>`. If file missing or corrupt, returns empty vec. Never errors.

### `delete_job`
Accepts `job_id: String`. Removes the matching record from `jobs.json` and persists. Returns `Ok(())`.

### `retry_job_item`
This command does NOT re-run transcription — that is handled by the queue runner in JS. Its only job is to reset a failed item's status fields in `jobs.json` to `waiting` so the UI can show it correctly before re-queuing. Takes `job_id: String`, `item_id: String`. Returns the updated `JobItemRecord`.

Register all three (plus `save_job` from F09) in `invoke_handler`.

---

## JobHistoryView component

### Props
```typescript
export let jobs: JobRecord[] = [];
```

### Emits
```typescript
dispatch('openTranscript', { item: JobItemRecord });
dispatch('retryFailed', { jobId: string, itemId: string });
dispatch('deleteJob', { jobId: string });
```

---

## Layout — job list

```
┌─ History ──────────────────────────────────────────────────────┐
│  [Search jobs…]                          [24 jobs]             │
│────────────────────────────────────────────────────────────────│
│  ▶ Jun 27, 14:30 · 10 videos · ✓ 9 done · ✗ 1 failed · 4m32s  │
│  ▶ Jun 27, 09:15 · 1 video  · ✓ Done                · 1m18s   │
│  ▶ Jun 26, 22:04 · 5 videos · ✓ All done             · 8m51s  │
│  ▶ Jun 25, 17:30 · 3 videos · ✓ All done             · 5m02s  │
└────────────────────────────────────────────────────────────────┘
```

### Job row (collapsed)
One line, full width. Clicking expands:
- **Date + time**: `Jun 27, 14:30` — formatted from `created_at`
- **Video count**: `10 videos` or `1 video`
- **Status summary**: `✓ 9 done · ✗ 1 failed` or `✓ All done` or `✗ All failed`
- **Time taken**: `4m32s` — formatted from `elapsed_ms`
- **Model badge**: small pill `tiny` / `base` / `small` on the right
- **▶ / ▼ chevron**: indicates expanded state

Hover shows a `[trash]` icon on the far right to delete the job.

---

## Layout — job row expanded

```
▼ Jun 27, 14:30 · 10 videos · ✓ 9 done · ✗ 1 failed · 4m32s   [tiny] [🗑]
  ┌─ Analytics ──────────────────────────────────────────────────────────┐
  │  Total words: 14,320  ·  Audio processed: 48m  ·  Avg: 5.3× realtime │
  └──────────────────────────────────────────────────────────────────────┘
  ┌─ Videos ─────────────────────────────────────────────────────────────┐
  │  ✓  [thumb] Video title 1              EN · 1,240w · 1m02s  [Open ↗] │
  │  ✓  [thumb] Video title 2              EN · 890w   · 0m48s  [Open ↗] │
  │  ✗  [thumb] Failed video title         Bot challenge         [↺ Retry] │
  │       └─ YouTube blocked this video. Try opening it in your browser  │
  │          first, then paste the URL again.                            │
  │  ✓  [thumb] Video title 4              FR · 2,100w · 2m15s  [Open ↗] │
  └──────────────────────────────────────────────────────────────────────┘
```

### Analytics panel
Shown at the top of the expanded job. Compact single line or two lines:
- **Total words transcribed**: sum of `word_count` for done items
- **Audio processed**: sum of `duration_secs` formatted as `Xh Ym` or `Ym Zs`
- **Average speed**: `total_audio_secs / (elapsed_ms / 1000)` → `5.3× realtime` (how many seconds of audio processed per second of wall time)
- **Model used**: already shown in the collapsed row badge

### Video rows inside expanded job
Each video in the job:

**Done video:**
- `✓` check (green) + thumbnail (40×24px) + title (truncated) + language badge + word count + elapsed time + `[Open ↗]` button
- Clicking `[Open ↗]` emits `openTranscript` — `+page.svelte` loads the transcript into `TranscriptPanel` in the right pane and switches `activeView` to `'history'`-transcript sub-view (or a detail pane)

**Failed video:**
- `✗` icon (red) + thumbnail + title + error label (short: "Bot challenge", "Network error", "Unsupported")
- `[↺ Retry]` button — emits `retryFailed`. `+page.svelte` adds the item back to a new single-item job and switches to Queue.
- Expanded error detail (one line below, indented): full plain-English explanation from `errors.ts` `errorMessageFor(code)`

**Cancelled video:**
- `—` dash + thumbnail + title + `"Cancelled"` in `var(--text-3)`
- No retry button (user deliberately cancelled)

---

## Right pane behaviour in History

When the user clicks `[Open ↗]` on a done video:
- Right pane slides in (same transition as F09) with `TranscriptPanel` loaded with the item's `{ language, plain, timestamped, srt }`.
- The video title appears at the top of `TranscriptPanel` (via the existing `defaultName` prop — pass the video title here instead of the URL).
- All tabs (Plain / Timestamped / SRT) and action buttons (Copy, Save) work normally.

---

## Search

A search input at the top of `JobHistoryView` filters across job titles, video titles, and transcript text. Search is client-side, runs on the already-loaded `jobs[]` array. Matching is case-insensitive substring. Results collapse non-matching jobs and highlight matching videos within expanded jobs.

---

## Before / after user flow

**Viewing past work:**
1. User clicks History
2. Sees a list of past jobs ordered by date
3. Clicks a job row to expand it
4. Sees analytics: "14,320 words · 48 min audio · 5.3× realtime"
5. Sees all 10 video rows with status
6. Clicks `[Open ↗]` on a done video → transcript appears in the right pane
7. Notices 1 failed video with "Bot challenge" → clicks `[↺ Retry]` → Queue opens and re-runs that video

**Diagnosing a failure:**
1. Expand a failed job
2. Failed row shows: `✗ Bot challenge`
3. Below it: "YouTube blocked this video. Try opening it in your browser first, then paste the URL again."
4. User understands exactly what happened and what to do

---

## Acceptance criteria

1. History tab loads job list within 300ms of clicking the tab.
2. Each job row shows correct date, video count, status summary, time taken, and model.
3. Expanding a job shows the analytics panel with correct word count, audio duration, and speed ratio.
4. Each video within a job shows its correct status icon, language, word count, and elapsed time.
5. Clicking `[Open ↗]` on a done video opens its transcript in the right pane with all tabs working.
6. Failed video rows show a short error label and the full plain-English explanation below.
7. Retry button on a failed video re-queues it and switches to Queue view.
8. Deleting a job removes it from the list and from `jobs.json`.
9. After completing a new job (F09), it appears at the top of the History list immediately.
10. Search filters jobs and videos in real time. Non-matching jobs collapse.
11. With no jobs, the friendly empty state is shown: "No jobs yet. Start one from the Transcribe tab."
12. `jobs.json` never exceeds 200 records — oldest are trimmed on `save_job`.

---

## Note for the coding agent

`HistoryView.svelte` (the current per-transcript list) is NOT deleted. It remains in `src/lib/desktop/` and continues to be called by `save_transcript` after each individual video completes. The individual transcript history (for the "find a specific past transcript" use case) is still accessible — either as a search within `JobHistoryView`, or by preserving the existing `HistoryView` as a tab within the History section. The simplest approach for v1: replace the History main view with `JobHistoryView` and let `[Open ↗]` inside job rows be the way to access individual transcripts. The per-transcript `HistoryView` can be deprecated in v2.

The `items[].plain` field in `jobs.json` can be large (full transcript text). For a 100-video job with 1,000-word transcripts each, `jobs.json` could reach ~1MB. This is acceptable for v1. If size becomes a concern in a later version, transcript text can be stored by reference (using the `id` from `history.json`) rather than inlined in `jobs.json`.
