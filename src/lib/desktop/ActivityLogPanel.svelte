<script>
  /**
   * Live event log panel shown under the queue list.
   *
   * Receives a `entries` array of severity-tagged events and renders them as
   * a streaming console with timestamps and severity colors. Auto-scrolls
   * to the bottom until the user scrolls up; once they do, a small "Jump
   * to latest" affordance appears so they can re-attach without losing
   * their place.
   *
   * @typedef {'info'|'success'|'warn'|'error'} Severity
   * @typedef {{ id: number, ts: string, severity: Severity, message: string }} LogEntry
   */

  import { onMount } from 'svelte';

  /** @type {LogEntry[]} */
  export let entries = [];

  /** @type {{ onClear?: () => void, onCopy?: () => void }} */
  export let handlers = {};

  let scrollerEl = /** @type {HTMLDivElement | null} */ (null);
  let stickToBottom = true;
  /** Set when the user scrolls up; cleared by the Jump button. */
  let userScrolledUp = false;

  // Track which entry IDs existed on initial mount so we can skip animating them.
  let mountedEntryIds = /** @type {Set<number>} */ (new Set());
  let mounted = false;
  onMount(() => {
    mounted = true;
    mountedEntryIds = new Set(entries.map((e) => e.id));
  });

  // Reactive: when new entries arrive and we're pinned, scroll the panel.
  // Guard with `mounted` to skip the initial render, and use rAF (runs after
  // browser layout) instead of queueMicrotask (runs before layout) so that
  // scrollHeight access doesn't force a synchronous layout in WKWebView.
  $: if (mounted && entries.length > 0 && stickToBottom && scrollerEl) {
    requestAnimationFrame(() => {
      if (scrollerEl) {
        scrollerEl.scrollTop = scrollerEl.scrollHeight;
      }
    });
  }

  function handleScroll() {
    if (!scrollerEl) return;
    const distanceFromBottom =
      scrollerEl.scrollHeight - scrollerEl.scrollTop - scrollerEl.clientHeight;
    const atBottom = distanceFromBottom < 24;
    stickToBottom = atBottom;
    userScrolledUp = !atBottom;
  }

  function jumpToLatest() {
    if (!scrollerEl) return;
    scrollerEl.scrollTo({
      top: scrollerEl.scrollHeight,
      behavior: 'smooth',
    });
    stickToBottom = true;
    userScrolledUp = false;
  }

  /** @param {Severity} severity */
  function severityColor(severity) {
    if (severity === 'success') return 'var(--log-success, #4ade80)';
    if (severity === 'error') return 'var(--error)';
    if (severity === 'warn') return 'var(--highlight, #9c5a2e)';
    return 'var(--log-info, var(--highlight, #9c5a2e))';
  }

  /** @param {Severity} severity */
  function severityLabel(severity) {
    if (severity === 'success') return '✓';
    if (severity === 'error') return '✗';
    if (severity === 'warn') return '⚠';
    return '·';
  }
</script>

<section class="activity-log" aria-label="Activity log">
  <header class="al-header">
    <div class="al-title-group">
      <span class="al-title">Activity</span>
      <span class="al-count">{entries.length} event{entries.length === 1 ? '' : 's'}</span>
    </div>
    <div class="al-actions">
      <button
        type="button"
        class="al-btn"
        on:click={handlers.onCopy}
        disabled={entries.length === 0}
        title="Copy log to clipboard"
      >Copy</button>
      <button
        type="button"
        class="al-btn"
        on:click={handlers.onClear}
        disabled={entries.length === 0}
        title="Clear log"
      >Clear</button>
    </div>
  </header>

  <div class="al-scroller" bind:this={scrollerEl} on:scroll={handleScroll}>
    {#if entries.length === 0}
      <div class="al-empty">
        <span class="al-empty-title">No activity yet.</span>
        <span class="al-empty-desc">Run a transcription to see live progress here.</span>
      </div>
    {:else}
      <ol class="al-list" role="log">
        {#each entries as entry (entry.id)}
          <li
            class="al-row"
            class:al-row-animated={mounted && !mountedEntryIds.has(entry.id)}
            class:al-row-success={entry.severity === 'success'}
            class:al-row-error={entry.severity === 'error'}
            class:al-row-warn={entry.severity === 'warn'}
          >
            <span class="al-ts">{entry.ts}</span>
            <span
              class="al-sev"
              style:color={severityColor(entry.severity)}
              aria-hidden="true"
            >{severityLabel(entry.severity)}</span>
            <span class="al-msg">{entry.message}</span>
          </li>
        {/each}
      </ol>
    {/if}

    {#if userScrolledUp && !stickToBottom}
      <button
        type="button"
        class="al-jump"
        on:click={jumpToLatest}
        aria-label="Jump to latest event"
      >
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M3 6L8 11L13 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Jump to latest
      </button>
    {/if}
  </div>
</section>

<style>
  .activity-log {
    margin-top: 12px;
    border-top: 1px solid var(--glass-border-soft);
    padding-top: 12px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .al-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 8px;
    flex-shrink: 0;
  }

  .al-title-group {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .al-title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .al-count {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-3);
  }

  .al-actions {
    display: flex;
    gap: 4px;
  }

  .al-btn {
    background: transparent;
    border: 1px solid var(--glass-border-soft);
    border-radius: 4px;
    color: var(--text-2);
    font-family: inherit;
    font-size: 10.5px;
    font-weight: 500;
    padding: 2px 8px;
    cursor: pointer;
    transition: color 0.12s, background 0.12s, border-color 0.12s;
  }
  .al-btn:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
    border-color: var(--glass-border);
  }
  .al-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .al-scroller {
    position: relative;
    max-height: 220px;
    overflow-y: auto;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 8px;
    /* Mono fallback so column widths stay aligned across OSes. */
    font-family: ui-monospace, 'SF Mono', SFMono-Regular, 'Menlo', 'Consolas', monospace;
    font-size: 11.5px;
    line-height: 1.55;
  }

  .al-list {
    list-style: none;
    margin: 0;
    padding: 4px 0;
  }

  .al-row {
    display: grid;
    grid-template-columns: 64px 14px 1fr;
    column-gap: 8px;
    align-items: baseline;
    padding: 2px 12px;
    color: var(--text-2);
    border-left: 2px solid transparent;
  }
  /* Only animate rows that arrive after the initial mount batch. */
  .al-row.al-row-animated {
    animation: al-row-in 180ms ease-out both;
  }
  /* Severity stripes — solid 2px left edge in the severity color, but only
     for non-default severities to keep the panel calm by default. */
  .al-row-success { border-left-color: var(--log-success, #4ade80); }
  .al-row-error   { border-left-color: var(--error); background: rgba(181, 64, 64, 0.05); }
  .al-row-warn    { border-left-color: var(--highlight, #9c5a2e); }

  @keyframes al-row-in {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .al-row.al-row-animated { animation: none; }
    .al-jump { transition: none; }
  }

  .al-ts {
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    font-size: 10.5px;
  }

  .al-sev {
    font-weight: 700;
    text-align: center;
    font-size: 10.5px;
    line-height: 1;
  }

  .al-msg {
    word-break: break-word;
    /* Keep message lines easy to scan without consuming too much width. */
  }

  .al-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 28px 12px;
    color: var(--text-3);
  }

  .al-empty-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    /* The empty state lives outside the monospace block. */
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  }

  .al-empty-desc {
    font-size: 11.5px;
    /* Same — keep the empty state aligned with the rest of the UI. */
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  }

  .al-jump {
    position: sticky;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--surface-1);
    border: 1px solid var(--glass-border);
    border-radius: 999px;
    color: var(--text);
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
    font-size: 10.5px;
    font-weight: 500;
    padding: 4px 10px;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(60, 45, 30, 0.1);
    transition: background 0.12s, transform 0.12s;
  }
  .al-jump:hover {
    background: var(--surface-3);
  }
</style>
