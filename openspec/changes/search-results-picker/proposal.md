## Why

The app already handles single video URLs and playlist URLs (F07 + F08) but
not YouTube **search results pages** like
`https://www.youtube.com/results?search_query=ipad+air+m4`. When the user
pastes such a URL today, the sidecar probe falls back to single-video mode,
transcribes one arbitrary entry (or fails outright), and gives the user no
way to choose which video from the result page they want. There is no
way to discover or batch-select videos without manually opening each one
and copying its watch URL.

This change extends the existing URL-detection layer so a `/results?`
URL is recognised, expanded into a list of candidate videos via
yt-dlp's `ytsearch[N]:` extractor, and routed through the same
VideoPicker UI that playlists already use. The user gets one input
(search URL) and many selectable outputs, which is the natural
ergonomic for "I want to transcribe the top few videos about X."

## What Changes

- Extend `probe_url` in `api/sidecar.py` to detect YouTube `/results?`
  URLs and resolve them as `type: "search"` with an `entries[]` array,
  using `ytsearch[N]:<query>` under the hood. Default `N = 50`, capped
  at 100 to keep the probe under a couple of seconds.
- Reuse the existing `VideoPicker` UI for search results — no new
  frontend component is needed. The picker's left pane shows the
  query text (e.g. "Search: ipad air m4") instead of "Playlist: …".
- Add a new `search-results` capability spec formalising the
  detection, extraction, and selection rules.
- Extend the `local-transcription-flow` capability with one new
  requirement covering search-results detection (the playlist
  detection requirement is unchanged).
- Non-breaking: single-video and playlist flows continue to behave
  exactly as today. The only new behaviour is the search-page branch.

## Capabilities

### New Capabilities

- `search-results`: Detect YouTube search-results URLs, expand them
  into a list of candidate videos via yt-dlp's `ytsearch[N]:` extractor,
  and present the same VideoPicker UI playlists already use so the user
  can multi-select and queue them. Covers the URL-detection, the
  extraction, and the selection UI rules specific to search pages.

### Modified Capabilities

<!-- None. The existing `local-transcription-flow` spec lives in the
     in-flight `tauri-desktop-app` change and is not yet promoted to
     `openspec/specs/`, so it is not a "modified capability" from the
     perspective of this change. Search-results routing is fully
     captured in the new `search-results` capability above. -->

## Impact

- **Code**:
  - `api/sidecar.py` — extend `probe_url()` with a YouTube
    `/results?search_query=…` branch that calls yt-dlp with
    `ytsearch[count]:<query>` and returns `type: "search"` plus
    `entries[]`. Reuses the existing playlist-entry normalisation.
  - `src/lib/desktop/VideoPicker.svelte` — small label tweak
    ("Search: …" vs "Playlist: …") gated on the new optional
    `kind` field the probe result provides. No structural changes.
  - `src/routes/+page.svelte` — when the probe result is
    `type === 'search'`, set `activeView = 'picker'` exactly like
    the playlist branch already does. The existing `playlistProbeResult`
    variable is renamed to `probeResult` since it now covers both
    kinds; the picker receives a `kind` prop so it can label
    appropriately.
  - `src/lib/desktop/UrlInputPanel.svelte` — no structural changes;
    the picker-mode branch already exists and renders the picker.
- **Dependencies**: none new. `yt-dlp` already supports the
  `ytsearch:` extractor. No new Python or JS packages.
- **Deployment**: no impact. Sidecar gains one branch; the rest of
  the build pipeline is untouched.
- **UX**: A search URL becomes equivalent to a playlist URL from the
  user's perspective — they pick videos, click Transcribe, the queue
  runs them sequentially. No new affordances to learn.
- **Limits**: `N = 50` by default, capped at 100, matches YouTube's
  own ~20-per-page granularity. Larger counts are rejected with a
  clear error so a user pasting a malformed query doesn't silently
  cap to 5 results.