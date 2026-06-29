# F05 — History View

## Priority
P1 — F04 (storage) is already complete. This spec can be worked immediately.

---

## Current state
The desktop shell in `+page.svelte` has three `activeView` states: `transcribe`, `history`, `settings`. The sidebar navigation (`SidebarNav.svelte`) switches `activeView` correctly.

**However:** the `desktop-content` area does not branch on `activeView`. It always renders the same layout: `UrlInputPanel` on the left, `TranscriptPanel` on the right — regardless of which view is active. When the user clicks History in the sidebar, the topbar title says "History" but the content area is unchanged. There is no `HistoryView` component.

The Rust storage layer is fully built: `load_history`, `delete_transcript`, and `clear_history` commands exist and work. The data is there — the UI just doesn't show it.

---

## After state
The History view shows a scrollable list of past transcriptions. Each row shows the source URL, language, word count, and date. Clicking a row opens the full transcript in the right-hand `TranscriptPanel`. The user can delete records from the list.

| Area | Before | After |
|---|---|---|
| History sidebar item | Stub placeholder text | Live list of past transcripts |
| Right pane when history item selected | Unchanged (shows blank TranscriptPanel) | Shows the selected transcript's content |
| Delete action | Does not exist | Trash icon on each row removes the record |
| Empty state | Stub text | Friendly empty state ("No transcriptions yet. Start one from the Transcribe tab.") |

---

## Target components

1. **`src/lib/desktop/HistoryView.svelte`** — new component. Renders the list and handles selection + delete.
2. **`src/routes/+page.svelte`** — load history on mount; wire `HistoryView` into the desktop shell's `history` branch; pass the selected record to `TranscriptPanel`.

---

## Layout

```
Desktop shell when activeView === 'history':
┌─────────────────────────────────────────────────────┐
│ SidebarNav        │  desktop-topbar: "History"       │
│  [Transcribe]     ├──────────────────────────────    │
│  [History] ←      │  desktop-content                 │
│  [Settings]       │  ┌──────────────┬──────────────┐ │
│                   │  │ HistoryView  │ TranscriptP  │ │
│                   │  │ (380px fixed)│ anel (flex)  │ │
│                   │  └──────────────┴──────────────┘ │
└─────────────────────────────────────────────────────┘
```

The left pane width stays at 380px (same as `UrlInputPanel`). The right pane shows `TranscriptPanel` with the selected record loaded, or its empty state if nothing is selected.

---

## HistoryView component

### Header
- "History" title (14px, 600 weight)
- A count chip: "42 transcripts"
- A search input (placeholder: "Search transcripts…") — filters the list client-side by URL or transcript text as the user types

### List item (each past record)
- **Line 1:** URL displayed as hostname + path, truncated to 1 line, with a small link-out icon
- **Line 2:** Language badge (e.g. "EN") · word count · relative date ("2 hours ago", "Yesterday", "Jun 20")
- **Right side:** Trash icon (appears on hover) — calls `delete_transcript`, removes from list

### Selection state
- Selected item has a highlighted background (accent-tinted)
- Clicking an item loads its transcript into `TranscriptPanel` — sets `result` to the record's `{ language, plain, timestamped, srt }` and sets `activeTab` to `'plain'`

### Empty state (no records)
Show the document icon + "No transcriptions yet" + "Head to the Transcribe tab to get started."

### Empty state (search returns nothing)
"No results for '…'" with a small clear-search link.

---

## Before / after user flow

**Before:**
1. User clicks History in sidebar
2. Sees static text stub — nothing useful

**After:**
1. User clicks History in sidebar
2. List of past transcriptions loads (from `load_history` Rust command)
3. User clicks any row → the right pane fills with that transcript (Plain tab selected by default)
4. User can switch to Timestamped / SRT tabs in the right pane
5. User can copy or save the transcript using the existing right-pane action buttons
6. User hovers a row and clicks the trash icon → record is deleted; row disappears from the list; right pane returns to empty state if the deleted record was selected

---

## Data loading
On `+page.svelte` mount (after Tauri APIs are ready): call `invoke('load_history')` and store the result in a reactive `historyRecords` array. Pass `historyRecords` into `<HistoryView>` as a prop. When a record is deleted inside `HistoryView`, emit an event upward so `+page.svelte` can remove it from `historyRecords` (keeping the in-memory list and the disk store in sync without a full reload).

---

## Acceptance criteria
1. Navigate to History — list loads within 300ms of clicking the tab.
2. Each past transcript appears as a row with URL, language, word count, and date.
3. Clicking a row shows that transcript's plain text in the right pane.
4. The Plain / Timestamped / SRT tabs work on the loaded history record.
5. Copy and Save buttons in the right pane work on the loaded history record.
6. Deleting a record removes it from the list immediately and persists after app restart.
7. Typing in the search box filters the list in real time.
8. With no records, the friendly empty state is shown.
9. Navigating to Transcribe and completing a new transcription adds that record to the top of the History list.
