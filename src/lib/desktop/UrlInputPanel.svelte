<script>
  import { createEventDispatcher, onDestroy } from 'svelte';
  import UrlDropZone from './UrlDropZone.svelte';
  import VideoPicker from './VideoPicker.svelte';
  import ProbeActivityStrip from './ProbeActivityStrip.svelte';

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
  /** @type {any} listen function from Tauri (for probe-activity events) */
  export let listenFn = null;
  /** When true (playlist detected), show "Transcribe X videos" button */
  export let pickerMode = false;
  /** Number of selected videos when pickerMode is true */
  export let selectedCount = 0;
  /**
   * URLs already present in transcription history — forwarded to
   * VideoPicker so it can grey out already-transcribed rows.
   * @type {Set<string>}
   */
  export let transcribedUrls = new Set();

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
   * @type {{ kind: 'playlist'|'search', title?: string, query?: string, entries: any[], uploader?: string, total_count?: number | null } | null}
   */
  let listProbeResult = null;

  /**
   * The picker's currently selected entries, kept in sync via its
   * `selectionChange` event. The footer "Transcribe N videos →" button
   * uses this directly instead of relying on `transcribePicker`/`transcribe`
   * (see F12 spec section C — the old footer button was a dead no-op).
   * @type {Array<any>}
   */
  let selectedPickerEntries = [];

  /** True while a "Load more" page fetch is in flight. */
  let loadingMore = false;
  /** Set when the last "Load more" fetch failed. */
  let loadMoreError = false;

  /** F14 probe activity — drives the ProbeActivityStrip under the URL input. */
  /** @type {Array<{id: number, severity: 'info'|'success'|'warn'|'error', message: string}>} */
  let probeActivity = [];
  /** Age (in seconds) of the cache entry currently being shown. null = no cache hit. */
  let cacheAge = null;
  /** Active `probe-activity` event subscription. Cleaned up on URL change / unmount. */
  let probeActivityUnlisten = null;
  /** Per-probe state — used to translate emitted events into activity messages. */
  let liveProbeSeenEntries = 0;
  let liveProbeTotalCount = null;

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
    probeActivity = [];
    cacheAge = null;
    liveProbeSeenEntries = 0;
    liveProbeTotalCount = null;
    if (probeActivityUnlisten) {
      try { probeActivityUnlisten(); } catch {}
      probeActivityUnlisten = null;
    }
  }

  function cancelDebounce() {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function pushActivity(severity, message) {
    probeActivity = [{ id: ++pushActivityId, severity, message }, ...probeActivity].slice(0, 6);
  }
  let pushActivityId = 0;

  async function runProbe(probeUrl) {
    if (!invokeFn) return;
    resetProbe();
    liveProbeSeenEntries = 0;
    liveProbeTotalCount = null;
    probeState = 'probing';
    pushActivity('info', 'Checking URL…');

    // 1. Cache check first (F14) — instant picker render on cache hit.
    let cacheChecked = false;
    try {
      const cached = await invokeFn('get_cached_probe', { url: probeUrl });
      cacheChecked = true;
      if (cached && cached.type !== 'error') {
        // Get the actual age for display. We don't currently get the age
        // back from get_cached_probe (it returns the value only), so we
        // recompute via a quick second call. To keep this simple, we just
        // show "cached" without an exact age on this path — the user
        // can hit Refresh to force a fresh probe.
        cacheAge = 0;
        if (cached.type === 'video') {
          probeState = 'preview';
          probeResult = cached;
          listProbeResult = null;
          pushActivity('success', `Loaded from cache · ${cached.title || 'video'}`);
        } else if (cached.type === 'playlist' || cached.type === 'search') {
          probeState = 'idle';
          probeResult = null;
          listProbeResult = { ...cached, kind: cached.kind || cached.type };
          pushActivity('success', `Loaded from cache · ${cached.entries?.length ?? 0} videos`);
          dispatch('playlist', listProbeResult);
        }
        return;
      }
    } catch (e) {
      // Cache miss is fine; fall through to live probe.
      cacheChecked = false;
    }

    // 2. Subscribe to probe-activity events BEFORE invoking the probe, so
    // we don't miss any early `status` heartbeats from the sidecar.
    if (listenFn) {
      try {
        probeActivityUnlisten = await listenFn('probe-activity', (/** @type {any} */ event) => {
          const payload = event.payload;
          if (!payload || typeof payload !== 'object') return;
          handleProbeActivityEvent(payload);
        });
      } catch (e) {
        // Listener setup failure shouldn't block the probe itself — the
        // picker will still get the final result.
        console.warn('probe-activity listener setup failed:', e);
      }
    }

    try {
      const res = await invokeFn('probe_url', { url: probeUrl });
      handleProbeResult(res, probeUrl);
    } catch (e) {
      probeState = 'error';
      probeError = 'Could not load URL.';
      pushActivity('error', 'Probe failed');
    } finally {
      if (probeActivityUnlisten) {
        try { probeActivityUnlisten(); } catch {}
        probeActivityUnlisten = null;
      }
    }
  }

  /**
   * Apply one `probe-activity` event from the sidecar. Called both during
   * the live probe (via the listenFn subscription) and indirectly via the
   * final `probe_url` invoke resolving. Idempotent for entries the final
   * result also contains — dedupe by `id`.
   * @param {any} payload
   */
  function handleProbeActivityEvent(payload) {
    const ev = payload.event;
    if (ev === 'status') {
      pushActivity('info', payload.message || 'working…');
      return;
    }
    if (ev === 'entry') {
      const entry = payload.entry;
      if (!entry || !entry.id) return;
      // Append to listProbeResult.entries if we're already in picker mode
      // for this URL; otherwise buffer so handleProbeResult can populate it
      // when the final result lands.
      const existing = listProbeResult?.entries ?? [];
      if (!existing.some((/** @type {any} */ e) => e.id === entry.id)) {
        const next = { ...(listProbeResult ?? {}), entries: [...existing, entry] };
        if (!listProbeResult) {
          // First entry before final result — seed the picker shell so
          // rows render live. `kind`/`title`/`total_count` are filled in
          // when the final result arrives.
          next.kind = 'playlist';
        }
        listProbeResult = next;
        liveProbeSeenEntries = next.entries.length;
        pushActivity('info', `Streaming entry ${liveProbeSeenEntries}`);
      }
      return;
    }
    if (ev === 'done') {
      // The final `done` event carries totals — promote them. We don't
      // push a generic "Probe ready" message here because the listener
      // fires AFTER the probe_url invoke resolves in Rust's stream loop
      // (the result blob is captured synchronously, the event emit is
      // async), so handleProbeResult's "N videos · ready" already landed
      // first. Pushing "Probe ready" here would overwrite it with a less
      // informative message. The total_count update still runs so the
      // picker's "20 of N videos" header is correct as soon as the
      // totals arrive.
      if (typeof payload.total_count === 'number') {
        liveProbeTotalCount = payload.total_count;
        listProbeResult = { ...(listProbeResult ?? {}), total_count: payload.total_count };
      }
      return;
    }
    if (ev === 'error') {
      // Mid-probe error (rare — usually the final result carries the
      // error). Surface it but don't replace any `done` that already won.
      if (probeState === 'probing') {
        pushActivity('error', payload.message || 'probe error');
      }
      return;
    }
  }

  /**
   * Handle the final probe_url invoke result (one-shot, after the
   * streaming events have settled). Dedupes entries against what the
   * listener already streamed — both code paths may include the same
   * entries, and the picker uses id as the keyed-each identity.
   * @param {any} res
   * @param {string} probeUrl
   */
  function handleProbeResult(res, probeUrl) {
    if (!res || typeof res !== 'object') {
      probeState = 'error';
      probeError = 'Could not load URL.';
      pushActivity('error', 'Probe failed');
      return;
    }
    if (res.type === 'video') {
      probeState = 'preview';
      probeResult = res;
      listProbeResult = null;
      pushActivity('success', res.title || 'video');
    } else if (res.type === 'playlist' || res.type === 'search') {
      probeState = 'idle';
      probeResult = null;
      // Merge: any entries already streamed via probe-activity events
      // stay in place; the final result's entries dedupe by id.
      const streamed = listProbeResult?.entries ?? [];
      const finalEntries = Array.isArray(res.entries) ? res.entries : [];
      const seen = new Set(streamed.map((/** @type {any} */ e) => e.id));
      const merged = [
        ...streamed,
        ...finalEntries.filter((/** @type {any} */ e) => e.id && !seen.has(e.id)),
      ];
      listProbeResult = {
        ...res,
        kind: res.kind || res.type,
        entries: merged,
        total_count: res.total_count ?? listProbeResult?.total_count ?? null,
      };
      pushActivity('success', `${merged.length} videos · ready`);
      // Empty entries surface an inline error instead of opening a useless picker.
      if (merged.length === 0) {
        listProbeResult = null;
        probeState = 'error';
        probeError = `No videos found for this ${res.type === 'search' ? 'search' : 'playlist'}.`;
      } else {
        // Notify parent so it can switch activeView to 'picker'.
        dispatch('playlist', listProbeResult);
      }
    } else {
      probeState = 'error';
      probeError = res.code === 'UNSUPPORTED_PLATFORM'
        ? 'This URL is not supported.'
        : (res.message || 'Could not load URL.');
      pushActivity('error', `${res.code || 'ERROR'} · ${res.message || ''}`);
    }

    // Cache successful results (F14) so re-paste is instant. Errors and
    // empty results are not cached — the user should see a fresh probe
    // next time.
    if (res.type && res.type !== 'error' && invokeFn) {
      invokeFn('cache_probe', { url: probeUrl, result: res }).catch(() => {});
    }
  }

  /** Bypass the cache and force a fresh probe. Called from the strip's Refresh button. */
  async function handleRefreshProbe() {
    if (!url || !invokeFn) return;
    try {
      await invokeFn('invalidate_probe', { url });
    } catch {}
    cacheAge = null;
    runProbe(url);
  }

  function onUrlChange(newUrl) {
    cancelDebounce();
    if (!newUrl || !newUrl.startsWith('http')) {
      resetProbe();
      return;
    }
    // Flip to 'probing' immediately (not just once runProbe actually starts)
    // so callers gating on probeState — e.g. the page-level "Transcribe"
    // handler — can't slip through during the debounce window and enqueue
    // the raw, unprobed URL as a fake single video before we know whether
    // it's actually a playlist/channel.
    probeState = 'probing';
    debounceTimer = setTimeout(() => {
      runProbe(newUrl);
    }, 800);
  }

  /**
   * Fetch the next page of playlist/channel entries and append them to the
   * existing list (never replacing what's already loaded). `page_start`/
   * `page_end` are 1-indexed, continuing right after the entries already on
   * screen.
   */
  async function handleLoadMore() {
    if (!invokeFn || !listProbeResult || loadingMore) return;
    const pageStart = listProbeResult.entries.length + 1;
    const pageEnd = pageStart + 19; // 20-entry page, matches PLAYLIST_PAGE_SIZE
    loadingMore = true;
    loadMoreError = false;
    try {
      const res = await invokeFn('probe_url_page', { url, pageStart, pageEnd });
      if (res.type === 'page' && Array.isArray(res.entries)) {
        const existing = listProbeResult.entries ?? [];
        const seen = new Set(existing.map((/** @type {any} */ e) => e.id));
        const newEntries = res.entries.filter((/** @type {any} */ e) => e.id && !seen.has(e.id));
        listProbeResult = {
          ...listProbeResult,
          entries: [...existing, ...newEntries],
          total_count: res.total_count ?? listProbeResult.total_count,
        };
      } else {
        loadMoreError = true;
      }
    } catch (e) {
      loadMoreError = true;
    } finally {
      loadingMore = false;
    }
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
    if (probeActivityUnlisten) {
      try { probeActivityUnlisten(); } catch {}
      probeActivityUnlisten = null;
    }
  });

  function handleTranscribe() {
    if (!url.trim() || loading) return;
    dispatch('transcribe');
  }

  /**
   * Called when the user clicks "Transcribe N videos →" in picker mode.
   * Uses the picker's own selection (kept in sync via `selectionChange`)
   * instead of relying on the picker's internal startJob button — this is
   * the footer action-bar button, a separate UI element from the one
   * inside VideoPicker itself. Previously this dispatched an event nobody
   * handled, so clicking it silently did nothing (F12 spec section C).
   *
   * Also invalidates the probe cache for the source URL so the next paste
   * of the same URL triggers a fresh probe (catches newly added videos).
   */
  function handleTranscribePicker() {
    if (selectedPickerEntries.length === 0) return;
    if (url && invokeFn) {
      invokeFn('invalidate_probe', { url }).catch(() => {});
    }
    handlePickerStartJob(selectedPickerEntries);
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
    <!-- F14: in picker mode, the URL input collapses to a single line
         pinned at the top so the user always sees what they pasted
         even when the picker below scrolls. The activity strip and
         source hint are dropped here (the picker is the result, no
         need for either). Below the URL is the picker, scrollable. -->
    {#if pickerMode && listProbeResult}
      <div class="picker-url-line">
        <UrlDropZone bind:value={url} placeholder="Paste a video URL or drop one here…" {loading} />
      </div>
      <div class="picker-scroll">
        <VideoPicker
          entries={listProbeResult.entries ?? []}
          playlistTitle={listProbeResult.kind === 'search' ? '' : (listProbeResult.title ?? '')}
          searchQuery={listProbeResult.kind === 'search' ? (listProbeResult.query ?? '') : ''}
          kind={listProbeResult.kind}
          totalCount={listProbeResult.total_count ?? null}
          {loadingMore}
          {loadMoreError}
          {transcribedUrls}
          on:selectionChange={(e) => { selectedCount = e.detail.count; selectedPickerEntries = e.detail.selected; }}
          on:startJob={(e) => handlePickerStartJob(e.detail.selected)}
          on:loadMore={handleLoadMore}
        />
      </div>
    {:else}
      <!-- Non-picker mode: full URL input section with activity strip
           underneath (the original F14 layout). -->
      <div class="panel-section">
        <header class="section-header">
          <h3 class="section-title">Source</h3>
        </header>
        <UrlDropZone bind:value={url} placeholder="Paste a video URL or drop one here…" {loading} />
        {#if probeState === 'error'}
          <p class="section-hint error">{probeError}</p>
        {:else if !pickerMode}
          <p class="section-hint">YouTube · Instagram · TikTok · Twitter/X · 1000+ more</p>
        {/if}
        <!-- F14: live probe feedback strip. Shows real-time status and
             stays put until the picker takes over (then disappears). -->
        {#if probeState === 'probing' || probeState === 'error' || probeActivity.length > 0}
          <ProbeActivityStrip
            messages={probeActivity}
            cacheAge={cacheAge}
            onRefresh={handleRefreshProbe}
          />
        {/if}
        {#if probeState === 'preview' && probeResult}
          <div class="preview-card">
            <div class="preview-thumb-wrap">
              {#if probeResult.thumbnail}
                <img
                  class="preview-thumb"
                  src={probeResult.thumbnail}
                  alt=""
                  on:error={(e) => {
                    console.error('[preview-thumb] failed to load:', probeResult.thumbnail);
                    e.currentTarget.style.display = 'none';
                    e.currentTarget.nextElementSibling.style.display = 'grid';
                  }}
                  on:load={() => console.log('[preview-thumb] loaded ok:', probeResult.thumbnail)}
                />
              {/if}
              <div class="preview-thumb-placeholder" style={probeResult.thumbnail ? 'display:none' : ''} aria-hidden="true">
                <svg width="22" height="22" viewBox="0 0 24 24" fill="none">
                  <rect x="2" y="4" width="20" height="16" rx="2" stroke="currentColor" stroke-width="1.4"/>
                  <path d="M10 9l5 3-5 3V9z" fill="currentColor"/>
                </svg>
              </div>
            </div>
            <div class="preview-meta">
              <p class="preview-title">{probeResult.title || 'Video'}</p>
              {#if probeResult.uploader || probeResult.duration}
                <p class="preview-sub">
                  {#if probeResult.uploader}{probeResult.uploader}{/if}
                  {#if probeResult.uploader && probeResult.duration}<span class="dot-sep"> · </span>{/if}
                  {#if probeResult.duration}{formatDuration(probeResult.duration)}{/if}
                </p>
              {/if}
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

<!-- Action bar: sticky at the bottom of the panel so the
     "Transcribe N videos →" button is always visible, no matter
     how long the picker scrolls. Sits below .url-panel which has
     min-height:0 + flex so the parent flex column pins it. -->
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
  /* The .url-panel is the scrollable region inside the left pane; the
     footer .action-bar (defined further down) is the next flex sibling
     pinned at the bottom. Picker mode replaces .url-panel's scroll
     region with a contained .picker-scroll so the picker doesn't push
     the action bar off-screen. */
  .url-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 18px 20px 0;
    gap: 22px;
    min-height: 0;
  }

  /* Picker mode: URL line + scrollable picker. The .picker-scroll is
     the only scrollable region in this layout — the parent flex
     column keeps the action bar pinned at the bottom regardless of
     how many entries the picker has. */
  .picker-url-line {
    flex-shrink: 0;
    padding-bottom: 12px;
  }
  /* Compact URL input in picker mode — drop the full Source section
     chrome (label, hint, padding). Looks like a single-line pill. */
  .picker-url-line :global(.drop-zone) {
    padding: 8px 14px;
  }
  .picker-url-line :global(.drop-zone .dropzone-content) {
    gap: 8px;
  }

  .picker-scroll {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    /* Negative margin lets the picker fill to the very bottom of the
       scroll region (the action-bar above this provides the visual
       bottom border via its own top-border). */
    margin-bottom: -1px;
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
