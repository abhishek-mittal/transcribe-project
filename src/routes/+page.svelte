<script>
  /** @type {string} */
  let url = '';
  /** @type {'small' | 'medium' | 'large-v3-turbo' | 'large-v3'} */
  let model = 'small';
  let timestamps = true;
  let loading = false;
  /** @type {{ language: string, plain: string, timestamped: string|null, srt: string } | null} */
  let result = null;
  /** @type {string | null} */
  let error = null;
  /** @type {'plain' | 'timestamped' | 'srt'} */
  let activeTab = 'plain';
  let copied = false;

  const models = [
    { value: 'small', label: 'Small', desc: 'Fast · Less accurate' },
    { value: 'medium', label: 'Medium', desc: 'Balanced' },
    { value: 'large-v3-turbo', label: 'Large v3 Turbo', desc: 'High quality · Recommended' },
    { value: 'large-v3', label: 'Large v3', desc: 'Maximum quality · Slowest' },
  ];

  async function transcribe() {
    const trimmed = url.trim();
    if (!trimmed) return;

    loading = true;
    error = null;
    result = null;

    try {
      const response = await fetch('/api/transcribe', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url: trimmed, model, timestamps }),
      });

      const data = await response.json();

      if (!response.ok) {
        error = data.error ?? 'Transcription failed. Check the URL and try again.';
      } else {
        result = data;
        activeTab = timestamps && data.timestamped ? 'timestamped' : 'plain';
      }
    } catch (/** @type {any} */ e) {
      error = e?.message ?? 'Network error. Is the backend running?';
    } finally {
      loading = false;
    }
  }

  /** @param {string} text */
  async function copyToClipboard(text) {
    await navigator.clipboard.writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }

  function getActiveContent() {
    if (!result) return '';
    if (activeTab === 'plain') return result.plain;
    if (activeTab === 'timestamped') return result.timestamped ?? '';
    if (activeTab === 'srt') return result.srt;
    return '';
  }

  /** @param {KeyboardEvent} e */
  function handleKeydown(e) {
    if (e.key === 'Enter' && !loading) transcribe();
  }
</script>

<main>
  <div class="container">
    <!-- Header -->
    <header>
      <div class="logo">
        <svg width="32" height="32" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
          <rect width="32" height="32" rx="8" fill="var(--accent)"/>
          <path d="M10 16 L16 10 L22 16 L16 22 Z" fill="white" opacity="0.9"/>
          <circle cx="16" cy="16" r="3" fill="white"/>
        </svg>
        <span class="logo-text">Transcribe</span>
      </div>
      <p class="tagline">Convert YouTube &amp; Instagram videos to text with AI</p>
    </header>

    <!-- Input Card -->
    <div class="card">
      <div class="field">
        <label for="url-input">Video URL</label>
        <div class="input-row">
          <input
            id="url-input"
            type="url"
            bind:value={url}
            on:keydown={handleKeydown}
            placeholder="https://youtube.com/watch?v=... or https://instagram.com/reel/..."
            disabled={loading}
            autocomplete="off"
            spellcheck="false"
          />
          <button class="btn-primary" on:click={transcribe} disabled={loading || !url.trim()}>
            {#if loading}
              <span class="spinner"></span>
              <span>Transcribing…</span>
            {:else}
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M3 8L13 8M13 8L9 4M13 8L9 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <span>Transcribe</span>
            {/if}
          </button>
        </div>
      </div>

      <!-- Options -->
      <div class="options">
        <div class="option-group">
          <label>Model</label>
          <div class="model-grid">
            {#each models as m}
              <button
                class="model-btn"
                class:active={model === m.value}
                on:click={() => (model = m.value)}
                disabled={loading}
              >
                <span class="model-name">{m.label}</span>
                <span class="model-desc">{m.desc}</span>
              </button>
            {/each}
          </div>
        </div>

        <div class="option-group">
          <label>Options</label>
          <label class="toggle-row">
            <div class="toggle" class:on={timestamps}>
              <input type="checkbox" bind:checked={timestamps} disabled={loading} />
              <span class="thumb"></span>
            </div>
            <span>Include timestamps</span>
          </label>
        </div>
      </div>

      {#if loading}
        <div class="status-bar">
          <span class="status-dot pulse"></span>
          <span>Downloading audio and running transcription — this may take a few minutes for longer videos…</span>
        </div>
      {/if}
    </div>

    <!-- Error -->
    {#if error}
      <div class="error-card">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
          <path d="M8 5V8.5M8 11H8.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        <span>{error}</span>
      </div>
    {/if}

    <!-- Result -->
    {#if result}
      <div class="card result-card">
        <div class="result-header">
          <div class="tabs">
            <button
              class="tab"
              class:active={activeTab === 'plain'}
              on:click={() => (activeTab = 'plain')}
            >Plain Text</button>
            {#if result.timestamped}
              <button
                class="tab"
                class:active={activeTab === 'timestamped'}
                on:click={() => (activeTab = 'timestamped')}
              >Timestamped</button>
            {/if}
            <button
              class="tab"
              class:active={activeTab === 'srt'}
              on:click={() => (activeTab = 'srt')}
            >SRT Subtitles</button>
          </div>
          <div class="result-actions">
            <span class="lang-badge">{result.language.toUpperCase()}</span>
            <button
              class="btn-ghost"
              on:click={() => copyToClipboard(getActiveContent())}
            >
              {#if copied}
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                  <path d="M2.5 7L5.5 10L11.5 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                Copied
              {:else}
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                  <rect x="4.5" y="1.5" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M1.5 5.5H3M1.5 5.5V12.5H8.5V11" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                </svg>
                Copy
              {/if}
            </button>
          </div>
        </div>

        <div class="result-body">
          <pre class="transcript">{getActiveContent()}</pre>
        </div>
      </div>
    {/if}
  </div>
</main>

<style>
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(:root) {
    --bg: #0f0f11;
    --surface: #1a1a1f;
    --surface-2: #222228;
    --border: #2e2e36;
    --border-focus: #5a5af0;
    --text: #f0f0f5;
    --text-2: #8888a0;
    --text-3: #555566;
    --accent: #5a5af0;
    --accent-hover: #6e6ef5;
    --accent-dim: rgba(90, 90, 240, 0.12);
    --error: #e05858;
    --error-bg: rgba(224, 88, 88, 0.1);
    --success: #58c88a;
    --radius: 10px;
    --radius-sm: 6px;
  }

  :global(body) {
    background: var(--bg);
    color: var(--text);
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    font-size: 14px;
    line-height: 1.6;
    min-height: 100vh;
  }

  main {
    min-height: 100vh;
    padding: 48px 16px 80px;
  }

  .container {
    max-width: 720px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  /* Header */
  header {
    text-align: center;
    padding-bottom: 8px;
  }

  .logo {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }

  .logo-text {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.5px;
  }

  .tagline {
    color: var(--text-2);
    font-size: 15px;
  }

  /* Card */
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 24px;
  }

  /* Field */
  .field label {
    display: block;
    font-size: 12px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-2);
    margin-bottom: 8px;
  }

  .input-row {
    display: flex;
    gap: 10px;
  }

  input[type="url"] {
    flex: 1;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: 14px;
    padding: 10px 14px;
    outline: none;
    transition: border-color 0.15s;
    min-width: 0;
  }

  input[type="url"]:focus {
    border-color: var(--border-focus);
  }

  input[type="url"]::placeholder {
    color: var(--text-3);
  }

  input[type="url"]:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Buttons */
  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    color: white;
    border: none;
    border-radius: var(--radius-sm);
    font-size: 14px;
    font-weight: 500;
    padding: 10px 18px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s, opacity 0.15s;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-primary:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .btn-ghost {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: none;
    color: var(--text-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 12px;
    padding: 5px 10px;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }

  .btn-ghost:hover {
    color: var(--text);
    border-color: var(--text-3);
  }

  /* Options */
  .options {
    margin-top: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .option-group > label:first-child {
    display: block;
    font-size: 12px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-2);
    margin-bottom: 10px;
  }

  .model-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  @media (min-width: 480px) {
    .model-grid {
      grid-template-columns: repeat(4, 1fr);
    }
  }

  .model-btn {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-2);
    cursor: pointer;
    padding: 10px 12px;
    text-align: left;
    transition: border-color 0.15s, color 0.15s, background 0.15s;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .model-btn:hover:not(:disabled) {
    border-color: var(--text-3);
    color: var(--text);
  }

  .model-btn.active {
    border-color: var(--accent);
    background: var(--accent-dim);
    color: var(--text);
  }

  .model-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .model-name {
    font-size: 13px;
    font-weight: 500;
  }

  .model-desc {
    font-size: 11px;
    color: var(--text-2);
  }

  .model-btn.active .model-desc {
    color: var(--accent-hover);
  }

  /* Toggle */
  .toggle-row {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    user-select: none;
    color: var(--text);
  }

  .toggle {
    position: relative;
    width: 36px;
    height: 20px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 20px;
    transition: background 0.2s, border-color 0.2s;
    flex-shrink: 0;
  }

  .toggle.on {
    background: var(--accent);
    border-color: var(--accent);
  }

  .toggle input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }

  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    background: var(--text-2);
    border-radius: 50%;
    transition: transform 0.2s, background 0.2s;
  }

  .toggle.on .thumb {
    transform: translateX(16px);
    background: white;
  }

  /* Status bar */
  .status-bar {
    margin-top: 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: 13px;
    padding: 10px 12px;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }

  .pulse {
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  /* Spinner */
  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255,255,255,0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Error */
  .error-card {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 14px 16px;
    background: var(--error-bg);
    border: 1px solid rgba(224, 88, 88, 0.25);
    border-radius: var(--radius);
    color: var(--error);
    font-size: 13px;
  }

  .error-card svg {
    flex-shrink: 0;
    margin-top: 1px;
  }

  /* Result */
  .result-card {
    padding: 0;
    overflow: hidden;
  }

  .result-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    gap: 12px;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  .tab {
    background: none;
    border: none;
    color: var(--text-2);
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    transition: color 0.15s, background 0.15s;
  }

  .tab:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .tab.active {
    color: var(--text);
    background: var(--surface-2);
  }

  .result-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .lang-badge {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--accent-hover);
    background: var(--accent-dim);
    padding: 3px 8px;
    border-radius: 20px;
  }

  .result-body {
    padding: 20px;
    max-height: 480px;
    overflow-y: auto;
  }

  .transcript {
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    font-size: 13px;
    line-height: 1.75;
    color: var(--text);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
