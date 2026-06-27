## 1. Sidecar — search-results detection

- [x] 1.1 Add `_YOUTUBE_SEARCH_RESULTS_HOSTS` set and `is_youtube_search_results_url(url)` helper in `api/transcribe_core.py`, mirroring the existing `_is_youtube_url` / `_is_instagram_url` pattern
- [x] 1.2 Add `search_query_from_url(url)` helper in `api/transcribe_core.py` that uses `urllib.parse` to extract the `search_query` parameter (returns `None` when missing or empty)
- [x] 1.3 Extend `probe_url()` in `api/sidecar.py` with a new branch: when `is_youtube_search_results_url(url)` and `search_query_from_url(url)` returns a non-empty string, build `ytsearch[50]:<query>` and pass it to `extract_info` with `extract_flat: "in_playlist"`
- [x] 1.4 Return `{"type": "search", "query": <query>, "url": <original>, "count": <n>, "entries": [<normalised entries>]}` from the new branch; reuse the existing playlist-entry normalisation block
- [x] 1.5 Reject empty queries with `{"type": "error", "code": "INVALID_URL", "message": "search_query is empty"}` — verify by pasting `https://www.youtube.com/results?search_query=`
- [x] 1.6 Verify error classification by simulating a network failure — confirm `probe_url` returns `type: "error"` with `code: "NETWORK"` rather than crashing

## 2. Rust bridge — pass through new fields

- [x] 2.1 Update `probe_url` in `src-tauri/src/lib.rs` if needed to surface the `kind` and `query` fields to the frontend (likely no change — the existing command returns the sidecar JSON verbatim, so the new fields propagate automatically)
- [x] 2.2 Run `cargo check` in `src-tauri/` to confirm no Rust changes are required

## 3. Frontend — route search results to the picker

- [x] 3.1 In `src/routes/+page.svelte`, route the dispatched playlist event so search results route through the same flow. (Implemented in `UrlInputPanel.svelte` since the picker renders inside the panel after the right-pane refactor.)
- [x] 3.2 Update the probe-result handler so `type === 'search'` opens the picker inline (mirroring the playlist branch)
- [x] 3.3 Pass a `kind` prop to `<VideoPicker>` derived from the probe result's `type` (`'playlist'` or `'search'`)
- [x] 3.4 When the probe returns `type: 'search'` with `entries: []`, surface an inline error in the Transcribe panel instead of opening an empty picker

## 4. Frontend — picker label tweak

- [x] 4.1 Add an `export let kind = 'playlist'` prop to `src/lib/desktop/VideoPicker.svelte`
- [x] 4.2 Update the picker header to render `"Search: <query>"` when `kind === 'search'`, falling back to the existing `"Playlist: <title>"` when `kind === 'playlist'`
- [x] 4.3 Run `npm run check` and `npx vite build` to verify no Svelte type errors

## 5. Verification

- [ ] 5.1 Manually paste `https://www.youtube.com/results?search_query=ipad+air+m4` and confirm the picker opens with up to 50 entries (requires running Tauri app + network — not verifiable from CI terminal)
- [ ] 5.2 Select 3 videos, click Transcribe, confirm the Queue view runs all three sequentially (requires Tauri app)
- [ ] 5.3 Confirm existing single-video flow still works: paste `https://www.youtube.com/watch?v=...` and verify the preview card appears as before (requires Tauri app)
- [ ] 5.4 Confirm existing playlist flow still works: paste a playlist URL and verify the picker label reads "Playlist: …" not "Search: …" (requires Tauri app)
- [x] 5.5 Paste an empty search URL (`?search_query=`) and confirm an error is shown instead of an empty picker — verified via Python unit test (`probe_url` returns `type: "error"`, `code: "INVALID_URL"`, message `search_query is empty`); frontend surfacing this is wired through `probeError` state which the existing error UI already renders.
- [ ] 5.6 Confirm the existing duplicate-detection logic still flags URLs that appear in search results AND in history (requires Tauri app; `startBatchJob` is reused unchanged, so behaviour is identical to the existing playlist-batching flow)