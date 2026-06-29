<script>
  /**
   * TranscriptPanel — right-hand pane showing the active transcript.
   * Mirrors NeoDLP's metadata/result pane: title at top, format selector,
   * transcript body, and action buttons (Copy, Save) below.
   */
  import SaveTranscriptButton from './SaveTranscriptButton.svelte';

  export let result = null;          // { language, plain, timestamped, srt } | null
  export let activeTab = 'plain';    // 'plain' | 'timestamped' | 'srt'
  export let defaultName = '';
  export let onTabChange = () => {};
  export let onCopy = () => {};
  export let streamSegments = [];   // Array<{index, text, start, end, ts, displayed}>
  export let phase = 'idle';        // 'idle' | 'downloading' | 'transcribing' | 'done'
  export let timestamps = true;

  $: isStreaming = !result && (streamSegments.length > 0 || phase === 'transcribing');

  function getActiveContent() {
    if (!result) return '';
    if (activeTab === 'plain') return result.plain;
    if (activeTab === 'timestamped') return result.timestamped ?? '';
    if (activeTab === 'srt') return result.srt;
    return '';
  }

  function getWordCount(text) {
    if (!text) return 0;
    return text.trim().split(/\s+/).filter(Boolean).length;
  }

  function getDurationEstimate(text) {
    // ~150 words per minute spoken
    const words = getWordCount(text);
    const minutes = words / 150;
    if (minutes < 1) return `${Math.round(minutes * 60)}s`;
    return `${minutes.toFixed(1)} min`;
  }

  $: content = getActiveContent();
  $: wordCount = getWordCount(content);
  $: duration = getDurationEstimate(content);
</script>

<section class="transcript-panel">
  <header class="panel-header">
    <div class="panel-title-row">
      <h3 class="panel-title">Transcript</h3>
      {#if result}
        <span class="lang-badge">{result.language.toUpperCase()}</span>
      {/if}
    </div>
    <div class="panel-header-right">
      {#if result}
        <div class="panel-stats">
          <span class="stat"><strong>{wordCount.toLocaleString()}</strong> words</span>
          <span class="stat-sep">·</span>
          <span class="stat">~<strong>{duration}</strong></span>
        </div>
        <button type="button" class="header-copy-btn" on:click={onCopy} title="Copy to clipboard">
          <svg width="12" height="12" viewBox="0 0 14 14" fill="none" aria-hidden="true">
            <rect x="4.5" y="1.5" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.4"/>
            <path d="M1.5 5.5H3M1.5 5.5V12.5H8.5V11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
          </svg>
          Copy
        </button>
      {/if}
    </div>
  </header>

  {#if result}
    <div class="format-tabs" role="tablist">
      <button role="tab" aria-selected={activeTab === 'plain'} class="format-tab" class:active={activeTab === 'plain'} on:click={() => onTabChange('plain')}>Plain</button>
      {#if result.timestamped}
        <button role="tab" aria-selected={activeTab === 'timestamped'} class="format-tab" class:active={activeTab === 'timestamped'} on:click={() => onTabChange('timestamped')}>Timestamped</button>
      {/if}
      {#if result.srt}
        <button role="tab" aria-selected={activeTab === 'srt'} class="format-tab" class:active={activeTab === 'srt'} on:click={() => onTabChange('srt')}>SRT</button>
      {/if}
    </div>

    <div class="transcript-body">
      {#if activeTab === 'srt' && result.srt}
        <pre class="transcript-text srt-text">{content}</pre>
      {:else if activeTab === 'timestamped' && result.timestamped}
        <div class="timestamped-text">
          {#each content.split('\n') as line}
            {#if line.match(/^\[\d{2}:\d{2}\]/)}
              <p class="ts-line"><span class="ts-marker">{line.slice(0, 7)}</span>{line.slice(7)}</p>
            {:else if line}
              <p class="ts-line">{line}</p>
            {/if}
          {/each}
        </div>
      {:else if result.plain}
        <div class="plain-text">
          {#each (result.plain || '').split('\n') as line}
            {#if line}
              <p class="plain-line">{line}</p>
            {/if}
          {/each}
        </div>
      {:else}
        <p class="no-data-hint">Not available for this transcription.</p>
      {/if}
    </div>

    <footer class="panel-actions">
      <SaveTranscriptButton {content} {defaultName} format={activeTab} />
    </footer>
  {:else if isStreaming}
    <div class="transcript-body">
      <div class="stream-transcript">
        {#each streamSegments as seg (seg.index)}
          <p class="stream-segment">
            {#if timestamps && seg.ts}<span class="seg-ts">[{seg.ts}]</span> {/if}{seg.displayed}{#if seg.displayed.length < seg.text.length}<span class="cursor-blink" aria-hidden="true"></span>{/if}
          </p>
        {/each}
        {#if streamSegments.length === 0}
          <span class="cursor-blink" aria-hidden="true"></span>
        {/if}
      </div>
    </div>
  {:else}
    <div class="empty-state">
      <div class="empty-icon">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>
      <h4 class="empty-title">No transcript yet</h4>
      <p class="empty-desc">Paste a video URL on the left and hit Transcribe (⌘↩).</p>
    </div>
  {/if}
</section>

<style>
  .transcript-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border-radius: 12px;
    border: 1px solid var(--glass-border-soft);
    overflow: hidden;
    min-height: 0;
  }
  .panel-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--glass-border-soft);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-shrink: 0;
  }
  .panel-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .panel-header-right {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .header-copy-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--surface-2);
    color: var(--text-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 5px;
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    padding: 5px 10px;
    cursor: pointer;
    flex-shrink: 0;
    transition: color 0.15s, background 0.15s;
  }
  .header-copy-btn:hover { color: var(--text); background: var(--surface-3); }
  .panel-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }
  .lang-badge {
    font-size: 10.5px;
    font-weight: 600;
    background: var(--accent);
    color: var(--accent-fg);
    padding: 2px 8px;
    border-radius: 999px;
  }
  .panel-stats {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-3);
  }
  .panel-stats strong {
    color: var(--text);
    font-weight: 600;
  }
  .stat-sep { opacity: 0.5; }

  .format-tabs {
    display: flex;
    gap: 2px;
    padding: 8px 12px 0;
    border-bottom: 1px solid var(--glass-border-soft);
    background: var(--surface-1);
    flex-shrink: 0;
  }
  .format-tab {
    background: transparent;
    border: none;
    padding: 8px 14px;
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-3);
    cursor: pointer;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color 0.15s, border-color 0.15s;
  }
  .format-tab:hover {
    color: var(--text-2);
  }
  .format-tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .transcript-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    min-height: 0;
  }
  /* ── Plain format ─────────────────────────────────── */
  .plain-text { display: flex; flex-direction: column; gap: 10px; }
  .plain-line {
    font-size: 13.5px;
    line-height: 1.7;
    color: var(--text);
    margin: 0;
  }
  /* ── Timestamped format ───────────────────────────── */
  .timestamped-text { display: flex; flex-direction: column; gap: 6px; }
  .ts-line {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text);
    margin: 0;
  }
  .ts-marker {
    color: var(--highlight, #9c5a2e);
    font-weight: 700;
    margin-right: 6px;
    font-size: 11px;
    letter-spacing: 0.02em;
  }
  /* ── SRT format ───────────────────────────────────── */
  .srt-text {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12px;
    line-height: 1.7;
    color: var(--text);
    white-space: pre-wrap;
    word-wrap: break-word;
    margin: 0;
  }
  .no-data-hint {
    font-size: 13px;
    color: var(--text-3);
    margin: 0;
  }

  .panel-actions {
    padding: 10px 16px;
    border-top: 1px solid var(--glass-border-soft);
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface-1);
    flex-shrink: 0;
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
  }
  .empty-icon {
    width: 56px;
    height: 56px;
    border-radius: 12px;
    background: var(--surface-2);
    display: grid;
    place-items: center;
    margin-bottom: 12px;
    color: var(--text-2);
  }
  .empty-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-2);
    margin: 0 0 4px;
  }
  .empty-desc {
    font-size: 12.5px;
    color: var(--text-3);
    margin: 0;
  }

  .stream-transcript {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stream-segment {
    font-family: 'Fraunces', 'Iowan Old Style', Georgia, serif;
    font-size: 16px;
    line-height: 1.8;
    color: var(--text);
    letter-spacing: -0.1px;
    margin: 0;
  }

  .seg-ts {
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 12px;
    font-weight: 500;
    color: var(--highlight);
    margin-right: 2px;
  }

  .cursor-blink {
    display: inline-block;
    width: 2px;
    height: 1.1em;
    background: var(--highlight);
    border-radius: 1px;
    vertical-align: text-bottom;
    animation: blink 1s step-end infinite;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0; }
  }
</style>