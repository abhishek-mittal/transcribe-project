<script>
  import { createEventDispatcher } from 'svelte';

  /**
   * @typedef {{ id: string, url: string, title: string, thumbnail: string, duration_secs: number, status: string, error_code: string|null, error_message: string|null, language: string|null, plain: string|null, timestamped: string|null, srt: string|null, word_count: number|null, started_at: string|null, completed_at: string|null, elapsed_ms: number|null }} JobItemRecord
   * @typedef {{ id: string, model: string, timestamps: boolean, created_at: string, completed_at: string, elapsed_ms: number, total_items: number, success_count: number, failure_count: number, cancelled_count: number, total_words: number, total_audio_secs: number, items: JobItemRecord[] }} JobRecord
   */

  /** @type {JobRecord[]} */
  export let jobs = [];

  const dispatch = createEventDispatcher();

  let query = '';
  /** @type {Set<string>} */
  let expandedIds = new Set();

  function toggleExpand(id) {
    const next = new Set(expandedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    expandedIds = next;
  }

  $: filtered = !query.trim()
    ? jobs
    : jobs.filter((j) => {
        const q = query.trim().toLowerCase();
        return (
          j.items.some(
            (i) =>
              i.title?.toLowerCase().includes(q) ||
              i.plain?.toLowerCase().includes(q) ||
              i.url?.toLowerCase().includes(q)
          )
        );
      });

  function clearSearch() {
    query = '';
  }

  /** @param {string} iso */
  function fmtDate(iso) {
    if (!iso) return '';
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) +
      ', ' + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  }

  /** @param {number} ms */
  function fmtElapsed(ms) {
    if (!ms) return '';
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    const rem = s % 60;
    return `${m}m${rem > 0 ? String(rem).padStart(2, '0') + 's' : ''}`;
  }

  /** @param {number} secs */
  function fmtAudio(secs) {
    if (!secs) return '0s';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s > 0 ? s + 's' : ''}`.trim();
    return `${s}s`;
  }

  /** @param {number} totalAudioSecs @param {number} elapsedMs */
  function fmtSpeed(totalAudioSecs, elapsedMs) {
    if (!totalAudioSecs || !elapsedMs) return '';
    const ratio = totalAudioSecs / (elapsedMs / 1000);
    return `${ratio.toFixed(1)}×`;
  }

  /** @param {JobRecord} job */
  function statusSummary(job) {
    if (job.failure_count === 0 && job.cancelled_count === 0) return '✓ All done';
    const parts = [];
    if (job.success_count > 0) parts.push(`✓ ${job.success_count} done`);
    if (job.failure_count > 0) parts.push(`✗ ${job.failure_count} failed`);
    if (job.cancelled_count > 0) parts.push(`— ${job.cancelled_count} cancelled`);
    return parts.join(' · ');
  }

  /** @param {JobItemRecord} item */
  function itemStatusLabel(item) {
    if (item.status === 'done') {
      return `${item.language?.toUpperCase() ?? ''} · ${(item.word_count ?? 0).toLocaleString()}w`;
    }
    if (item.status === 'failed') {
      if (item.error_code === 'BOT_CHALLENGE') return 'Bot challenge';
      if (item.error_code === 'NETWORK') return 'Network error';
      if (item.error_code === 'UNSUPPORTED_PLATFORM') return 'Unsupported';
      return 'Error';
    }
    if (item.status === 'cancelled') return 'Cancelled';
    return item.status;
  }

  /** @param {string|null} code */
  function humanError(code) {
    switch (code) {
      case 'BOT_CHALLENGE': return 'YouTube blocked this video. Try opening it in your browser first, then paste the URL again.';
      case 'NETWORK': return 'Network error while downloading. Check your connection and try again.';
      case 'UNSUPPORTED_PLATFORM': return 'This URL is not supported.';
      case 'MODEL_LOAD_FAILED': return 'Failed to load the speech model.';
      case 'FFMPEG_MISSING': return 'FFmpeg is required. Install with `brew install ffmpeg` and restart.';
      default: return 'An unexpected error occurred.';
    }
  }

  const MODEL_LABELS = { tiny: 'Tiny', base: 'Base', small: 'Small' };
</script>

<section class="job-history-view">
  <header class="jh-header">
    <div class="jh-header-top">
      <h3 class="jh-title">History</h3>
      <span class="count-chip">{jobs.length} job{jobs.length === 1 ? '' : 's'}</span>
    </div>
    <div class="search-wrapper">
      <svg class="search-icon" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <circle cx="7" cy="7" r="5" stroke="currentColor" stroke-width="1.4"/>
        <path d="M11 11L14.5 14.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
      </svg>
      <input
        type="text"
        class="search-input"
        placeholder="Search jobs…"
        bind:value={query}
        aria-label="Search jobs"
      />
    </div>
  </header>

  <div class="jh-list">
    {#if jobs.length === 0}
      <div class="empty-state">
        <div class="empty-icon">
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <rect x="3" y="4" width="18" height="3" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
            <rect x="3" y="10.5" width="18" height="3" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
            <rect x="3" y="17" width="18" height="3" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
          </svg>
        </div>
        <p class="empty-title">No jobs yet.</p>
        <p class="empty-desc">Start one from the Transcribe tab.</p>
      </div>
    {:else if filtered.length === 0}
      <div class="empty-state">
        <p class="empty-title">No results for &lsquo;{query}&rsquo;</p>
        <button type="button" class="clear-search-link" on:click={clearSearch}>Clear search</button>
      </div>
    {:else}
      {#each filtered as job (job.id)}
        {@const isExpanded = expandedIds.has(job.id)}
        <div class="job-entry" class:expanded={isExpanded}>
          <button
            type="button"
            class="job-row"
            on:click={() => toggleExpand(job.id)}
            aria-expanded={isExpanded}
          >
            <span class="chevron" class:rotated={isExpanded} aria-hidden="true">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M4 6L8 10L12 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </span>
            <div class="job-row-main">
              <span class="job-date">{fmtDate(job.created_at)}</span>
              <span class="job-stats">
                {job.total_items} video{job.total_items === 1 ? '' : 's'} ·
                {statusSummary(job)} ·
                {fmtElapsed(job.elapsed_ms)}
              </span>
            </div>
            <span class="model-pill">{MODEL_LABELS[job.model] || job.model}</span>
            <button
              type="button"
              class="delete-btn"
              aria-label="Delete job"
              on:click|stopPropagation={() => dispatch('deleteJob', { jobId: job.id })}
            >
              <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="M2.5 4.5h11M5.5 4.5V3a1 1 0 011-1h3a1 1 0 011 1v1.5m-7 0L4 13a1 1 0 001 1h6a1 1 0 001-1l-.5-8.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </button>
          </button>

          {#if isExpanded}
            <div class="job-detail">
              <!-- Analytics -->
              <div class="analytics-panel">
                <span class="analytics-item">
                  <strong>{(job.total_words ?? 0).toLocaleString()}</strong> words transcribed
                </span>
                <span class="dot-sep">·</span>
                <span class="analytics-item">
                  <strong>{fmtAudio(job.total_audio_secs)}</strong> audio processed
                </span>
                {#if job.total_audio_secs > 0 && job.elapsed_ms > 0}
                  <span class="dot-sep">·</span>
                  <span class="analytics-item">
                    <strong>{fmtSpeed(job.total_audio_secs, job.elapsed_ms)}</strong> realtime
                  </span>
                {/if}
              </div>

              <!-- Video rows -->
              <div class="video-rows">
                {#each job.items as item (item.id)}
                  <div class="video-item" class:done={item.status === 'done'} class:failed={item.status === 'failed'}>
                    <div class="vi-status-icon">
                      {#if item.status === 'done'}
                        <span class="icon-done" aria-label="Done">✓</span>
                      {:else if item.status === 'failed'}
                        <span class="icon-failed" aria-label="Failed">✗</span>
                      {:else}
                        <span class="icon-cancelled" aria-label="Cancelled">—</span>
                      {/if}
                    </div>
                    <div class="vi-thumb">
                      <img
                        src={item.thumbnail}
                        alt=""
                        loading="lazy"
                        on:error={(e) => {
                          e.currentTarget.style.display = 'none';
                          e.currentTarget.nextElementSibling.style.display = 'grid';
                        }}
                      />
                      <div class="vi-thumb-ph" style="display:none" aria-hidden="true">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none">
                          <rect x="2" y="4" width="20" height="16" rx="2" stroke="currentColor" stroke-width="1.4"/>
                        </svg>
                      </div>
                    </div>
                    <div class="vi-body">
                      <span class="vi-title">{item.title}</span>
                      {#if item.status === 'failed' && item.error_code}
                        <span class="vi-error-detail">{humanError(item.error_code)}</span>
                      {/if}
                    </div>
                    <div class="vi-meta">
                      {#if item.status === 'done'}
                        <span class="vi-stats">{itemStatusLabel(item)}</span>
                        {#if item.elapsed_ms}
                          <span class="vi-elapsed">{fmtElapsed(item.elapsed_ms)}</span>
                        {/if}
                        <button
                          type="button"
                          class="open-btn"
                          on:click={() => dispatch('openTranscript', { item })}
                          aria-label="Open transcript"
                        >Open ↗</button>
                      {:else if item.status === 'failed'}
                        <span class="vi-error-label">{itemStatusLabel(item)}</span>
                        <button
                          type="button"
                          class="retry-btn"
                          on:click={() => dispatch('retryFailed', { jobId: job.id, itemId: item.id })}
                        >↺ Retry</button>
                      {:else if item.status === 'cancelled'}
                        <span class="vi-cancelled">{itemStatusLabel(item)}</span>
                        <button
                          type="button"
                          class="retry-btn"
                          on:click={() => dispatch('retryFailed', { jobId: job.id, itemId: item.id })}
                          title="Re-queue this item"
                        >↺ Retry</button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</section>

<style>
  .job-history-view {
    width: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  .jh-header {
    padding: 16px 16px 12px;
    border-bottom: 1px solid var(--glass-border-soft);
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex-shrink: 0;
  }
  .jh-header-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .jh-title {
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

  .jh-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  /* Empty state */
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 32px;
    color: var(--text-3);
    text-align: center;
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

  /* Job entry */
  .job-entry {
    border-bottom: 1px solid var(--glass-border-soft);
  }
  .job-entry:last-child { border-bottom: none; }

  .job-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    background: none;
    border: none;
    font-family: inherit;
    cursor: pointer;
    text-align: left;
    position: relative;
    transition: background 0.12s;
  }
  .job-row:hover {
    background: var(--surface-2);
  }
  .job-entry.expanded .job-row {
    background: var(--surface-2);
  }

  .chevron {
    display: grid;
    place-items: center;
    color: var(--text-3);
    flex-shrink: 0;
    transition: transform 0.2s;
  }
  .chevron.rotated {
    transform: rotate(180deg);
  }

  .job-row-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .job-date {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text);
  }
  .job-stats {
    font-size: 11.5px;
    color: var(--text-3);
  }

  .model-pill {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: var(--surface-3);
    color: var(--text-2);
    padding: 2px 7px;
    border-radius: 999px;
    flex-shrink: 0;
    border: 1px solid var(--glass-border-soft);
  }

  .delete-btn {
    background: none;
    border: none;
    color: var(--text-3);
    padding: 5px;
    border-radius: 5px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.12s, color 0.12s;
    flex-shrink: 0;
  }
  .job-row:hover .delete-btn { opacity: 1; }
  .delete-btn:hover { color: var(--error); }

  /* Job detail */
  .job-detail {
    background: var(--surface-2);
    border-top: 1px solid var(--glass-border-soft);
  }

  .analytics-panel {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    font-size: 12px;
    color: var(--text-3);
    border-bottom: 1px solid var(--glass-border-soft);
    flex-wrap: wrap;
  }
  .analytics-item strong {
    color: var(--text-2);
    font-weight: 600;
  }
  .dot-sep { opacity: 0.5; }

  /* Video rows */
  .video-rows {
    display: flex;
    flex-direction: column;
  }

  .video-item {
    display: grid;
    grid-template-columns: 20px 40px 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    border-bottom: 1px solid var(--glass-border-soft);
    transition: background 0.1s;
  }
  .video-item:last-child { border-bottom: none; }
  .video-item.done:hover { background: var(--surface-3); }

  .vi-status-icon {
    font-size: 12px;
    font-weight: 600;
    text-align: center;
    flex-shrink: 0;
  }
  .icon-done { color: #4ade80; }
  .icon-failed { color: var(--error); }
  .icon-cancelled { color: var(--text-3); }

  .vi-thumb {
    position: relative;
    width: 40px;
    height: 24px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface-3);
    flex-shrink: 0;
  }
  .vi-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .vi-thumb-ph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text-3);
  }

  .vi-body {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .vi-title {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vi-error-detail {
    font-size: 11px;
    color: var(--text-3);
    line-height: 1.4;
    white-space: normal;
  }

  .vi-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    white-space: nowrap;
  }
  .vi-stats {
    font-size: 11px;
    color: var(--text-3);
  }
  .vi-elapsed {
    font-size: 11px;
    color: var(--text-3);
  }
  .vi-error-label {
    font-size: 11px;
    color: var(--error);
    font-weight: 500;
  }
  .vi-cancelled {
    font-size: 11px;
    color: var(--text-3);
  }

  .open-btn {
    background: var(--surface-3);
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
  .open-btn:hover { color: var(--text); background: var(--surface-2); }

  .retry-btn {
    background: var(--surface-3);
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
  .retry-btn:hover { color: var(--text); background: var(--surface-2); }
</style>
