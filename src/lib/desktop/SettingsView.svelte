<script>
  import { createEventDispatcher, onMount } from 'svelte';

  export let model = 'tiny';            // 'tiny' | 'base' | 'small'
  export let timestamps = true;
  export let darkMode = false;
  export let historyCount = 0;
  export let historySizeBytes = 0;
  /** @type {any} invoke function from Tauri */
  export let invokeFn = null;

  const dispatch = createEventDispatcher();

  /** Raw `Cookie:` header value pasted from DevTools' Network tab. */
  let cookiePaste = '';
  let hasSavedCookies = false;
  /** @type {'idle' | 'saving' | 'saved' | 'error'} */
  let cookieSaveState = 'idle';
  let cookieSaveError = '';

  onMount(async () => {
    if (!invokeFn) return;
    try {
      hasSavedCookies = await invokeFn('has_instagram_cookies');
    } catch (e) {
      console.warn('has_instagram_cookies failed:', e);
    }
  });

  async function saveCookies() {
    if (!invokeFn || !cookiePaste.trim()) return;
    cookieSaveState = 'saving';
    cookieSaveError = '';
    try {
      await invokeFn('save_instagram_cookies', { cookieHeader: cookiePaste.trim() });
      hasSavedCookies = true;
      cookiePaste = '';
      cookieSaveState = 'saved';
      setTimeout(() => {
        if (cookieSaveState === 'saved') cookieSaveState = 'idle';
      }, 2500);
    } catch (e) {
      cookieSaveState = 'error';
      cookieSaveError = String(e);
    }
  }

  async function clearCookies() {
    if (!invokeFn) return;
    try {
      await invokeFn('clear_instagram_cookies');
      hasSavedCookies = false;
      cookieSaveState = 'idle';
    } catch (e) {
      console.warn('clear_instagram_cookies failed:', e);
    }
  }

  const modelOptions = [
    { id: 'tiny', label: 'Tiny', size: '~75 MB', speed: '~1–2 min', notes: 'Fast, good for most content', default: true },
    { id: 'base', label: 'Base', size: '~145 MB', speed: '~2–4 min', notes: 'Better accuracy, slower' },
    { id: 'small', label: 'Small', size: '~460 MB', speed: '~5–10 min', notes: 'Best accuracy, significantly slower' },
  ];

  let confirmingClear = false;

  function selectModel(id) {
    if (id === model) return;
    model = id;
    dispatch('change', { model });
  }

  function toggleTimestamps() {
    timestamps = !timestamps;
    dispatch('change', { timestamps });
  }

  function setTheme(dark) {
    if (dark === darkMode) return;
    darkMode = dark;
    dispatch('change', { darkMode });
  }

  function requestClear() {
    confirmingClear = true;
  }

  function cancelClear() {
    confirmingClear = false;
  }

  function confirmClear() {
    confirmingClear = false;
    dispatch('clearHistory');
  }

  function fmtSize(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    return `${(kb / 1024).toFixed(1)} MB`;
  }
</script>

<section class="settings-view">
  <div class="settings-scroll">
    <div class="settings-section">
      <h3 class="section-title">Transcription</h3>
      <p class="section-desc">
        Larger models are more accurate but slower and use more disk space. The model is downloaded once on first use.
      </p>
      <div class="model-cards">
        {#each modelOptions as opt}
          <button
            type="button"
            class="model-card"
            class:active={model === opt.id}
            on:click={() => selectModel(opt.id)}
          >
            <div class="model-card-head">
              <span class="model-name">{opt.label}</span>
              {#if model === opt.id}
                <span class="model-selected-badge">Selected</span>
              {/if}
            </div>
            <div class="model-meta">
              <span>{opt.size}</span>
              <span class="dot-sep">·</span>
              <span>{opt.speed}</span>
            </div>
            <p class="model-notes">{opt.notes}</p>
          </button>
        {/each}
      </div>

      <label class="toggle-row">
        <div class="toggle" class:on={timestamps}>
          <input type="checkbox" checked={timestamps} on:change={toggleTimestamps} />
          <span class="thumb"></span>
        </div>
        <span>Include timestamps by default</span>
      </label>
    </div>

    <div class="settings-section">
      <h3 class="section-title">Appearance</h3>
      <div class="theme-buttons">
        <button type="button" class="theme-btn" class:active={!darkMode} on:click={() => setTheme(false)}>
          Light
        </button>
        <button type="button" class="theme-btn" class:active={darkMode} on:click={() => setTheme(true)}>
          Dark
        </button>
      </div>
    </div>

    <div class="settings-section">
      <h3 class="section-title">Instagram</h3>
      <p class="section-desc">
        Instagram now requires a logged-in session to fetch reels and posts, even public ones.
        Paste your session cookie below — it's the fastest way and doesn't need a browser extension.
      </p>

      <div class="ig-setup-card">
        <div class="ig-step"><span class="step-num">1</span><span>Open <strong>instagram.com</strong> in any browser while signed in.</span></div>
        <div class="ig-step"><span class="step-num">2</span><span>Open DevTools → <strong>Network</strong> tab, click any request to instagram.com.</span></div>
        <div class="ig-step"><span class="step-num">3</span><span>Under Request Headers, copy the full <strong>Cookie</strong> value and paste it below.</span></div>
      </div>

      <div class="ig-cookie-input">
        <textarea
          class="ig-cookie-textarea"
          rows="3"
          placeholder="sessionid=...; csrftoken=...; ds_user_id=...; ..."
          bind:value={cookiePaste}
          disabled={cookieSaveState === 'saving'}
        ></textarea>
        <div class="ig-cookie-actions">
          <button
            type="button"
            class="btn-primary-sm"
            on:click={saveCookies}
            disabled={!cookiePaste.trim() || cookieSaveState === 'saving'}
          >
            {cookieSaveState === 'saving' ? 'Saving…' : 'Save cookies'}
          </button>
          {#if hasSavedCookies}
            <span class="ig-cookie-status saved">✓ Cookies saved</span>
            <button type="button" class="btn-ghost" on:click={clearCookies}>Clear</button>
          {:else if cookieSaveState === 'saved'}
            <span class="ig-cookie-status saved">✓ Saved</span>
          {/if}
        </div>
        {#if cookieSaveState === 'error'}
          <p class="ig-cookie-error">Couldn't save: {cookieSaveError}</p>
        {/if}
        <p class="ig-cookie-caveat">
          We can't read an expiry from a pasted header (only the cookie's
          original <code>Set-Cookie</code> response has that), so there's no
          reliable "time remaining" to show. In practice the session stays
          valid until you sign out of Instagram in that browser or Instagram
          invalidates it server-side — if Instagram links start failing again,
          just repeat the steps above to refresh it.
        </p>
      </div>
    </div>

    <div class="settings-section">
      <h3 class="section-title">Storage</h3>
      <p class="storage-info">
        {historyCount} transcription{historyCount === 1 ? '' : 's'} · approximately {fmtSize(historySizeBytes)}
      </p>
      {#if !confirmingClear}
        <button type="button" class="btn-danger" on:click={requestClear} disabled={historyCount === 0}>
          Clear all history
        </button>
      {:else}
        <div class="confirm-row">
          <span>Are you sure? This cannot be undone.</span>
          <div class="confirm-actions">
            <button type="button" class="btn-ghost" on:click={cancelClear}>Cancel</button>
            <button type="button" class="btn-danger" on:click={confirmClear}>Clear history</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .settings-view {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }
  .settings-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 32px;
    max-width: 640px;
  }
  .settings-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .section-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }
  .section-desc {
    font-size: 12.5px;
    color: var(--text-2);
    line-height: 1.6;
    margin: 0;
  }

  .model-cards {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .model-card {
    text-align: left;
    background: var(--surface-1);
    border: 1.5px solid var(--glass-border-soft);
    border-radius: 10px;
    padding: 12px 14px;
    cursor: pointer;
    font-family: inherit;
    transition: border-color 0.15s, background 0.15s;
  }
  .model-card:hover {
    background: var(--surface-2);
  }
  .model-card.active {
    border-color: var(--accent);
    background: var(--surface-2);
  }
  .model-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .model-name {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text);
  }
  .model-selected-badge {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent-fg);
    background: var(--accent);
    padding: 2px 8px;
    border-radius: 999px;
  }
  .model-meta {
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-3);
    display: flex;
    gap: 6px;
  }
  .dot-sep { opacity: 0.6; }
  .model-notes {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--text-2);
  }

  .toggle-row {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    cursor: pointer;
    user-select: none;
    color: var(--text-2);
    font-size: 13px;
    margin-top: 4px;
  }
  .toggle {
    position: relative;
    width: 36px;
    height: 20px;
    background: var(--surface-3);
    border: 1px solid var(--glass-border-soft);
    border-radius: 22px;
    transition: background 0.25s, border-color 0.25s;
    flex-shrink: 0;
  }
  .toggle.on { background: var(--accent); border-color: var(--accent); }
  .toggle input { position: absolute; opacity: 0; width: 0; height: 0; }
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    background: var(--text-3);
    border-radius: 50%;
    transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1), background 0.25s;
  }
  .toggle.on .thumb { transform: translateX(15px); background: var(--accent-fg); }

  .theme-buttons {
    display: flex;
    gap: 8px;
  }
  .theme-btn {
    flex: 1;
    padding: 10px 16px;
    background: var(--surface-1);
    border: 1.5px solid var(--glass-border-soft);
    border-radius: 10px;
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s, color 0.15s;
  }
  .theme-btn:hover {
    background: var(--surface-2);
  }
  .theme-btn.active {
    border-color: var(--accent);
    background: var(--surface-2);
    color: var(--text);
  }

  .ig-setup-card {
    background: var(--surface-1);
    border: 1px solid var(--glass-border-soft);
    border-radius: 10px;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .ig-step {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 13px;
    color: var(--text-2);
    line-height: 1.5;
  }
  .step-num {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    background: var(--accent);
    color: var(--accent-fg);
    border-radius: 50%;
    font-size: 10px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .ig-step strong { color: var(--text); font-weight: 600; }

  .ig-cookie-input {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ig-cookie-textarea {
    width: 100%;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11.5px;
    line-height: 1.5;
    padding: 10px 12px;
    resize: vertical;
    outline: none;
  }
  .ig-cookie-textarea:focus {
    border-color: var(--text-3);
  }
  .ig-cookie-textarea:disabled {
    opacity: 0.5;
  }
  .ig-cookie-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .btn-primary-sm {
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: var(--radius-xs);
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 500;
    padding: 7px 14px;
    cursor: pointer;
    transition: background 0.2s;
  }
  .btn-primary-sm:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-primary-sm:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .ig-cookie-status {
    font-size: 12px;
    font-weight: 500;
  }
  .ig-cookie-status.saved {
    color: #4ade80;
  }
  .ig-cookie-error {
    font-size: 12px;
    color: var(--error);
    margin: 0;
  }
  .ig-cookie-caveat {
    font-size: 11.5px;
    color: var(--text-3);
    line-height: 1.5;
    margin: 2px 0 0;
  }
  .ig-cookie-caveat code {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    background: var(--surface-3);
    border-radius: 3px;
    padding: 1px 4px;
  }

  .storage-info {
    font-size: 13px;
    color: var(--text-2);
    margin: 0;
  }

  .btn-danger {
    align-self: flex-start;
    background: var(--error-bg);
    color: var(--error);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-xs);
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 500;
    padding: 8px 14px;
    cursor: pointer;
    transition: background 0.2s;
  }
  .btn-danger:hover:not(:disabled) {
    background: var(--error-border);
  }
  .btn-danger:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .confirm-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    background: var(--error-bg);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-xs);
    padding: 10px 14px;
    font-size: 12.5px;
    color: var(--error);
  }
  .confirm-actions {
    display: flex;
    gap: 8px;
  }
  .btn-ghost {
    background: var(--surface-2);
    color: var(--text-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: var(--radius-xs);
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    padding: 6px 11px;
    cursor: pointer;
  }
  .btn-ghost:hover { color: var(--text); background: var(--glass-bg-strong); }
</style>
