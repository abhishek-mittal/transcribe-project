<script>
  import { createEventDispatcher, onDestroy } from 'svelte';
  import UrlDropZone from './UrlDropZone.svelte';
  import VideoPicker from './VideoPicker.svelte';

  export let url = '';
  export let loading = false;
  export let phase = 'idle';
  export let modelProgress = null;
  export let timestamps = true;
  export let language = null;
  export let errorMessage = null;
  export let activeView = 'transcribe';
  /** @type {string} e.g. 'tiny' | 'base' | 'small' */
  export let model = 'tiny';
  /** @type {any} invoke function from Tauri */
  export let invokeFn = null;
  /** When true (playlist detected), show "Transcribe X videos" button */
  export let pickerMode = false;
  /** Number of selected videos when pickerMode is true */
  export let selectedCount = 0;

  /**
   * Set by parent when the URL already has a completed transcript in history.
   * @type {{ jobId: string, itemId: string, title: string } | null}
   */
  export let duplicateMatch = null;

  // Probe state is bound from the parent so it survives tab switches.
  /** @type {'idle' | 'probing' | 'preview' | 'error'} */
  export let probeState = 'idle';
  /** @type {any | null} */
  export let probeResult = null;
  /** @type {string | null} */
  export let probeError = null;

  /**
   * Set when the probe resolves to a list (playlist or search results).
   * Owned locally because the picker renders inside this panel; the parent
   * only needs to know `activeView === 'picker'` for layout decisions.
   * @type {{ kind: 'playlist'|'search', title?: string, query?: string, entries?: any[], uploader?: string } | null}
   */
  let listProbeResult = null;

  const dispatch = createEventDispatcher();

  /** @type {ReturnType<typeof setTimeout> | null} */
  let debounceTimer = null;

  const MODEL_LABELS = { tiny: 'Tiny', base: 'Base', small: 'Small' };

  function formatDuration(secs) {
    if (!secs) return '';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  function resetProbe() {
    probeState = 'idle';
    probeResult = null;
    probeError = null;
  }

  function cancelDebounce() {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  async function runProbe(probeUrl) {
    if (!invokeFn) return;
    probeState = 'probing';
    probeResult = null;
    probeError = null;
    try {
      const res = await invokeFn('probe_url', { url: probeUrl });
      if (res.type === 'video') {
        probeState = 'preview';
        probeResult = res;
        listProbeResult = null;
      } else if (res.type === 'playlist' || res.type === 'search') {
        probeState = 'idle';
        probeResult = null;
        // Search-results and playlists share the same flow; the picker
        // labels the source via `kind`. Empty entries surface an inline
        // error instead of opening a useless picker.
        if (Array.isArray(res.entries) && res.entries.length === 0) {
          listProbeResult = null;
          probeState = 'error';
          probeError = `No videos found for this ${res.type === 'search' ? 'search' : 'playlist'}.`;
        } else {
          listProbeResult = { ...res, kind: res.kind || res.type };
        }
        // Still notify parent so it can switch activeView to 'picker'.
        dispatch('playlist', listProbeResult ?? { kind: res.type, query: res.query, title: res.title });
      } else {
        probeState = 'error';
        probeError = res.code === 'UNSUPPORTED_PLATFORM'
          ? 'This URL is not supported.'
          : (res.message || 'Could not load URL.');
      }
    } catch (e) {
      probeState = 'error';
      probeError = 'Could not load URL.';
    }
  }

  function onUrlChange(newUrl) {
    cancelDebounce();
    if (!newUrl || !newUrl.startsWith('http')) {
      resetProbe();
      return;
    }
    debounceTimer = setTimeout(() => {
      runProbe(newUrl);
    }, 800);
  }

  let prevUrl = '';
  $: {
    if (url !== prevUrl) {
      prevUrl = url;
      listProbeResult = null;
      if (!loading) onUrlChange(url);
      if (!url) resetProbe();
    }
  }

  onDestroy(() => {
    cancelDebounce();
  });

  function handleTranscribe() {
    if (!url.trim() || loading) return;
    dispatch('transcribe');
  }

  function handleTranscribePicker() {
    dispatch('transcribePicker');
  }

  /**
   * Forward selected entries from the inline VideoPicker up to the parent.
   * The parent constructs the queue job from these entries.
   * @param {any[]} selected
   */
  function handlePickerStartJob(selected) {
    dispatch('startJob', { selected });
  }

  function handleCancel() {
    dispatch('cancel');
  }
</script>

<section class="url-panel">
  {#if activeView === 'transcribe' || activeView === 'picker'}
    {#if pickerMode && listProbeResult}
      <VideoPicker
        entries={listProbeResult.entries ?? []}
        playlistTitle={listProbeResult.kind === 'search' ? '' : (listProbeResult.title ?? '')}
        searchQuery={listProbeResult.kind === 'search' ? (listProbeResult.query ?? '') : ''}
        kind={listProbeResult.kind}
        on:selectionChange={(e) => (selectedCount = e.detail.count)}
        on:startJob={(e) => handlePickerStartJob(e.detail.selected)}
      />
    {/if}
    <div class="panel-section">
      <header class="section-header">
        <h3 class="section-title">Source</h3>
      </header>
      <UrlDropZone bind:value={url} placeholder="Paste a video URL or drop one here…" {loading} />
      {#if pickerMode && probeState !== 'error'}
        <!-- Playlist mode hint shown in picker state -->
      {:else if probeState === 'probing'}
        <p class="section-hint probing">
          <span class="spinner-inline"></span>
          Checking URL…
        </p>
      {:else if probeState === 'error'}
        <p class="section-hint error">{probeError}</p>
      {:else if !pickerMode}
        <p class="section-hint">YouTube · Instagram · TikTok · Twitter/X · 1000+ more</p>
      {/if}

      {#if duplicateMatch}
        <div class="duplicate-notice">
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="6.5" stroke="currentColor" stroke-width="1.4"/>
            <path d="M8 5v3.5M8 11h.01" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
          </svg>
          <span class="dup-text">
            {#if duplicateMatch.priorStatus === 'done'}
              Already transcribed: <strong>{duplicateMatch.title.length > 40 ? duplicateMatch.title.slice(0, 40) + '…' : duplicateMatch.title}</strong>
            {:else if duplicateMatch.priorStatus === 'failed'}
              Previous attempt failed: <strong>{duplicateMatch.title.length > 40 ? duplicateMatch.title.slice(0, 40) + '…' : duplicateMatch.title}</strong>
            {:else if duplicateMatch.priorStatus === 'cancelled'}
              Previous attempt was cancelled: <strong>{duplicateMatch.title.length > 40 ? duplicateMatch.title.slice(0, 40) + '…' : duplicateMatch.title}</strong>
            {:else}
              Already in history: <strong>{duplicateMatch.title.length > 40 ? duplicateMatch.title.slice(0, 40) + '…' : duplicateMatch.title}</strong>
            {/if}
          </span>
        </div>
      {/if}

      {#if probeState === 'preview' && probeResult}
        <div class="preview-card">
          <div class="preview-thumb-wrap">
            <img
              class="preview-thumb"
              src={probeResult.thumbnail}
              alt={probeResult.title}
              on:error={(e) => { e.currentTarget.style.display = 'none'; e.currentTarget.nextElementSibling.style.display = 'grid'; }}
            />
            <div class="preview-thumb-placeholder" style="display:none">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <rect x="2" y="4" width="20" height="16" rx="2" stroke="currentColor" stroke-width="1.4"/>
                <path d="M10 9l5 3-5 3V9z" fill="currentColor"/>
              </svg>
            </div>
          </div>
          <div class="preview-meta">
            <p class="preview-title">{probeResult.title}</p>
            <p class="preview-sub">
              {#if probeResult.uploader}<span>{probeResult.uploader}</span>{/if}
              {#if probeResult.uploader && probeResult.duration}<span class="dot-sep"> · </span>{/if}
              {#if probeResult.duration}<span>{formatDuration(probeResult.duration)}</span>{/if}
            </p>
          </div>
        </div>
      {/if}
    </div>

    <div class="panel-section">
      <header class="section-header">
        <h3 class="section-title">Options</h3>
      </header>
      <label class="toggle-row">
        <div class="toggle" class:on={timestamps}>
          <input type="checkbox" bind:checked={timestamps} disabled={loading} />
          <span class="thumb"></span>
        </div>
        <span class="toggle-label">Include timestamps</span>
      </label>
    </div>

    {#if !pickerMode}
      <div class="panel-section status-section">
        <header class="section-header">
          <h3 class="section-title">Status</h3>
        </header>
        <div class="status-row">
          {#if errorMessage}
            <span class="status-dot error"></span>
            <span class="status-text error">{errorMessage}</span>
          {:else if loading}
            <span class="status-dot pulse"></span>
            <span class="status-text">
              {#if phase === 'downloading' && modelProgress !== null}
                Downloading model · {Math.round(modelProgress * 100)}%
              {:else if phase === 'downloading'}
                Downloading audio…
              {:else if phase === 'transcribing'}
                Transcribing…
              {:else}
                Working…
              {/if}
            </span>
          {:else if language}
            <span class="status-dot ready"></span>
            <span class="status-text">Last run · <strong>{language.toUpperCase()}</strong></span>
          {:else}
            <span class="status-dot idle"></span>
            <span class="status-text">Idle</span>
          {/if}
        </div>
      </div>
    {/if}
  {:else if activeView === 'history'}
    <div class="panel-section">
      <header class="section-header">
        <h3 class="section-title">History</h3>
      </header>
      <p class="empty-hint">Recent transcriptions will appear here.</p>
    </div>
  {:else if activeView === 'settings'}
    <div class="panel-section">
      <header class="section-header">
        <h3 class="section-title">Settings</h3>
      </header>
      <p class="empty-hint">Settings coming in v1.1.</p>
    </div>
  {/if}
</section>

<footer class="action-bar">
  {#if loading}
    <button type="button" class="btn-stop" on:click={handleCancel}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
        <rect x="2" y="2" width="8" height="8" rx="1.5" fill="currentColor"/>
      </svg>
      <span>Stop</span>
      <span class="shortcut-hint"><kbd>⌘</kbd><kbd>.</kbd></span>
    </button>
  {:else if pickerMode}
    <button
      type="button"
      class="btn-primary"
      on:click={handleTranscribePicker}
      disabled={selectedCount === 0}
    >
      <span>Transcribe {selectedCount > 0 ? selectedCount : ''} video{selectedCount === 1 ? '' : 's'}</span>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M3 8L13 8M13 8L9 4M13 8L9 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span class="shortcut-hint"><kbd>⌘</kbd><kbd>↩</kbd></span>
    </button>
  {:else if duplicateMatch}
    <div class="action-col duplicate-actions">
      <button type="button" class="btn-primary" on:click={() => dispatch('viewDuplicate')}>
        View in History →
      </button>
      <button type="button" class="btn-ghost-sm" on:click={() => dispatch('forceTranscribe')}>
        Transcribe again
      </button>
    </div>
  {:else}
    <div class="action-col">
      <button
        type="button"
        class="btn-primary"
        on:click={handleTranscribe}
        disabled={!url.trim() || probeState === 'probing'}
      >
        <span>Transcribe</span>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M3 8L13 8M13 8L9 4M13 8L9 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span class="shortcut-hint"><kbd>⌘</kbd><kbd>↩</kbd></span>
      </button>
      <button
        type="button"
        class="model-badge"
        on:click={() => dispatch('goSettings')}
        title="Change model in Settings"
      >
        Model: {MODEL_LABELS[model] || model}
      </button>
    </div>
  {/if}
</footer>

<style>
  .url-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 18px 20px 0;
    gap: 22px;
    min-height: 0;
  }

  .panel-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .section-header {
    margin-bottom: 2px;
  }
  .section-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin: 0;
  }
  .section-hint {
    font-size: 11.5px;
    color: var(--text-3);
    margin: 4px 0 0;
  }
  .section-hint.probing {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .section-hint.error {
    color: var(--error);
  }
  .empty-hint {
    font-size: 12.5px;
    color: var(--text-3);
    margin: 0;
    font-style: italic;
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

  /* ── Preview card ─────────────────────────────────────── */
  .preview-card {
    border: 1px solid var(--glass-border-soft);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .preview-thumb-wrap {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    background: var(--surface-3);
    overflow: hidden;
  }
  .preview-thumb {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .preview-thumb-placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--surface-3);
    color: var(--text-3);
  }
  .preview-meta {
    padding: 8px 10px 10px;
  }
  .preview-title {
    font-size: 13.5px;
    font-weight: 500;
    color: var(--text);
    margin: 0 0 4px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .preview-sub {
    font-size: 12px;
    color: var(--text-3);
    margin: 0;
  }
  .dot-sep { opacity: 0.6; }

  /* ── Toggle ─────────────────────────────────────────────── */
  .toggle-row {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }
  .toggle {
    position: relative;
    width: 36px;
    height: 20px;
    background: var(--surface-2);
    border-radius: 999px;
    border: 1px solid var(--glass-border-soft);
    transition: background 0.2s;
  }
  .toggle.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .toggle input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
    margin: 0;
  }
  .toggle .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    background: var(--surface-1);
    border-radius: 50%;
    transition: transform 0.2s;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }
  .toggle.on .thumb {
    transform: translateX(16px);
  }
  .toggle-label {
    font-size: 13px;
    color: var(--text);
  }

  /* ── Status section ──────────────────────────────────────── */
  .status-section {
    margin-top: auto;
    padding-bottom: 18px;
  }
  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 8px;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-3);
    flex-shrink: 0;
  }
  .status-dot.idle { background: var(--text-3); }
  .status-dot.ready { background: #4ade80; box-shadow: 0 0 6px rgba(74, 222, 128, 0.6); }
  .status-dot.error { background: #ef4444; }
  .status-dot.pulse { background: var(--accent); animation: pulse 1.4s ease-in-out infinite; }
  .status-text {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .status-text.error { color: #ef4444; }
  .status-text strong { font-weight: 600; }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  /* ── Action bar ──────────────────────────────────────────── */
  .action-bar {
    padding: 14px 20px;
    border-top: 1px solid var(--glass-border-soft);
    background: var(--surface-1);
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .action-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .btn-primary {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: 8px;
    font-family: inherit;
    font-size: 14px;
    font-weight: 500;
    padding: 11px 18px;
    cursor: pointer;
    transition: background 0.2s, transform 0.15s;
    width: 100%;
  }
  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── Duplicate notice ─────────────────────────────── */
  .duplicate-notice {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.25);
    border-radius: 7px;
    padding: 9px 11px;
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-2);
    line-height: 1.45;
    flex-shrink: 0;
  }
  .duplicate-notice svg {
    color: #f59e0b;
    flex-shrink: 0;
    margin-top: 1px;
  }
  .dup-text strong {
    color: var(--text);
  }

  .duplicate-actions {
    gap: 8px;
  }

  .btn-ghost-sm {
    background: none;
    border: 1px solid var(--glass-border-soft);
    border-radius: 7px;
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-2);
    padding: 7px 14px;
    cursor: pointer;
    text-align: center;
    transition: color 0.15s, background 0.15s;
    width: 100%;
  }
  .btn-ghost-sm:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .model-badge {
    background: none;
    border: none;
    padding: 0;
    font-family: inherit;
    font-size: 11.5px;
    color: var(--text-3);
    cursor: pointer;
    text-align: center;
    transition: color 0.15s;
  }
  .model-badge:hover {
    color: var(--text-2);
    text-decoration: underline;
  }

  .btn-stop {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: transparent;
    color: var(--text);
    border: 1.5px solid #ef4444;
    border-radius: 8px;
    font-family: inherit;
    font-size: 14px;
    font-weight: 500;
    padding: 9.5px 18px;
    cursor: pointer;
    transition: background 0.2s, color 0.2s;
  }
  .btn-stop:hover {
    background: #ef4444;
    color: white;
  }
  .btn-stop svg { stroke: currentColor; }

  .shortcut-hint {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    opacity: 0.6;
    font-size: 11px;
  }
  .shortcut-hint kbd {
    display: inline-block;
    padding: 1px 5px;
    background: rgba(255, 255, 255, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 3px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 10px;
  }
  .btn-stop .shortcut-hint kbd {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.3);
  }
</style>
