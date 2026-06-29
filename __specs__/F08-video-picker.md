# F08 — Video Picker (Playlist / Page URL)

## Priority
P1 — Depends on F07 (probe result with entries array). Single-video flow is unaffected until this is built.

---

## Current state

No picker exists. Playlist URLs either fail or transcribe only the first entry. The right pane always shows `TranscriptPanel` regardless of what the user pasted.

After F07 ships, a playlist probe result will be available as a JS object with `entries[]`. This spec defines what the user sees and does with those entries.

---

## After state

When a playlist probe completes, the right pane of the Transcribe view becomes a video picker: a scrollable list of all videos in the playlist, each with thumbnail, title, and duration. The user selects the videos they want, then hits "Transcribe X videos" to start a batch job.

| Area | Before | After |
|---|---|---|
| Paste playlist URL | Undefined behaviour | Right pane shows video picker list |
| Left pane | Shows URL input + Transcribe button | Shows URL input + "Found N videos" count + "Transcribe X" button |
| Video selection | Not possible | Checkbox per row; Select All / Deselect All |
| Start batch | Not possible | "Transcribe X videos" button → switches to Queue (F09) |

---

## Target components

1. **`src/lib/desktop/VideoPicker.svelte`** — new component. Receives `entries[]` from the probe result. Renders the selectable list. Emits `startJob` event with the selected entries.
2. **`src/routes/+page.svelte`** — when `activeView === 'picker'`, render `VideoPicker` in the right pane instead of `TranscriptPanel`. Left pane stays as `UrlInputPanel` (showing the "Found N videos" state). Handle the `startJob` event to begin the batch queue (F09).
3. **`src/lib/desktop/UrlInputPanel.svelte`** — add a `pickerMode` state: when active, replace the Transcribe button with a "Transcribe X videos" button that is disabled until at least 1 video is selected. The count "X" is passed in as a prop from `+page.svelte`.

---

## Layout

```
Desktop shell — Transcribe tab, playlist URL pasted:

┌─────────────────────────────────────────────────────────────┐
│ Sidebar  │  topbar: "Transcribe"                            │
│          ├──────────────────────────────────────────────────│
│          │ Left pane (380px)  │  Right pane (flex)          │
│          │                    │                             │
│          │ [URL input]        │  ┌─ VideoPicker ──────────┐ │
│          │                    │  │ □ Select all (24)       │ │
│          │ Found 24 videos    │  │ ─────────────────────── │ │
│          │ My Playlist ·      │  │ ☑ [thumb] Title 1  3:20 │ │
│          │ Channel Name       │  │ ☑ [thumb] Title 2  1:45 │ │
│          │                    │  │ □ [thumb] Title 3  8:12 │ │
│          │ ─────────────────  │  │ ☑ [thumb] Title 4  2:30 │ │
│          │ [Transcribe 3 vid] │  │ ...                     │ │
│          │                    │  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## VideoPicker component

### Props
```typescript
export let entries: Array<{
  id: string;
  title: string;
  thumbnail: string;
  duration: number;      // seconds
  url: string;
}> = [];
export let playlistTitle: string = '';
export let uploader: string = '';
```

### Emits
```typescript
// Fired when user clicks "Transcribe" in the left pane action bar
// Payload: the selected subset of entries, in original order
dispatch('startJob', { selected: Entry[] });
```

### Header row
- Checkbox: "Select all" / "Deselect all" (checked when all selected, indeterminate when some selected)
- Count label: "24 videos" or "3 of 24 selected"

### Video row
Each row in the list:
- Checkbox (left)
- Thumbnail: 56×32px (16:9). Loaded as `<img>` with `loading="lazy"`. Grey placeholder on error.
- Title: 13px, 500 weight, single line, ellipsis overflow
- Duration: formatted as `M:SS` or `H:MM:SS`, 11.5px, `var(--text-3)`, right-aligned

Row height: 48px. Alternating background subtle stripe not needed — hover highlight only.

### Selection behaviour
- Clicking anywhere on a row (not just the checkbox) toggles selection.
- Selected rows have a faint accent-tinted background.
- The selected count is kept in a reactive `selectedIds: Set<string>`.
- The selected count is passed upward to `+page.svelte` via a reactive prop so `UrlInputPanel` can update its button label.

### Keyboard
- `Space` on a focused row toggles its checkbox.
- `Cmd+A` selects all.

### Empty state
If probe returned 0 entries (edge case): "No videos found in this playlist."

### Large playlists
For playlists with 100+ entries, virtual scrolling is NOT required in v1 — standard DOM rendering with `overflow-y: auto` is sufficient. Revisit if performance issues arise.

---

## UrlInputPanel — picker mode

When `pickerMode` is true (passed as prop from `+page.svelte`):

Replace the normal "Transcribe" button with:
```
[  Transcribe 3 videos  →  ⌘↩  ]
```

Disabled when `selectedCount === 0`. Active when `selectedCount >= 1`.

Also show below the URL input:
```
Found 24 videos
My Playlist · Channel Name
```
In place of the normal "YouTube · Instagram…" hint text.

The "Options" section (timestamps toggle) remains visible — timestamps preference applies to all videos in the batch.

---

## Starting a batch job

When the user clicks "Transcribe X videos":

1. `VideoPicker` emits `startJob` with the selected entries array.
2. `+page.svelte` receives the event and:
   a. Stores the job definition: `{ entries: SelectedEntry[], model, timestamps, startedAt: Date }`.
   b. Switches `activeView` to `'queue'`.
   c. Begins processing the queue (F09).
3. `activeView` changes to `'queue'` — the sidebar Queue item activates, the picker disappears.

The URL input is NOT cleared. If the user navigates back to Transcribe, the picker re-renders with the same entries and selection state.

---

## Before / after user flow

**Before:**
1. Paste playlist URL → undefined behaviour

**After:**
1. Paste `https://youtube.com/playlist?list=PLxxx`
2. Left pane shows "Checking URL…" spinner (800ms debounce + probe)
3. Probe completes → left pane shows "Found 24 videos · My Playlist"
4. Right pane shows video picker list — all 24 videos, all pre-selected (default: all selected)
5. User unchecks 3 videos they don't want
6. Left pane button shows "Transcribe 21 videos"
7. User clicks → view switches to Queue (F09), batch job starts

---

## Acceptance criteria

1. Paste a YouTube playlist URL — picker renders within 5 seconds showing all videos with correct titles and thumbnails.
2. All videos are selected by default on load.
3. "Select all" checkbox is checked by default. Unchecking it deselects all rows. Checking it re-selects all.
4. Clicking a row toggles its selection. The button label updates immediately ("Transcribe X videos").
5. "Transcribe X videos" button is disabled when 0 videos are selected.
6. Thumbnails load lazily. A broken thumbnail shows a grey placeholder — does not break the row layout.
7. Durations are formatted correctly (e.g. `3:20`, `1:05:42`).
8. Clicking "Transcribe 21 videos" switches the view to Queue and the job starts.
9. Navigating away from Transcribe tab and back preserves the picker state (same URL, same selection).
10. Pasting a new URL while in picker mode triggers a new probe and resets the picker.

---

## Note for the coding agent

Thumbnail images are remote HTTPS URLs from YouTube's CDN (`i.ytimg.com`). The CSP fix in F07 (`img-src 'self' https: data:`) must be applied before thumbnails will render here. Do not re-apply the CSP change if F07 is already merged.

The `entries` array comes from the probe result in `+page.svelte`. `VideoPicker` is a pure display component — it does not call any Tauri commands. All Tauri invocations happen in `+page.svelte`.

Do not attempt to pre-download thumbnails or cache them locally — the Tauri webview handles remote image loading natively.
