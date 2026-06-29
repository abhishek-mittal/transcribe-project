<script>
  import { createEventDispatcher } from 'svelte';

  /** @type {Array<{id: string, title: string, thumbnail: string, duration: number, url: string}>} */
  export let entries = [];
  export let playlistTitle = '';
  export let uploader = '';
  /**
   * Whether this picker is sourcing from a playlist or a YouTube search
   * results page. The header label adapts accordingly.
   * @type {'playlist' | 'search'}
   */
  export let kind = 'playlist';
  /**
   * The search query string. Only used when kind === 'search'.
   * @type {string}
   */
  export let searchQuery = '';
  /**
   * Total entries available at the source (e.g. full channel length), when
   * known. `null` when the source doesn't expose a count without a full
   * scan (most channel tabs). Drives the "Load N more" button and the
   * "20 of N videos" header label.
   * @type {number | null}
   */
  export let totalCount = null;
  /** True while a "Load more" page request is in flight. */
  export let loadingMore = false;
  /** Set when the last "Load more" request failed — shows a retry prompt. */
  export let loadMoreError = false;
  /**
   * URLs already present in transcription history. Matched against each
   * entry's `url` (not title) to mark already-transcribed rows.
   * @type {Set<string>}
   */
  export let transcribedUrls = new Set();

  const dispatch = createEventDispatcher();

  /**
   * Plain function, not a `$:` reactive declaration — it's referenced
   * from a non-reactive `let selectedIds = ...` initializer below, which
   * runs during component construction before any `$:` block has had a
   * chance to assign `isTranscribed`. Making it reactive previously threw
   * "isTranscribed is not a function" on first render (it still reads
   * `transcribedUrls` fresh on every call via closure, so no reactivity
   * is lost).
   * @param {{id: string, url: string}} entry
   */
  function isTranscribed(entry) {
    return transcribedUrls.has(entry.url);
  }
  $: selectableEntries = entries.filter((e) => !isTranscribed(e));
  $: transcribedCount = entries.length - selectableEntries.length;

  let hideTranscribed = false;

  /** @type {Set<string>} */
  let selectedIds = new Set(entries.filter((e) => !isTranscribed(e)).map((e) => e.id));

  // Re-sync selection when a new entry list arrives (initial probe or after
  // "Load more" appends a page) — newly appended non-transcribed entries
  // start selected, already-known selections are preserved.
  let prevEntries = entries;
  $: {
    if (entries !== prevEntries) {
      const known = new Set(prevEntries.map((e) => e.id));
      const next = new Set(selectedIds);
      for (const e of entries) {
        if (!known.has(e.id) && !isTranscribed(e)) next.add(e.id);
      }
      selectedIds = next;
      prevEntries = entries;
    }
  }

  $: allSelected = selectableEntries.length > 0 && selectableEntries.every((e) => selectedIds.has(e.id));
  $: someSelected = selectedIds.size > 0 && !allSelected;
  $: selectedCount = selectedIds.size;

  // Keep parent in sync with selected count AND the actual selected entries
  // — the "Transcribe N videos" footer button needs the entries themselves,
  // not just the count, to build the queue job (see F12 spec section C).
  $: selectedEntries = entries.filter((e) => selectedIds.has(e.id));
  $: dispatch('selectionChange', { count: selectedCount, selected: selectedEntries });

  function toggleAll() {
    if (allSelected) {
      selectedIds = new Set();
    } else {
      selectedIds = new Set(selectableEntries.map((e) => e.id));
    }
    selectedIds = selectedIds; // trigger reactivity
  }

  /** @param {string} id */
  function toggleRow(id) {
    const entry = entries.find((e) => e.id === id);
    if (entry && isTranscribed(entry)) return;
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedIds = next;
  }

  function handleStart() {
    dispatch('startJob', { selected: selectedEntries });
  }

  function handleLoadMore() {
    dispatch('loadMore');
  }

  /** @param {number} secs */
  function fmtDuration(secs) {
    if (!secs) return '';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  /** @param {KeyboardEvent} e */
  function handleRowKey(e, id) {
    if (e.key === ' ') {
      e.preventDefault();
      toggleRow(id);
    }
  }

  /** @param {KeyboardEvent} e */
  function handleGlobalKey(e) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'a') {
      e.preventDefault();
      selectedIds = new Set(selectableEntries.map((en) => en.id));
      selectedIds = selectedIds;
    }
  }
</script>

<svelte:window on:keydown={handleGlobalKey} />

<section class="video-picker" aria-label="Video picker">
  {#if entries.length === 0}
    <div class="empty-state">
      <p class="empty-title">
        {#if kind === 'search'}
          No videos found for this search.
        {:else}
          No videos found in this playlist.
        {/if}
      </p>
    </div>
  {:else}
    <header class="picker-header">
      {#if kind === 'search' && searchQuery}
        <p class="picker-subtitle">Search: <strong>{searchQuery}</strong></p>
      {:else if playlistTitle}
        <p class="picker-subtitle">Playlist: <strong>{playlistTitle}</strong></p>
      {/if}
      <div class="header-row">
        <label class="select-all-row">
          <input
            type="checkbox"
            class="check"
            checked={allSelected}
            indeterminate={someSelected}
            on:change={toggleAll}
            aria-label="Select all videos"
          />
          <span class="select-all-label">
            {#if selectedCount === selectableEntries.length}
              {selectableEntries.length} of {selectableEntries.length} selectable{#if transcribedCount > 0} ({transcribedCount} already transcribed){/if}
            {:else}
              {selectedCount} of {selectableEntries.length} selected{#if transcribedCount > 0} ({transcribedCount} already transcribed){/if}
            {/if}
          </span>
        </label>
        <span class="entry-count-label">
          {#if totalCount != null && totalCount > entries.length}
            {entries.length} of {totalCount} videos
          {:else}
            {entries.length} videos
          {/if}
        </span>
      </div>
      {#if transcribedCount > 0}
        <label class="filter-row">
          <input
            type="checkbox"
            class="check"
            bind:checked={hideTranscribed}
            aria-label="Hide already transcribed videos"
          />
          <span class="filter-label">Hide transcribed ({transcribedCount})</span>
        </label>
      {/if}
    </header>

    {@const visibleEntries = hideTranscribed ? entries.filter((e) => !isTranscribed(e)) : entries}
    <div class="video-list" role="list">
      {#each visibleEntries as entry (entry.id)}
        {@const isSelected = selectedIds.has(entry.id)}
        {@const isDone = isTranscribed(entry)}
        <div
          class="video-row"
          class:selected={isSelected}
          class:transcribed={isDone}
          role="listitem"
          tabindex="0"
          on:click={() => toggleRow(entry.id)}
          on:keydown={(e) => handleRowKey(e, entry.id)}
          aria-label="{entry.title}, {isDone ? 'already transcribed' : isSelected ? 'selected' : 'not selected'}"
        >
          <input
            type="checkbox"
            class="check"
            checked={isSelected}
            disabled={isDone}
            on:change={() => toggleRow(entry.id)}
            on:click|stopPropagation
            tabindex="-1"
            aria-hidden="true"
          />
          <div class="thumb-wrap">
            <img
              class="thumb"
              src={entry.thumbnail}
              alt=""
              loading="lazy"
              on:error={(e) => {
                e.currentTarget.style.display = 'none';
                e.currentTarget.nextElementSibling.style.display = 'grid';
              }}
            />
            <div class="thumb-placeholder" style="display:none" aria-hidden="true">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                <rect x="2" y="4" width="20" height="16" rx="2" stroke="currentColor" stroke-width="1.4"/>
                <path d="M10 9l5 3-5 3V9z" fill="currentColor"/>
              </svg>
            </div>
          </div>
          <span class="video-title">{entry.title}</span>
          {#if isDone}
            <span class="transcribed-badge">✓ Transcribed</span>
          {:else if entry.duration}
            <span class="video-duration">{fmtDuration(entry.duration)}</span>
          {/if}
        </div>
      {/each}
      {#if totalCount == null || entries.length < totalCount}
        <div class="load-more-row">
          {#if loadingMore}
            <span class="load-more-spinner" aria-label="Loading more videos">
              <span class="spinner-inline"></span> Loading…
            </span>
          {:else if loadMoreError}
            <button type="button" class="load-more-btn error" on:click={handleLoadMore}>
              Failed to load more. Retry?
            </button>
          {:else}
            <button type="button" class="load-more-btn" on:click={handleLoadMore}>
              {#if totalCount != null}
                Load 20 more ({totalCount - entries.length} remaining)
              {:else}
                Load more
              {/if}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .video-picker {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border-radius: 12px;
    border: 1px solid var(--glass-border-soft);
    overflow: hidden;
    min-height: 0;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
  }
  .empty-title {
    font-size: 13.5px;
    color: var(--text-3);
    margin: 0;
  }

  .picker-header {
    padding: 10px 16px;
    border-bottom: 1px solid var(--glass-border-soft);
    background: var(--surface-1);
    flex-shrink: 0;
  }

  .picker-subtitle {
    font-size: 12px;
    color: var(--text-3);
    margin: 0 0 6px;
    line-height: 1.4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .picker-subtitle strong {
    color: var(--text-1);
    font-weight: 600;
  }

  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .select-all-row {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    user-select: none;
    min-width: 0;
  }

  .select-all-label {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-count-label {
    font-size: 11.5px;
    color: var(--text-3);
    flex-shrink: 0;
    white-space: nowrap;
  }

  .filter-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
    cursor: pointer;
    user-select: none;
  }
  .filter-label {
    font-size: 12px;
    color: var(--text-3);
  }

  .check {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .video-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .video-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    height: 48px;
    cursor: pointer;
    transition: background 0.12s;
    border-bottom: 1px solid var(--glass-border-soft);
    outline: none;
  }
  .video-row:last-child {
    border-bottom: none;
  }
  .video-row:hover {
    background: var(--surface-2);
  }
  .video-row.selected {
    background: var(--surface-3);
  }
  .video-row:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .video-row.transcribed {
    opacity: 0.45;
    pointer-events: none;
    cursor: default;
  }

  .thumb-wrap {
    position: relative;
    width: 56px;
    height: 32px;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-3);
  }
  .thumb {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .thumb-placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--surface-3);
    color: var(--text-3);
  }

  .video-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .video-duration {
    font-size: 11.5px;
    color: var(--text-3);
    flex-shrink: 0;
    white-space: nowrap;
  }

  .transcribed-badge {
    font-size: 11px;
    font-weight: 500;
    color: #4ade80;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .load-more-row {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px 14px;
  }

  .load-more-btn {
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 7px;
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-2);
    padding: 7px 16px;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
  }
  .load-more-btn:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .load-more-btn.error {
    color: #f59e0b;
    border-color: rgba(245, 158, 11, 0.3);
  }
  .load-more-btn.error:hover {
    background: rgba(245, 158, 11, 0.08);
  }

  .load-more-spinner {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .spinner-inline {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 1.5px solid var(--glass-border-soft);
    border-top-color: var(--text-2);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
