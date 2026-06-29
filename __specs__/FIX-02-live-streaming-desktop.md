# FIX-02 — Live Streaming Segments in Desktop TranscriptPanel

## Priority
P1 — UX gap. The desktop right pane is blank for the entire duration of transcription.

---

## Current state
The desktop shell has two panes side-by-side: `UrlInputPanel` (left) and `TranscriptPanel` (right).

`+page.svelte` accumulates live transcript segments in `streamSegments` — an array that grows character-by-character via the typewriter effect — but this array is **never passed to `TranscriptPanel`**. The right pane only receives the final `result` prop. While transcription is running, the right pane shows its empty state ("No transcript yet. Paste a video URL on the left…") for the entire duration.

The typewriter/streaming view **exists** in the web layout branch (`{:else}` block of `+page.svelte`, lines 608–666) but is not reused in the desktop layout branch.

`TranscriptPanel.svelte` currently accepts only:
- `result` — the final transcript object or null
- `activeTab` — format tab selection
- `defaultName` — filename slug for save
- `onTabChange` / `onCopy` — callbacks

---

## After state

| Location | Before | After |
|---|---|---|
| `TranscriptPanel.svelte` props | `result`, `activeTab`, `defaultName`, callbacks | + `streamSegments`, `phase`, `timestamps` |
| Right pane during transcription | "No transcript yet" empty state | Live segments appear with typewriter effect |
| Right pane after transcription | Final result tabs (Plain / Timestamped / SRT) | Same as before — no change |
| Right pane when idle | "No transcript yet" empty state | Same as before — no change |

---

## Target components

1. **`src/lib/desktop/TranscriptPanel.svelte`** — receives new props, renders streaming view in the empty-state branch
2. **`src/routes/+page.svelte`** — passes `streamSegments`, `phase`, `timestamps` to `<TranscriptPanel>`

---

## Before / after flow

**Before:**
1. User pastes URL and clicks Transcribe
2. Left pane shows "Fetching audio…" status
3. Right pane stays blank ("No transcript yet") until sidecar finishes
4. Result appears in right pane when done

**After:**
1. User pastes URL and clicks Transcribe
2. Left pane shows "Fetching audio…" status
3. Right pane continues to show empty state during the `downloading` phase
4. As soon as the first transcript segment arrives (`phase === 'transcribing'`), the right pane switches to the streaming view — segments appear one-by-one with typewriter effect, matching the web layout behaviour
5. When transcription finishes, the right pane switches to result tabs (Plain / Timestamped / SRT) as normal

---

## What to do

### In `TranscriptPanel.svelte`

Add three new props at the top of the `<script>` block:
```
export let streamSegments = [];   // Array<{index, text, start, end, ts, displayed}>
export let phase = 'idle';        // 'idle' | 'downloading' | 'transcribing' | 'done'
export let timestamps = true;
```

In the template, replace the current `{:else}` empty-state block with this logic:
- If `streamSegments.length > 0` OR `phase === 'transcribing'` → show the streaming view (same `.stream-transcript` / `.stream-segment` / `.cursor-blink` markup from `+page.svelte`)
- Otherwise → show the existing "No transcript yet" empty state

Copy the styles for `.stream-transcript`, `.stream-segment`, `.seg-ts`, `.cursor-blink` from `+page.svelte` into `TranscriptPanel.svelte`.

### In `+page.svelte`

Find the `<TranscriptPanel>` usage inside the desktop shell (currently around line 447–453). Add the three new props:
```svelte
<TranscriptPanel
  {result}
  bind:activeTab
  defaultName={url}
  {streamSegments}
  {phase}
  {timestamps}
  onTabChange={(t) => (activeTab = t)}
  onCopy={() => copyToClipboard(getActiveContent())}
/>
```

---

## Acceptance criteria
1. Start a transcription in the desktop app (`npm run tauri:dev`).
2. During the `downloading` phase: right pane stays in the "No transcript yet" empty state.
3. When the first segment arrives: the right pane switches to the streaming view; text appears character-by-character.
4. When transcription finishes: the streaming view is replaced by the result tabs (Plain / Timestamped / SRT).
5. Switching to a new transcription clears the right pane back to empty state before the next run begins.
6. The web layout (non-Tauri) is unaffected.
