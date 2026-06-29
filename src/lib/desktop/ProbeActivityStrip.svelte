<script>
  /**
   * F14 — visible probe feedback.
   *
   * Shows a compact (10–12px tall) status line under the URL input so the
   * user can see what the probe sidecar is doing — replaces the previous
   * "Checking URL…" spinner which gave no indication of progress.
   *
   * Drives off a small reactive `messages` array (latest first); the most
   * recent message is rendered at full opacity, older ones fade out. Auto-
   * collapses to a single low-key "Ready · X videos" line once the probe
   * finishes.
   *
   * @typedef {{ id: number, severity: 'info'|'success'|'warn'|'error', message: string }} ProbeMsg
   */

  /** @type {ProbeMsg[]} */
  export let messages = [];
  /** Show a "Refresh" link next to a cache hit. */
  export let cacheAge = null;
  /** Fires when the user clicks "Refresh" — bypasses the cache. */
  export let onRefresh = () => {};

  let idSeq = 0;
  /** @param {Omit<ProbeMsg, 'id'>} m */
  export function push(m) {
    const entry = { id: ++idSeq, ...m };
    messages = [entry, ...messages].slice(0, 6);
  }

  $: latest = messages[0] ?? null;
  $: stale = messages.slice(1);

  /** @param {ProbeMsg['severity']} s */
  function color(s) {
    if (s === 'error') return 'var(--error)';
    if (s === 'warn') return 'var(--highlight)';
    if (s === 'success') return 'var(--log-success, #4ade80)';
    return 'var(--text-3)';
  }

  /** @param {ProbeMsg['severity']} s */
  function glyph(s) {
    if (s === 'error') return '✗';
    if (s === 'warn') return '⚠';
    if (s === 'success') return '✓';
    return '·';
  }

  /** @param {number} ageSecs */
  function formatAge(ageSecs) {
    if (ageSecs == null) return '';
    if (ageSecs < 60) return 'just now';
    const m = Math.floor(ageSecs / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    return `${Math.floor(h / 24)}d ago`;
  }
</script>

{#if latest}
  <div class="probe-activity" role="status" aria-live="polite">
    <span class="probe-glyph" style:color={color(latest.severity)} aria-hidden="true">{glyph(latest.severity)}</span>
    <span class="probe-msg">{latest.message}</span>
    {#if cacheAge != null && latest.severity === 'success'}
      <span class="probe-cache-age">· cached {formatAge(cacheAge)}</span>
      <button type="button" class="probe-refresh" on:click={onRefresh}>Refresh</button>
    {/if}
    {#if stale.length > 0 && latest.severity !== 'success'}
      <span class="probe-stale">+{stale.length} earlier</span>
    {/if}
  </div>
{/if}

<style>
  .probe-activity {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 10.5px;
    line-height: 1.4;
    padding: 4px 0 2px;
    color: var(--text-3);
    min-height: 18px;
  }
  .probe-glyph {
    flex-shrink: 0;
    font-weight: 700;
    font-size: 11px;
    width: 10px;
    text-align: center;
  }
  .probe-msg {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .probe-cache-age {
    color: var(--text-3);
    opacity: 0.8;
    flex-shrink: 0;
  }
  .probe-refresh {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--accent);
    font-family: inherit;
    font-size: inherit;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    flex-shrink: 0;
  }
  .probe-refresh:hover {
    opacity: 0.7;
  }
  .probe-stale {
    opacity: 0.5;
    font-size: 10px;
    flex-shrink: 0;
  }
</style>