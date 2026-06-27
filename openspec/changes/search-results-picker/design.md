## Context

The current `probe_url()` in `api/sidecar.py` (introduced in F07) recognises
two URL shapes:

- Single video (`_type == "url"` or no `entries`) → returns `type: "video"`
- Playlist (`_type == "playlist"` or `entries` present) → returns
  `type: "playlist"` with a normalised `entries[]`

yt-dlp handles YouTube search-results pages as a third shape: when
`extract_info` is called with the URL alone, yt-dlp resolves the page
metadata but returns no entries — the page itself is just HTML. The
proper way to get the entries is to rewrite the URL as
`ytsearch[N]:<query>` and pass that to yt-dlp, which then expands it
into a multi-entry result.

This change adds a third detection branch in `probe_url` for
`youtube.com/results?search_query=…` URLs. The branch:

1. Parses `search_query` out of the URL's query string.
2. Builds a `ytsearch[50]:<query>` synthetic URL.
3. Calls `extract_info` with `extract_flat: "in_playlist"` (the same
   flat-extraction mode the playlist branch uses) to get a list of
   entries without downloading anything.
4. Returns `type: "search"` with the entries plus a `query` field.

The frontend picks up `type: "search"` and routes to the existing
VideoPicker with a `kind: "search"` flag so the picker labels the
source as "Search: …" rather than "Playlist: …".

## Goals / Non-Goals

**Goals:**

- Detect `youtube.com/results?search_query=…` URLs as a third URL kind.
- Expand them into a list of up to 50 video entries via `ytsearch[50]:`.
- Reuse the existing VideoPicker UI for selection (no new components).
- Distinguish "Search: …" from "Playlist: …" in the picker header.
- Keep single-video and playlist flows behaving exactly as today.

**Non-Goals:**

- Passing through YouTube's `sp=` filter parameters (uploads, type,
  duration filters). yt-dlp's `ytsearch:` extractor accepts a query
  string only, not a full filter spec. Adding filter support would
  require rewriting the filter into yt-dlp's format and is out of
  scope for this change.
- Pagination / "load more". The user gets 50 results; if they want
  more, they refine the query. Adding a "Next page" button would
  require loading 50+ entries and tracking offset, which is
  disproportionate to the value for a v1 feature.
- Non-YouTube search engines (Google, Bing, DuckDuckGo). Out of
  scope; we only recognise YouTube's `/results?` shape.
- Changing the duplicate-detection rules. Search-results entries
  route through the existing duplicate logic unchanged.

## Decisions

### Decision: Reuse VideoPicker with a `kind` field rather than build a new component

**Rationale:** The picker already supports the exact entry shape search
results produce (`id`, `title`, `thumbnail`, `duration`, `url`). The only
distinction a search result needs is a different header label. Adding a
`kind` prop to `VideoPicker` (default `playlist`, set to `search` when
the probe returns `type: "search"`) is the smallest possible diff and
keeps the UI components canonical.

**Alternatives considered:**

- *Two separate components (`PlaylistPicker` + `SearchPicker`)*:
  doubles the maintenance surface for what is essentially the same
  component with different header text.
- *Infer kind from entries*: brittle and would mis-classify a
  playlist whose first entry happened to be returned in search
  order.

### Decision: Use `ytsearch[50]:` with a hard cap of 100, no env override

**Rationale:** YouTube's own UI shows ~20 results per page; 50 is a
comfortable round number that covers nearly every useful query without
requiring pagination. yt-dlp's `ytsearch[N]:` accepts any N; capping
at 100 protects against pathological inputs that could stall the probe
for many seconds. There is no env var for N in v1 — a constant is
simpler and the user can't usefully request more than 100 anyway.

**Alternatives considered:**

- *Env var (`YT_SEARCH_MAX`) for N*: adds a knob without a real use
  case. If we later need different limits for different surfaces,
  add the env var then.
- *Allow user to set N from the picker*: complicated UX, low value.

### Decision: Parse `search_query` from URL with stdlib, no new dependency

**Rationale:** `urllib.parse` is already imported in
`transcribe_core.py` for host classification. Adding the same import
to `sidecar.py` is free. Using a regex or `yt-dlp`'s URL parser would
be overkill for a single query parameter.

### Decision: Pass `kind` through the probe result, not inferred from URL on the frontend

**Rationale:** Keeping the source-of-truth in the sidecar means the
frontend doesn't need to re-parse the URL just to label the picker.
The probe result already returns `type`, `title`, `uploader`, `count`
— adding `kind` (and a `query` for search results) is the natural
extension.

## Risks / Trade-offs

- **Risk:** YouTube rate-limits `ytsearch:` more aggressively than
  single-video resolution, especially from datacenter IPs.
  **Mitigation:** the desktop app uses the user's residential IP, so
  the rate-limit risk is low. We surface `BOT_CHALLENGE` errors via
  the existing error-classification path; the user sees the standard
  "YouTube is blocking this video" message and can retry.

- **Risk:** yt-dlp's `ytsearch[N]:` URL doesn't accept the `sp=` filter
  parameters YouTube's web UI does. A user who pastes a filtered
  search URL silently loses their filter.
  **Mitigation:** documented as a non-goal in the proposal. If users
  complain, v2 can translate the filter set. The probe still works —
  it just returns the unfiltered top-N.

- **Risk:** Picking 50 entries by default produces a long list to
  scroll through for niche queries where only 3–4 results are useful.
  **Mitigation:** the VideoPicker already supports per-row selection,
  so the user just leaves the rest unchecked. No "select first 5"
  default that would surprise users.

- **Trade-off:** The sidecar now has three URL-detection branches
  (video / playlist / search). Each branch returns a slightly
  different shape (`type`, optional `entries`, `kind`, `query`).
  **Mitigation:** keep the frontend's branching on `type` minimal —
  the picker treats `kind === "search"` and `kind === "playlist"`
  identically except for the header label.

## Migration Plan

No data migration. No new dependencies. Deploy = rebuild the sidecar
binary (`scripts/build_sidecar.py`) and reload the Tauri app. Rollback
= revert to the previous sidecar build; the frontend change is
backwards-compatible because the new `kind` field defaults to
`"playlist"` when missing.

## Open Questions

- **Should `N` be user-configurable?** Currently hardcoded to 50.
  Decision: no for v1; revisit if users complain.
- **Should we honour `sp=` filters?** Decision: no for v1; revisit
  if the unfiltered top-N is too noisy for popular queries.
- **Should the picker remember the last search query?** Decision: no
  for v1; history already keeps the last URLs the user transcribed.