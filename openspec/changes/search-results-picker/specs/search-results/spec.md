## ADDED Requirements

### Requirement: Search-results URL detection
The system SHALL recognise YouTube search-results URLs as a third URL kind.

#### Scenario: Plain search URL is detected
- **WHEN** the user pastes `https://www.youtube.com/results?search_query=ipad+air+m4`
- **THEN** the sidecar's `probe_url` returns `type: "search"` with `entries[]` populated from yt-dlp's `ytsearch[N]:ipad air m4` resolution

#### Scenario: Search URL with filter parameters is detected
- **WHEN** the user pastes `https://www.youtube.com/results?search_query=ipad+air+m4&sp=EgIIAw%253D%253D`
- **THEN** the sidecar's `probe_url` returns `type: "search"` and the `sp` filter is not passed through to yt-dlp

#### Scenario: Non-YouTube search URL is not detected
- **WHEN** the user pastes a URL that is not a YouTube results page (e.g. a Google search URL or a YouTube channel page)
- **THEN** the sidecar's `probe_url` returns `type: "video"` or `type: "playlist"` exactly as it does today — search-results detection is opt-in only for YouTube's `/results?` shape

### Requirement: Search-results entry expansion
The sidecar SHALL resolve a detected search-results URL into a list of video entries using yt-dlp's `ytsearch[N]:<query>` extractor with `N = 50` by default and `N` capped at `100`.

#### Scenario: Default 50-entry expansion
- **WHEN** the user pastes a search URL and no `count` parameter is provided
- **THEN** the sidecar returns up to 50 entries in the order yt-dlp returns them (typically best-match first)

#### Scenario: Empty query is rejected
- **WHEN** the user pastes a URL with an empty `search_query` parameter
- **THEN** the sidecar returns `type: "error"` with `code: "INVALID_URL"` and a message identifying the missing query

#### Scenario: Resolution failure surfaces as error
- **WHEN** yt-dlp fails to resolve the search query (network error, rate limit, bot challenge)
- **THEN** the sidecar returns `type: "error"` with the same error code the single-video branch would return (e.g. `NETWORK`, `BOT_CHALLENGE`)

### Requirement: Picker routes search results identically to playlists
The frontend MUST route a probe result with `type: "search"` to the existing VideoPicker exactly the same way it routes a `type: "playlist"` result.

#### Scenario: Search result opens the picker
- **WHEN** `probe_url` returns `type: "search"` with at least one entry
- **THEN** the frontend switches `activeView` to `'picker'` and renders `VideoPicker` with the entries, identical to the playlist branch

#### Scenario: Picker label distinguishes search from playlist
- **WHEN** the picker renders entries from a `type: "search"` probe
- **THEN** the picker header reads "Search: <query>" using the `query` field the sidecar extracts from the original URL

#### Scenario: Empty entries do not open the picker
- **WHEN** `probe_url` returns `type: "search"` with `entries: []`
- **THEN** the frontend surfaces an error "No videos found for this search" instead of opening an empty picker

### Requirement: Search-results batch transcription
The queue runner MUST process the videos selected from a search-results picker in the order they appear in the picker's selection array, transcribing each via the existing `run_sidecar` flow.

#### Scenario: Selected videos queue sequentially
- **WHEN** the user selects 3 videos from a search-results picker and clicks Transcribe
- **THEN** the queue runs each video in turn, mirroring the existing playlist-batching behaviour, and the Queue view shows each item with its own progress and result

#### Scenario: Mixed search + history records coexist
- **WHEN** the user batches a search-results selection that includes URLs already present in history
- **THEN** the existing duplicate-detection logic flags them before the batch starts, identical to the playlist-batching flow