<script>
  import { createEventDispatcher } from 'svelte';

  export let records = [];      // Array<{ id, url, language, plain, timestamped, srt, word_count, created_at }>
  export let selectedId = null;

  const dispatch = createEventDispatcher();

  let query = '';

  $: filtered = !query.trim()
    ? records
    : records.filter((r) => {
        const q = query.trim().toLowerCase();
        return r.url?.toLowerCase().includes(q) || r.plain?.toLowerCase().includes(q);
      });

  function hostnameAndPath(url) {
    try {
      const u = new URL(url);
      return `${u.hostname}${u.pathname}`;
    } catch {
      return url;
    }
  }

  function relativeDate(iso) {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return '';
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffHours = diffMs / (1000 * 60 * 60);
    if (diffHours < 1) return 'Just now';
    if (diffHours < 24) return `${Math.floor(diffHours)} hour${Math.floor(diffHours) === 1 ? '' : 's'} ago`;
    if (diffHours < 48) return 'Yesterday';
    return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function select(record) {
    dispatch('select', record);
  }

  /** @param {Event} e */
  function remove(e, id) {
    e.stopPropagation();
    dispatch('delete', id);
  }

  function clearSearch() {
    query = '';
  }
</script>

<section class="history-view">
  <header class="history-header">
    <div class="history-header-top">
      <h3 class="history-title">History</h3>
      <span class="count-chip">{records.length} transcript{records.length === 1 ? '' : 's'}</span>
    </div>
    <div class="search-wrapper">
      <svg class="search-icon" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <circle cx="7" cy="7" r="5" stroke="currentColor" stroke-width="1.4"/>
        <path d="M11 11L14.5 14.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
      </svg>
      <input
        type="text"
        class="search-input"
        placeholder="Search transcripts…"
        bind:value={query}
      />
    </div>
  </header>

  <div class="history-list">
    {#if records.length === 0}
      <div class="empty-state">
        <div class="empty-icon">
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <p class="empty-title">No transcriptions yet</p>
        <p class="empty-desc">Head to the Transcribe tab to get started.</p>
      </div>
    {:else if filtered.length === 0}
      <div class="empty-state">
        <p class="empty-title">No results for &lsquo;{query}&rsquo;</p>
        <button type="button" class="clear-search-link" on:click={clearSearch}>Clear search</button>
      </div>
    {:else}
      {#each filtered as record (record.id)}
        <button
          type="button"
          class="history-item"
          class:active={selectedId === record.id}
          on:click={() => select(record)}
        >
          <div class="item-line1">
            <svg class="link-icon" width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M6.5 1.5h-3A2 2 0 001.5 3.5v9A2 2 0 003.5 14.5h9A2 2 0 0014.5 12.5v-3M14.5 1.5h-4m4 0v4m0-4L8 8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <span class="item-url">{hostnameAndPath(record.url)}</span>
          </div>
          <div class="item-line2">
            <span class="lang-pill">{(record.language || '').toUpperCase()}</span>
            <span class="dot-sep">·</span>
            <span>{(record.word_count ?? 0).toLocaleString()} words</span>
            <span class="dot-sep">·</span>
            <span>{relativeDate(record.created_at)}</span>
          </div>
          <button
            type="button"
            class="trash-btn"
            aria-label="Delete transcript"
            on:click={(e) => remove(e, record.id)}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M2.5 4.5h11M5.5 4.5V3a1 1 0 011-1h3a1 1 0 011 1v1.5m-7 0L4 13a1 1 0 001 1h6a1 1 0 001-1l-.5-8.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
        </button>
      {/each}
    {/if}
  </div>
</section>

<style>
  .history-view {
    width: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .history-header {
    padding: 16px 16px 12px;
    border-bottom: 1px solid var(--glass-border-soft);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .history-header-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .history-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }
  .count-chip {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-3);
    background: var(--surface-2);
    padding: 3px 9px;
    border-radius: 999px;
  }
  .search-wrapper {
    position: relative;
  }
  .search-icon {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-3);
    pointer-events: none;
  }
  .search-input {
    width: 100%;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 8px;
    padding: 7px 10px 7px 30px;
    font-family: inherit;
    font-size: 12.5px;
    color: var(--text);
  }
  .search-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .history-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .history-item {
    position: relative;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 8px;
    padding: 10px 36px 10px 12px;
    cursor: pointer;
    font-family: inherit;
    transition: background 0.15s;
  }
  .history-item:hover {
    background: var(--surface-2);
  }
  .history-item.active {
    background: var(--surface-3);
  }
  .item-line1 {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .link-icon {
    color: var(--text-3);
    flex-shrink: 0;
  }
  .item-url {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item-line2 {
    margin-top: 4px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .lang-pill {
    font-size: 10px;
    font-weight: 600;
    background: var(--surface-3);
    padding: 1px 6px;
    border-radius: 999px;
    color: var(--text-2);
  }
  .dot-sep { opacity: 0.6; }

  .trash-btn {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    background: transparent;
    border: none;
    color: var(--text-3);
    padding: 6px;
    border-radius: 6px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s, color 0.15s, background 0.15s;
  }
  .history-item:hover .trash-btn,
  .history-item.active .trash-btn {
    opacity: 1;
  }
  .trash-btn:hover {
    color: var(--error);
    background: var(--error-bg);
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 32px;
    color: var(--text-3);
    gap: 4px;
  }
  .empty-icon {
    width: 56px;
    height: 56px;
    border-radius: 12px;
    background: var(--surface-2);
    display: grid;
    place-items: center;
    margin-bottom: 8px;
    color: var(--text-2);
  }
  .empty-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-2);
    margin: 0;
  }
  .empty-desc {
    font-size: 12px;
    color: var(--text-3);
    margin: 0;
  }
  .clear-search-link {
    background: none;
    border: none;
    color: var(--accent);
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    text-decoration: underline;
    margin-top: 4px;
  }
</style>
