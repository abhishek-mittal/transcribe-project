<script>
  import { createEventDispatcher } from 'svelte';
  import { fly } from 'svelte/transition';
  import TranscriptPanel from './TranscriptPanel.svelte';
  import ActivityLogPanel from './ActivityLogPanel.svelte';

  /**
   * @typedef {'waiting'|'starting'|'downloading'|'transcribing'|'done'|'failed'|'cancelled'} JobItemStatus
   * @typedef {{ id: string, url: string, title: string, thumbnail: string, duration: number, status: JobItemStatus, error: string|null, errorCode: string|null, result: any, startedAt: string|null, completedAt: string|null, wordCount: number|null, downloadPercent?: number|null, downloadedBytes?: number|null, totalBytes?: number|null, speedBps?: number|null, etaSecs?: number|null, streamSegments?: any[] }} JobItem
   */

  /** @type {JobItem[]} */
  export let items = [];
  export let timestamps = true;
  /**
   * Live activity-log entries for the panel below the queue list.
   * @type {Array<{ id: number, ts: string, severity: 'info'|'success'|'warn'|'error', message: string }>}
   */
  export let activityEntries = [];
  /** Callbacks for the activity log panel (Clear / Copy). */
  export let activityHandlers = {};

  const dispatch = createEventDispatcher();

  // Local tab state for the slide-in TranscriptPanel. Previously this was
  // `export let activeTab` with `bind:activeTab` so the parent's tab state
  // stayed in sync, but having three different components all bind to the
  // same parent variable (transcribe tab, queue tab, history tab) caused
  // Svelte 4's binding tracker to hold stale references across
  // mount/unmount cycles and freeze the UI on Queue click. Now it's local.
  /** @type {'plain'|'timestamped'|'srt'} */
  let activeTab = 'plain';

  function handleTabChange(t) {
    activeTab = t;
  }

  /** @type {string | null} */
  let selectedItemId = null;
  let copied = false;

  $: selectedItem = items.find((i) => i.id === selectedItemId) ?? null;
  $: doneCount = items.filter((i) => i.status === 'done').length;
  $: allTerminal = items.length > 0 && items.every((i) => ['done', 'failed', 'cancelled'].includes(i.status)) && !items.some((i) => i.status === 'starting');

  function selectItem(item) {
    if (item.status !== 'done') return;
    selectedItemId = selectedItemId === item.id ? null : item.id;
  }

  function getActiveContent() {
    if (!selectedItem?.result) return '';
    if (activeTab === 'plain') return selectedItem.result.plain || '';
    if (activeTab === 'timestamped') return selectedItem.result.timestamped || '';
    if (activeTab === 'srt') return selectedItem.result.srt || '';
    return '';
  }

  function handleCopy() {
    const content = getActiveContent();
    if (!content) return;
    navigator.clipboard.writeText(content).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 2000);
    });
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

  /**
   * Clamp a value to 0–100. Returns null when the value is missing/non-numeric.
   * @param {unknown} value
   */
  function clampPercent(value) {
    if (typeof value !== 'number' || !Number.isFinite(value)) return null;
    return Math.max(0, Math.min(100, value));
  }

  /**
   * Human-readable byte count with adaptive units (B / KB / MB).
   * @param {unknown} bytes
   */
  function fmtBytes(bytes) {
    if (typeof bytes !== 'number' || !Number.isFinite(bytes) || bytes <= 0) return null;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
    return `${(kb / 1024).toFixed(kb / 1024 < 10 ? 2 : 1)} MB`;
  }

  /**
   * Human-readable speed (bytes/sec → KB/s or MB/s).
   * @param {unknown} bps
   */
  function fmtSpeed(bps) {
    if (typeof bps !== 'number' || !Number.isFinite(bps) || bps <= 0) return null;
    const kbps = bps / 1024;
    if (kbps < 1024) return `${kbps.toFixed(kbps < 10 ? 1 : 0)} KB/s`;
    return `${(kbps / 1024).toFixed(1)} MB/s`;
  }

  /**
   * Compact ETA formatter — "12s", "3m 04s", or "—" when unknown.
   * @param {unknown} secs
   */
  function fmtEta(secs) {
    if (typeof secs !== 'number' || !Number.isFinite(secs) || secs < 0) return null;
    const s = Math.round(secs);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    const r = s % 60;
    return `${m}m ${String(r).padStart(2, '0')}s`;
  }

  /**
   * Compose the secondary line under the downloading row's progress bar.
   * Examples:
   *   "5.2 MB / 12.4 MB · ↓ 1.8 MB/s · ETA 4s"
   *   "5.2 MB / 12.4 MB"   (no speed yet)
   *   "62%"                (total unknown)
   * @param {JobItem} item
   */
  function downloadDetail(item) {
    const parts = [];
    const dl = fmtBytes(item.downloadedBytes);
    const tot = fmtBytes(item.totalBytes);
    if (dl && tot) parts.push(`${dl} / ${tot}`);
    else if (dl) parts.push(dl);
    const sp = fmtSpeed(item.speedBps);
    if (sp) parts.push(`↓ ${sp}`);
    const eta = fmtEta(item.etaSecs);
    if (eta) parts.push(`ETA ${eta}`);
    return parts.join(' · ');
  }

  /** @param {KeyboardEvent} e */
  function handleKey(e) {
    if (e.key === 'Escape') {
      selectedItemId = null;
    }
  }
</script>

<svelte:window on:keydown={handleKey} />

<div class="queue-view">
  <div class="queue-pane" class:has-panel={!!selectedItem}>
    <div class="queue-list">
      <header class="queue-header">
        <div class="queue-title">
          {#if allTerminal}
            Queue · ✓ All done · {doneCount} of {items.length}
          {:else}
            Queue · {doneCount} of {items.length} complete
          {/if}
        </div>
        <div class="queue-actions">
          {#if allTerminal}
            <button class="btn-ghost" on:click={() => dispatch('viewHistory')}>
              View in History
            </button>
          {:else}
            <button class="btn-ghost btn-cancel" on:click={() => dispatch('cancelJob')}>
              Cancel job
            </button>
          {/if}
        </div>
      </header>

    {#if items.length === 0}
      <div class="empty-state">
        <div class="empty-icon">
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <rect x="3" y="4" width="18" height="3" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
            <rect x="3" y="10.5" width="18" height="3" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
            <rect x="3" y="17" width="18" height="3" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
          </svg>
        </div>
        <p class="empty-title">No active job.</p>
        <p class="empty-desc">Start one from the Transcribe tab.</p>
      </div>
    {:else}
      <div class="item-list" role="list">
        {#each items as item, i (item.id)}
          {@const isActive = item.status === 'starting' || item.status === 'downloading' || item.status === 'transcribing'}
          {@const isDone = item.status === 'done'}
          {@const isFailed = item.status === 'failed'}
          {@const isCancelled = item.status === 'cancelled'}
          {@const isSelected = selectedItemId === item.id}
          <div
            class="queue-row"
            class:clickable={isDone}
            class:selected={isSelected}
            class:active-row={isActive}
            role="listitem"
            on:click={() => isDone && selectItem(item)}
            on:keydown={(e) => e.key === 'Enter' && isDone && selectItem(item)}
            tabindex={isDone ? 0 : -1}
            aria-selected={isDone ? isSelected : undefined}
          >
            <span class="row-num">{i + 1}</span>

            <div class="row-thumb">
              <img
                src={item.thumbnail}
                alt=""
                loading="lazy"
                on:error={(e) => {
                  e.currentTarget.style.display = 'none';
                  e.currentTarget.nextElementSibling.style.display = 'grid';
                }}
              />
              <div class="thumb-placeholder" style="display:none" aria-hidden="true">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
                  <rect x="2" y="4" width="20" height="16" rx="2" stroke="currentColor" stroke-width="1.4"/>
                  <path d="M10 9l5 3-5 3V9z" fill="currentColor"/>
                </svg>
              </div>
            </div>

            <div class="row-body">
              <span class="row-title">{item.title}</span>
              {#if isFailed && item.error}
                <span class="row-error-detail">{item.error}</span>
              {/if}
            </div>

            <div class="row-status">
              {#if item.status === 'waiting'}
                <span class="status-text waiting">○ Waiting</span>
                <button
                  class="cancel-item-btn"
                  aria-label="Remove from queue"
                  on:click|stopPropagation={() => dispatch('cancelItem', { id: item.id })}
                  title="Remove from queue"
                >
                  <svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                    <path d="M2 2L10 10M10 2L2 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                  </svg>
                </button>
              {:else if item.status === 'starting'}
                <div class="status-downloading">
                  <div class="dl-bar" aria-label="Starting">
                    <div class="dl-bar-fill indeterminate"></div>
                  </div>
                  <div class="status-downloading-meta">
                    <span class="status-text downloading">⟳ Starting…</span>
                  </div>
                  <button
                    class="cancel-active-btn"
                    aria-label="Cancel"
                    title="Cancel"
                    on:click|stopPropagation={() => dispatch('cancelItem', { id: item.id })}
                  >
                    <svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                      <path d="M2 2L10 10M10 2L2 10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                    </svg>
                  </button>
                </div>
              {:else if item.status === 'downloading'}
                {#if true}
                  {@const pct = clampPercent(item.downloadPercent)}
                  {@const detail = downloadDetail(item)}
                  <div class="status-downloading">
                    <div class="dl-bar" data-ready={pct !== null} aria-label="Download progress">
                      <div
                        class="dl-bar-fill"
                        style="width: {pct ?? 0}%"
                      ></div>
                    </div>
                    <div class="status-downloading-meta">
                      <span class="status-text downloading">
                        {#if pct !== null}
                          ↓ {pct.toFixed(pct >= 100 ? 0 : 1)}%
                        {:else}
                          ↓ Downloading…
                        {/if}
                      </span>
                      {#if detail}
                        <span class="status-detail">{detail}</span>
                      {/if}
                    </div>
                    <button
                      class="cancel-active-btn"
                      aria-label="Cancel download"
                      title="Cancel download"
                      on:click|stopPropagation={() => dispatch('cancelItem', { id: item.id })}
                    >
                      <svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                        <path d="M2 2L10 10M10 2L2 10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                      </svg>
                    </button>
                  </div>
                {/if}
              {:else if item.status === 'transcribing'}
                <div class="status-transcribing">
                  <div class="progress-bar">
                    <div class="progress-fill"></div>
                  </div>
                  <span class="status-text transcribing">
                    ✦ {item.streamSegments?.length ?? 0} segs
                  </span>
                </div>
              {:else if item.status === 'done'}
                <span class="status-text done">
                  ✓ Done · {item.wordCount ? `${(item.wordCount / 1000).toFixed(1)}k words` : ''}
                </span>
              {:else if item.status === 'failed'}
                <div class="status-failed">
                  <span class="status-text failed">✗ Error</span>
                  <button
                    class="retry-btn"
                    aria-label="Retry"
                    on:click|stopPropagation={() => dispatch('retryItem', { id: item.id })}
                    title="Retry"
                  >↺ Retry</button>
                </div>
              {:else if item.status === 'cancelled'}
                <div class="status-cancelled">
                  <span class="status-text cancelled">— Cancelled</span>
                  <button
                    class="retry-btn"
                    aria-label="Retry"
                    on:click|stopPropagation={() => dispatch('retryItem', { id: item.id })}
                    title="Re-queue this item"
                  >↺ Retry</button>
                </div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
    </div>

    <ActivityLogPanel entries={activityEntries} handlers={activityHandlers} />
  </div>

  {#if selectedItem?.result}
    <div class="transcript-panel-wrap" transition:fly={{ x: 300, duration: 200 }}>
      <TranscriptPanel
        result={selectedItem.result}
        activeTab={activeTab}
        defaultName={selectedItem.title}
        {timestamps}
        onTabChange={handleTabChange}
        onCopy={handleCopy}
      />
    </div>
  {/if}
</div>

<style>
  .queue-view {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  .queue-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    /* Sit flush against the transcript panel on the right. */
  }
  .queue-pane.has-panel {
    /* Match the existing has-panel transition so the right pane animates
       in cleanly without the left pane jumping. */
    flex: 1;
  }

  .queue-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--surface-1);
    border-radius: 12px;
    border: 1px solid var(--glass-border-soft);
    overflow: hidden;
    transition: flex 0.2s;
  }

  .queue-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid var(--glass-border-soft);
    flex-shrink: 0;
  }

  .queue-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .queue-actions {
    display: flex;
    gap: 8px;
  }

  .btn-ghost {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--surface-2);
    color: var(--text-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 6px;
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    padding: 5px 10px;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
  }
  .btn-ghost:hover { color: var(--text); background: var(--surface-3); }
  .btn-cancel { color: var(--error); border-color: var(--error-border); }
  .btn-cancel:hover { background: var(--error-bg); color: var(--error); }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 32px;
    color: var(--text-3);
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

  .item-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .queue-row {
    display: grid;
    grid-template-columns: 28px 56px 1fr 220px;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--glass-border-soft);
    transition: background 0.12s;
    position: relative;
  }
  .queue-row:last-child { border-bottom: none; }
  .queue-row.clickable { cursor: pointer; }
  .queue-row.clickable:hover { background: var(--surface-2); }
  .queue-row.selected { background: var(--surface-3); }
  .queue-row.active-row { background: rgba(var(--accent), 0.04); }

  .row-num {
    font-size: 11px;
    color: var(--text-3);
    text-align: right;
  }

  .row-thumb {
    position: relative;
    width: 56px;
    height: 32px;
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-3);
    flex-shrink: 0;
  }
  .row-thumb img {
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
    color: var(--text-3);
  }

  .row-body {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-error-detail {
    font-size: 11px;
    color: var(--error);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-status {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }

  .status-text {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
  }
  .status-text.waiting { color: var(--text-3); }
  .status-text.downloading { color: var(--text); font-variant-numeric: tabular-nums; }
  .status-text.transcribing { color: var(--text-2); font-size: 11px; }
  .status-text.done { color: #4ade80; }
  .status-text.failed { color: var(--error); }
  .status-text.cancelled { color: var(--text-3); }

  .pulse { animation: pulse 1.4s ease-in-out infinite; }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }

  .status-downloading {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-areas:
      "bar bar"
      "meta btn";
    column-gap: 8px;
    row-gap: 3px;
    align-items: center;
    min-width: 0;
  }
  .status-downloading .dl-bar { grid-area: bar; }
  .status-downloading .status-downloading-meta { grid-area: meta; }
  .status-downloading .cancel-active-btn { grid-area: btn; }

  .dl-bar {
    width: 100%;
    height: 4px;
    background: var(--surface-3);
    border-radius: 99px;
    overflow: hidden;
    position: relative;
  }

  .dl-bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 99px;
    /* Width is driven by inline style; this transition smooths the
       per-tick updates (≈2 Hz from yt-dlp) without lag. */
    transition: width 0.18s ease-out;
  }

  /* Indeterminate shimmer used until the first real percent arrives. */
  .dl-bar-fill::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(255, 255, 255, 0.18) 50%,
      transparent 100%
    );
    animation: dl-shimmer 1.4s ease-in-out infinite;
    opacity: 0;
    transition: opacity 0.2s;
  }
  .dl-bar:not([data-ready="true"]) .dl-bar-fill::after { opacity: 1; }

  /* "Starting..." phase: indeterminate travelling bar */
  .dl-bar-fill.indeterminate {
    width: 30%;
    animation: dl-slide 1.6s ease-in-out infinite;
  }
  @keyframes dl-slide {
    0%   { margin-left: 0%;   width: 30%; }
    50%  { margin-left: 70%;  width: 30%; }
    100% { margin-left: 0%;   width: 30%; }
  }

  @keyframes dl-shimmer {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(100%); }
  }

  .status-detail {
    font-size: 10.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-transcribing {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 3px;
  }
  .progress-bar {
    width: 80px;
    height: 3px;
    background: var(--surface-3);
    border-radius: 99px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 99px;
    animation: progress-pulse 1.5s ease-in-out infinite;
    width: 60%;
  }
  @keyframes progress-pulse {
    0% { width: 20%; }
    50% { width: 80%; }
    100% { width: 20%; }
  }

  .status-failed {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .status-cancelled {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .status-downloading-meta {
    display: flex;
    align-items: baseline;
    justify-content: flex-end;
    gap: 6px;
    min-width: 0;
  }
  .status-downloading-meta .status-text { flex-shrink: 0; }

  .cancel-active-btn {
    background: none;
    border: 1px solid var(--glass-border-soft);
    color: var(--text-3);
    padding: 3px;
    border-radius: 4px;
    cursor: pointer;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    transition: color 0.12s, border-color 0.12s, background 0.12s;
  }
  .cancel-active-btn:hover {
    color: var(--error);
    border-color: var(--error-border, var(--glass-border-soft));
    background: var(--error-bg, var(--surface-2));
  }

  .retry-btn {
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 4px;
    font-family: inherit;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-2);
    padding: 2px 7px;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
  }
  .retry-btn:hover {
    color: var(--text);
    background: var(--surface-3);
  }

  .cancel-item-btn {
    background: none;
    border: none;
    color: var(--text-3);
    padding: 4px;
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s, color 0.15s;
  }
  .queue-row:hover .cancel-item-btn { opacity: 1; }
  .cancel-item-btn:hover { color: var(--error); }

  /* ── Right panel ──────────────────────────────────────────── */
  .transcript-panel-wrap {
    width: 420px;
    flex-shrink: 0;
    display: flex;
    margin-left: 14px;
    min-height: 0;
  }
</style>
