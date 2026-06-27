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

  const dispatch = createEventDispatcher();

  /** @type {Set<string>} */
  let selectedIds = new Set(entries.map((e) => e.id));

  $: allSelected = entries.length > 0 && selectedIds.size === entries.length;
  $: someSelected = selectedIds.size > 0 && selectedIds.size < entries.length;
  $: selectedCount = selectedIds.size;

  // Keep parent in sync with selected count
  $: dispatch('selectionChange', { count: selectedCount });

  function toggleAll() {
    if (allSelected) {
      selectedIds = new Set();
    } else {
      selectedIds = new Set(entries.map((e) => e.id));
    }
    selectedIds = selectedIds; // trigger reactivity
  }

  /** @param {string} id */
  function toggleRow(id) {
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedIds = next;
  }

  function handleStart() {
    const selected = entries.filter((e) => selectedIds.has(e.id));
    dispatch('startJob', { selected });
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
      selectedIds = new Set(entries.map((en) => en.id));
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
          {#if selectedCount === entries.length}
            {entries.length} videos
          {:else}
            {selectedCount} of {entries.length} selected
          {/if}
        </span>
      </label>
    </header>

    <div class="video-list" role="list">
      {#each entries as entry (entry.id)}
        {@const isSelected = selectedIds.has(entry.id)}
        <div
          class="video-row"
          class:selected={isSelected}
          role="listitem"
          tabindex="0"
          on:click={() => toggleRow(entry.id)}
          on:keydown={(e) => handleRowKey(e, entry.id)}
          aria-label="{entry.title}, {isSelected ? 'selected' : 'not selected'}"
        >
          <input
            type="checkbox"
            class="check"
            checked={isSelected}
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
          {#if entry.duration}
            <span class="video-duration">{fmtDuration(entry.duration)}</span>
          {/if}
        </div>
      {/each}
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

  .select-all-row {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    user-select: none;
  }

  .select-all-label {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-2);
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
</style>
