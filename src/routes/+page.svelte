<script>
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';

  let url = '';
  // Fixed to 'base' — it's bundled with the function so cold starts skip
  // the HF download. See api/transcribe.py for the rationale.
  const model = 'tiny';
  let timestamps = true;
  let loading = false;
  /** @type {{ language: string, plain: string, timestamped: string|null, srt: string } | null} */
  let result = null;
  /** @type {string | null} */
  let error = null;
  /** @type {'plain' | 'timestamped' | 'srt'} */
  let activeTab = 'plain';
  let copied = false;
  let darkMode = false;
  /** @type {number | null} */
  let openFaq = null;

  /** @type {Array<{index: number, text: string, start: number, end: number, ts?: string}>} */
  let streamingSegments = [];
  /** @type {'idle' | 'downloading' | 'transcribing' | 'done'} */
  let phase = 'idle';

  const steps = [
    {
      number: '01',
      title: 'Copy a video URL',
      desc: 'Grab the link from YouTube, Instagram Reels, TikTok, or any of the 1000+ supported platforms.',
    },
    {
      number: '02',
      title: 'Paste & hit Transcribe',
      desc: 'Drop the URL in the box above. The audio is downloaded and processed for you.',
    },
    {
      number: '03',
      title: 'Copy or export',
      desc: 'Get plain text, a timestamped transcript, or an SRT subtitle file — one click to copy.',
    },
  ];

  const faqs = [
    { q: 'Which platforms are supported?', a: 'YouTube, Instagram Reels, TikTok, Twitter/X, Vimeo, and 1000+ more sites.' },
    { q: 'How long does transcription take?', a: 'A 5-minute video typically takes 1–3 minutes depending on server load. Longer videos take proportionally more time.' },
    { q: 'What languages are supported?', a: '99+ languages are supported, and the spoken language is auto-detected. The detected language is shown in the result.' },
    { q: 'What is the difference between the output formats?', a: 'Plain Text is clean readable text. Timestamped adds [MM:SS] markers before each segment. SRT is the standard subtitle format compatible with most video players and editors.' },
    { q: 'Is my video data stored or shared?', a: 'No. Audio is downloaded into a temporary directory, transcribed in memory, and deleted immediately. Nothing is stored or logged.' },
    { q: 'Why can’t I change the engine?', a: 'We pick a balance of speed and accuracy for you, so you get great results without any configuration.' },
  ];

  async function transcribe() {
    const trimmed = url.trim();
    if (!trimmed) return;

    loading = true;
    error = null;
    result = null;
    streamingSegments = [];
    phase = 'idle';

    try {
      const response = await fetch('/api/transcribe/stream', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url: trimmed, model, timestamps }),
      });

      if (!response.ok || !response.body) {
        const raw = await response.text();
        let msg = `Transcription failed (HTTP ${response.status})`;
        try {
          const d = JSON.parse(raw);
          if (d?.error) msg = d.error;
        } catch { /* ignore */ }
        if (response.status === 404) {
          msg = 'API endpoint not found (404). For local dev run `npm run dev:all`.';
        } else if (response.status === 504 || response.status === 408) {
          msg = 'The transcription timed out. Try a shorter video.';
        }
        error = msg;
        return;
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        // SSE blocks are separated by double newlines
        const events = buffer.split('\n\n');
        buffer = events.pop() ?? '';

        for (const block of events) {
          if (!block.trim()) continue;
          let eventType = 'message';
          let dataStr = '';
          for (const line of block.split('\n')) {
            if (line.startsWith('event: ')) eventType = line.slice(7).trim();
            else if (line.startsWith('data: ')) dataStr = line.slice(6);
          }
          if (!dataStr) continue;

          /** @type {any} */
          let payload;
          try { payload = JSON.parse(dataStr); } catch { continue; }

          if (eventType === 'status') {
            phase = payload.phase;
          } else if (eventType === 'segment') {
            streamingSegments = [...streamingSegments, payload];
          } else if (eventType === 'done') {
            phase = 'done';
            result = assembleResult(streamingSegments, payload.language);
            activeTab = timestamps && result.timestamped ? 'timestamped' : 'plain';
          } else if (eventType === 'error') {
            error = payload.error ?? 'Transcription failed.';
          }
        }
      }
    } catch (/** @type {any} */ e) {
      error = e?.message ?? 'Network error. Is the backend running?';
    } finally {
      loading = false;
    }
  }

  /**
   * @param {Array<{index: number, text: string, start: number, end: number, ts?: string}>} segments
   * @param {string} language
   */
  function assembleResult(segments, language) {
    const plain = segments.map(s => s.text).join('\n');
    const timestamped = timestamps
      ? segments.map(s => `[${s.ts ?? fmtTimestamp(s.start)}] ${s.text}`).join('\n')
      : null;
    const srt = segments
      .map((s, i) => `${i + 1}\n${fmtSrt(s.start)} --> ${fmtSrt(s.end)}\n${s.text}`)
      .join('\n\n');
    return { language, plain, timestamped, srt };
  }

  /** @param {number} seconds */
  function fmtTimestamp(seconds) {
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }

  /** @param {number} seconds */
  function fmtSrt(seconds) {
    const ms = Math.round(seconds * 1000);
    const h = Math.floor(ms / 3_600_000);
    const m = Math.floor((ms % 3_600_000) / 60_000);
    const s = Math.floor((ms % 60_000) / 1000);
    const r = ms % 1000;
    const p = (/** @type {number} */ n, /** @type {number} */ w) => String(n).padStart(w, '0');
    return `${p(h, 2)}:${p(m, 2)}:${p(s, 2)},${p(r, 3)}`;
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

  /** @param {number} i */
  function toggleFaq(i) {
    openFaq = openFaq === i ? null : i;
  }
</script>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link href="https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,400;0,9..144,500;1,9..144,400&family=Inter:wght@400;450;500;600;700&display=swap" rel="stylesheet" />
</svelte:head>

<div class="app" class:dark={darkMode}>

  <!-- Decorative background orbs -->
  <div class="bg-orbs" aria-hidden="true">
    <span class="orb orb-1"></span>
    <span class="orb orb-2"></span>
    <span class="orb orb-3"></span>
  </div>

  <!-- Nav -->
  <nav class="glass nav">
    <div class="nav-inner">
      <div class="logo">
        <span class="logo-mark">
          <svg width="22" height="22" viewBox="0 0 22 22" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M6 9V13M9 7V15M12 5V17M15 8V14M18 10V12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
        </span>
        <span class="logo-text">Transcribe</span>
      </div>
      <button class="glass theme-toggle" on:click={() => (darkMode = !darkMode)} aria-label="Toggle theme">
        {#if darkMode}
          <svg width="16" height="16" viewBox="0 0 18 18" fill="none">
            <circle cx="9" cy="9" r="3.5" stroke="currentColor" stroke-width="1.5"/>
            <path d="M9 1.5V3M9 15V16.5M1.5 9H3M15 9H16.5M3.7 3.7L4.75 4.75M13.25 13.25L14.3 14.3M14.3 3.7L13.25 4.75M4.75 13.25L3.7 14.3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 18 18" fill="none">
            <path d="M15.5 10.5A7 7 0 017.5 2.5a7 7 0 000 13 7 7 0 008-5z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        {/if}
      </button>
    </div>
  </nav>

  <main>
    <div class="container">

      <!-- Hero -->
      <header>
        <h1>
          Turn any video into <span class="italic">text</span>,<br/>
          <span class="serif-italic">beautifully.</span>
        </h1>
        <p class="tagline">Paste a YouTube, Instagram, or TikTok URL and get a polished transcript — with timestamps and SRT subtitles — in seconds.</p>
      </header>

      <!-- Transcribe Card -->
      <div class="glass card transcribe-card">
        <div class="field">
          <label for="url-input">Video URL</label>
          <div class="input-row">
            <div class="input-wrapper">
              <svg class="input-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M6.5 1.5h-3A2 2 0 001.5 3.5v9A2 2 0 003.5 14.5h9A2 2 0 0014.5 12.5v-3M14.5 1.5h-4m4 0v4m0-4L8 8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <input
                id="url-input"
                type="url"
                bind:value={url}
                on:keydown={handleKeydown}
                placeholder="e.g. https://youtube.com/watch?v=dQw4w9WgXcQ"
                disabled={loading}
                autocomplete="off"
                spellcheck="false"
              />
            </div>
            <button class="btn-primary" on:click={transcribe} disabled={loading || !url.trim()}>
              {#if loading}
                <span class="spinner"></span>
                <span>Transcribing…</span>
              {:else}
                <span>Transcribe</span>
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path d="M3 8L13 8M13 8L9 4M13 8L9 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              {/if}
            </button>
          </div>
        </div>

        <div class="options-row">
          <label class="toggle-row">
            <div class="toggle" class:on={timestamps}>
              <input type="checkbox" bind:checked={timestamps} disabled={loading} />
              <span class="thumb"></span>
            </div>
            <span>Include timestamps</span>
          </label>
        </div>

        {#if loading}
          <div class="status-bar">
            <span class="status-dot pulse"></span>
            <span>
              {#if phase === 'downloading'}
                Downloading audio…
              {:else if phase === 'transcribing'}
                Transcribing{streamingSegments.length > 0 ? ` — ${streamingSegments.length} segment${streamingSegments.length === 1 ? '' : 's'} so far…` : '…'}
              {:else}
                Preparing — this may take a moment…
              {/if}
            </span>
          </div>
        {/if}
      </div>

      <!-- Error -->
      {#if error}
        <div class="glass error-card">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
            <path d="M8 5V8.5M8 11H8.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
          <span>{error}</span>
        </div>
      {/if}

      <!-- Result (streaming) -->
      {#if streamingSegments.length > 0 || phase === 'done'}
        <div class="glass card result-card">
          <div class="result-header">
            <div class="tabs">
              {#if phase === 'done'}
                <button class="tab" class:active={activeTab === 'plain'} on:click={() => (activeTab = 'plain')}>Plain</button>
                {#if result?.timestamped}
                  <button class="tab" class:active={activeTab === 'timestamped'} on:click={() => (activeTab = 'timestamped')}>Timestamped</button>
                {/if}
                <button class="tab" class:active={activeTab === 'srt'} on:click={() => (activeTab = 'srt')}>SRT</button>
              {:else}
                <span class="live-badge">
                  <span class="live-dot pulse"></span>
                  Live
                </span>
              {/if}
            </div>
            <div class="result-actions">
              {#if phase === 'done' && result}
                <span class="lang-badge">{result.language.toUpperCase()}</span>
                <button class="btn-ghost" on:click={() => copyToClipboard(getActiveContent())}>
                  {#if copied}
                    <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M2.5 7L5.5 10L11.5 4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
                    Copied
                  {:else}
                    <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><rect x="4.5" y="1.5" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.4"/><path d="M1.5 5.5H3M1.5 5.5V12.5H8.5V11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>
                    Copy
                  {/if}
                </button>
              {/if}
            </div>
          </div>
          <div class="result-body">
            {#if phase === 'done' && result && activeTab !== 'plain'}
              <pre class="transcript">{getActiveContent()}</pre>
            {:else if streamingSegments.length === 0}
              <p class="no-speech">No speech detected in this audio.</p>
            {:else}
              <div class="stream-transcript">
                {#each streamingSegments as seg (seg.index)}
                  <p class="stream-segment" in:fly={{ y: 10, duration: 320, easing: cubicOut }}>
                    {#if timestamps && seg.ts}<span class="seg-ts">[{seg.ts}]</span> {/if}{seg.text}
                  </p>
                {/each}
                {#if loading && phase === 'transcribing'}
                  <span class="cursor-blink" aria-hidden="true"></span>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- How it works -->
      <section class="section">
        <div class="section-header">
          <span class="eyebrow">Workflow</span>
          <h2 class="section-title">How it <span class="serif-italic">works</span></h2>
        </div>
        <div class="steps-grid">
          {#each steps as step}
            <div class="glass step-card">
              <div class="step-number">{step.number}</div>
              <h3 class="step-title">{step.title}</h3>
              <p class="step-desc">{step.desc}</p>
            </div>
          {/each}
        </div>
      </section>

      <!-- FAQ -->
      <section class="section">
        <div class="section-header">
          <span class="eyebrow">FAQ</span>
          <h2 class="section-title">Frequently <span class="serif-italic">asked</span></h2>
        </div>
        <div class="faq-list">
          {#each faqs as faq, i}
            <div class="glass faq-item" class:open={openFaq === i}>
              <button class="faq-trigger" on:click={() => toggleFaq(i)}>
                <span>{faq.q}</span>
                <svg class="faq-chevron" class:rotated={openFaq === i} width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path d="M4 6L8 10L12 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>
              {#if openFaq === i}
                <div class="faq-body">{faq.a}</div>
              {/if}
            </div>
          {/each}
        </div>
      </section>

    </div>
  </main>

  <!-- Footer -->
  <footer>
    <div class="footer-inner">
      <div class="footer-brand">
        <span class="logo-mark small">
          <svg width="14" height="14" viewBox="0 0 22 22" fill="none">
            <path d="M6 9V13M9 7V15M12 5V17M15 8V14M18 10V12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
        </span>
        <span>Transcribe</span>
      </div>
      <p class="footer-copy">© {new Date().getFullYear()} Transcribe. All rights reserved.</p>
    </div>
  </footer>

</div>

<style>
  :global(*) { box-sizing: border-box; margin: 0; padding: 0; }

  /* ─── Light theme (default) — warm reader palette ── */
  :global(:root) {
    --bg-base: #faf6f0;
    --bg-grad-1: #f5d9be;
    --bg-grad-2: #d8d3ef;
    --bg-grad-3: #c9e4dc;

    --glass-bg: rgba(255, 253, 250, 0.6);
    --glass-bg-strong: rgba(255, 253, 250, 0.82);
    --glass-border: rgba(255, 255, 255, 0.75);
    --glass-border-soft: rgba(60, 45, 30, 0.08);
    --glass-shadow: 0 8px 28px rgba(60, 45, 30, 0.07), inset 0 1px 0 rgba(255,255,255,0.7);
    --glass-shadow-lg: 0 14px 44px rgba(60, 45, 30, 0.1), inset 0 1px 0 rgba(255,255,255,0.8);

    --surface-2: rgba(255, 253, 250, 0.55);
    --surface-3: rgba(60, 45, 30, 0.05);

    --text: #2a2520;
    --text-2: #6a6258;
    --text-3: #a39b8f;

    --accent: #2a2520;
    --accent-hover: #3d362e;
    --accent-fg: #faf6f0;

    --highlight: #9c5a2e;

    --error: #b54040;
    --error-bg: rgba(181, 64, 64, 0.07);
    --error-border: rgba(181, 64, 64, 0.2);

    --radius: 20px;
    --radius-sm: 12px;
    --radius-xs: 8px;
  }

  /* ─── Dark theme ──────────────────────────────────── */
  :global(.app.dark) {
    --bg-base: #14110e;
    --bg-grad-1: #3d2818;
    --bg-grad-2: #1f2647;
    --bg-grad-3: #0f3338;

    --glass-bg: rgba(40, 35, 30, 0.5);
    --glass-bg-strong: rgba(50, 44, 38, 0.72);
    --glass-border: rgba(255, 250, 240, 0.08);
    --glass-border-soft: rgba(255, 250, 240, 0.06);
    --glass-shadow: 0 8px 32px rgba(0, 0, 0, 0.45), inset 0 1px 0 rgba(255,250,240,0.06);
    --glass-shadow-lg: 0 14px 44px rgba(0, 0, 0, 0.55), inset 0 1px 0 rgba(255,250,240,0.08);

    --surface-2: rgba(255, 250, 240, 0.05);
    --surface-3: rgba(255, 250, 240, 0.07);

    --text: #f4ede2;
    --text-2: #b8ad9d;
    --text-3: #7a7164;

    --accent: #f4ede2;
    --accent-hover: #ffffff;
    --accent-fg: #14110e;

    --highlight: #e0a878;

    --error: #f4a5a5;
    --error-bg: rgba(244, 165, 165, 0.08);
    --error-border: rgba(244, 165, 165, 0.18);
  }

  :global(body) {
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    font-feature-settings: 'cv11', 'ss01', 'ss03';
    font-size: 15px;
    line-height: 1.65;
    min-height: 100vh;
    background: var(--bg-base);
    color: var(--text);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;
  }

  /* ─── App shell ───────────────────────────────────── */
  .app {
    position: relative;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-base);
    color: var(--text);
    overflow-x: hidden;
    transition: background 0.4s ease, color 0.4s ease;
  }

  /* ─── Background orbs (glass backdrop) ────────────── */
  .bg-orbs {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
  }

  .orb {
    position: absolute;
    border-radius: 50%;
    filter: blur(90px);
    opacity: 0.6;
    transition: background 0.6s ease, opacity 0.6s ease;
  }

  .orb-1 {
    width: 520px; height: 520px;
    top: -140px; left: -140px;
    background: var(--bg-grad-1);
  }
  .orb-2 {
    width: 560px; height: 560px;
    top: 28%; right: -180px;
    background: var(--bg-grad-2);
  }
  .orb-3 {
    width: 460px; height: 460px;
    bottom: -140px; left: 22%;
    background: var(--bg-grad-3);
  }

  .app.dark .orb { opacity: 0.45; }

  /* ─── Glass primitive ─────────────────────────────── */
  .glass {
    background: var(--glass-bg);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(22px) saturate(180%);
    -webkit-backdrop-filter: blur(22px) saturate(180%);
    box-shadow: var(--glass-shadow);
  }

  /* ─── Nav ─────────────────────────────────────────── */
  nav.nav {
    position: sticky;
    top: 14px;
    z-index: 50;
    border-radius: 999px;
    max-width: 760px;
    width: calc(100% - 32px);
    margin: 14px auto 0;
  }

  .nav-inner {
    padding: 0 8px 0 18px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .logo-mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px; height: 28px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--accent-fg);
  }
  .logo-mark.small { width: 22px; height: 22px; border-radius: 6px; }

  .logo-text {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.2px;
    color: var(--text);
  }

  .theme-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    color: var(--text-2);
    cursor: pointer;
    transition: color 0.2s, transform 0.3s ease;
  }
  .theme-toggle:hover { color: var(--text); transform: rotate(20deg); }

  /* ─── Main ────────────────────────────────────────── */
  main {
    position: relative;
    z-index: 1;
    flex: 1;
    padding: 72px 20px 80px;
  }

  .container {
    max-width: 720px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 40px;
  }

  /* ─── Hero ────────────────────────────────────────── */
  header {
    text-align: center;
    padding-bottom: 4px;
  }

  h1 {
    font-family: 'Fraunces', 'Cormorant Garamond', Georgia, serif;
    font-size: clamp(42px, 7vw, 64px);
    font-weight: 400;
    letter-spacing: -1.8px;
    line-height: 1.05;
    color: var(--text);
    margin-bottom: 20px;
    font-feature-settings: 'ss01';
  }

  .italic {
    font-style: italic;
    color: var(--highlight);
  }

  .serif-italic {
    font-style: italic;
    color: var(--text-2);
  }

  .tagline {
    font-size: 17px;
    color: var(--text-2);
    max-width: 540px;
    margin: 0 auto;
    line-height: 1.6;
  }

  /* ─── Card ────────────────────────────────────────── */
  .card {
    border-radius: var(--radius);
  }

  .transcribe-card {
    padding: 24px;
    box-shadow: var(--glass-shadow-lg);
  }

  /* ─── Field ───────────────────────────────────────── */
  .field label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-3);
    margin-bottom: 10px;
  }

  .input-row {
    display: flex;
    gap: 10px;
  }

  .input-wrapper {
    position: relative;
    flex: 1;
    min-width: 0;
  }

  .input-icon {
    position: absolute;
    left: 14px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-3);
    pointer-events: none;
  }

  input[type="url"] {
    width: 100%;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-family: inherit;
    font-size: 15px;
    padding: 13px 14px 13px 40px;
    outline: none;
    transition: border-color 0.2s, background 0.2s, box-shadow 0.2s;
  }

  input[type="url"]:focus {
    border-color: var(--text-3);
    background: var(--glass-bg-strong);
    box-shadow: 0 0 0 4px rgba(28, 28, 50, 0.05);
  }

  input[type="url"]::placeholder { color: var(--text-3); }
  input[type="url"]:disabled { opacity: 0.5; cursor: not-allowed; }

  /* ─── Buttons ─────────────────────────────────────── */
  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: var(--radius-sm);
    font-family: inherit;
    font-size: 15px;
    font-weight: 500;
    padding: 13px 22px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.2s, transform 0.15s, box-shadow 0.2s;
    box-shadow: 0 4px 14px rgba(42, 37, 32, 0.2);
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
    transform: translateY(-1px);
    box-shadow: 0 8px 20px rgba(42, 37, 32, 0.28);
  }
  .btn-primary:active:not(:disabled) { transform: translateY(0); }
  .btn-primary:disabled { opacity: 0.4; cursor: not-allowed; box-shadow: none; }

  .btn-ghost {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--surface-2);
    color: var(--text-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: var(--radius-xs);
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    padding: 6px 11px;
    cursor: pointer;
    transition: color 0.2s, background 0.2s, border-color 0.2s;
  }
  .btn-ghost:hover { color: var(--text); background: var(--glass-bg-strong); }

  /* ─── Options row ─────────────────────────────────── */
  .options-row {
    margin-top: 16px;
    display: flex;
    align-items: center;
    justify-content: flex-start;
  }

  /* ─── Toggle ──────────────────────────────────────── */
  .toggle-row {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    cursor: pointer;
    user-select: none;
    color: var(--text-2);
    font-size: 13px;
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

  /* ─── Status bar ──────────────────────────────────── */
  .status-bar {
    margin-top: 18px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
    font-size: 13px;
    padding: 12px 14px;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    border: 1px solid var(--glass-border-soft);
  }

  .status-dot {
    width: 8px; height: 8px;
    border-radius: 50%;
    background: var(--highlight);
    flex-shrink: 0;
  }

  /* ─── Spinner & animations ────────────────────────── */
  .spinner {
    width: 13px; height: 13px;
    border: 2px solid rgba(255,255,255,0.25);
    border-top-color: var(--accent-fg);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .pulse { animation: pulse 1.6s ease-in-out infinite; }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.35; } }

  /* ─── Error ───────────────────────────────────────── */
  .error-card {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 14px 16px;
    background: var(--error-bg);
    border-color: var(--error-border);
    border-radius: var(--radius-sm);
    color: var(--error);
    font-size: 13px;
  }
  .error-card svg { flex-shrink: 0; margin-top: 1px; }

  /* ─── Result ──────────────────────────────────────── */
  .result-card { padding: 0; overflow: hidden; }

  .result-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--glass-border-soft);
    gap: 12px;
    flex-wrap: wrap;
  }

  .tabs { display: flex; gap: 2px; }

  .tab {
    background: none;
    border: none;
    color: var(--text-2);
    cursor: pointer;
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    padding: 6px 12px;
    border-radius: var(--radius-xs);
    transition: color 0.2s, background 0.2s;
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--text); background: var(--surface-2); }

  .result-actions { display: flex; align-items: center; gap: 8px; }

  .lang-badge {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    color: var(--text-2);
    background: var(--surface-2);
    padding: 4px 9px;
    border-radius: 20px;
    border: 1px solid var(--glass-border-soft);
  }

  .result-body { padding: 20px; max-height: 480px; overflow-y: auto; }

  .transcript {
    font-family: 'Fraunces', 'Iowan Old Style', Georgia, serif;
    font-size: 16px;
    line-height: 1.8;
    color: var(--text);
    white-space: pre-wrap;
    word-break: break-word;
    letter-spacing: -0.1px;
  }

  /* ─── Sections ────────────────────────────────────── */
  .section { display: flex; flex-direction: column; gap: 22px; }

  .section-header { text-align: center; }

  .eyebrow {
    display: inline-block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.18em;
    color: var(--text-3);
    margin-bottom: 8px;
  }

  .section-title {
    font-family: 'Fraunces', 'Cormorant Garamond', Georgia, serif;
    font-size: 36px;
    font-weight: 400;
    letter-spacing: -1px;
    color: var(--text);
    line-height: 1.15;
  }

  /* ─── Steps grid ──────────────────────────────────── */
  .steps-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 14px;
  }
  @media (min-width: 640px) {
    .steps-grid { grid-template-columns: repeat(3, 1fr); }
  }

  .step-card {
    border-radius: var(--radius);
    padding: 22px 20px;
    transition: transform 0.3s ease, box-shadow 0.3s ease;
  }
  .step-card:hover {
    transform: translateY(-3px);
    box-shadow: var(--glass-shadow-lg);
  }

  .step-number {
    font-family: 'Fraunces', Georgia, serif;
    font-size: 32px;
    color: var(--highlight);
    margin-bottom: 14px;
    line-height: 1;
    font-style: italic;
    font-weight: 400;
  }

  .step-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
    margin-bottom: 8px;
    letter-spacing: -0.2px;
  }

  .step-desc {
    font-size: 14px;
    color: var(--text-2);
    line-height: 1.7;
  }

  /* ─── FAQ ─────────────────────────────────────────── */
  .faq-list { display: flex; flex-direction: column; gap: 10px; }

  .faq-item {
    border-radius: var(--radius-sm);
    overflow: hidden;
    transition: box-shadow 0.25s ease;
  }
  .faq-item.open { box-shadow: var(--glass-shadow-lg); }

  .faq-trigger {
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 18px 22px;
    text-align: left;
    font-family: inherit;
    font-size: 15px;
    font-weight: 500;
    color: var(--text);
    transition: background 0.2s;
  }
  .faq-trigger:hover { background: var(--surface-2); }

  .faq-chevron { flex-shrink: 0; color: var(--text-3); transition: transform 0.25s ease; }
  .faq-chevron.rotated { transform: rotate(180deg); }

  .faq-body {
    padding: 0 22px 20px;
    font-size: 14.5px;
    color: var(--text-2);
    line-height: 1.75;
  }

  /* ─── Footer ──────────────────────────────────────── */
  footer {
    position: relative;
    z-index: 1;
    padding: 32px 20px 28px;
  }

  .footer-inner {
    max-width: 720px;
    margin: 0 auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px;
    padding-top: 24px;
    border-top: 1px solid var(--glass-border-soft);
  }

  .footer-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .footer-copy {
    font-size: 12px;
    color: var(--text-3);
  }

  /* ─── Streaming transcript ────────────────────────── */
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

  .live-badge {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    font-weight: 600;
    color: var(--highlight);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--highlight);
    flex-shrink: 0;
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

  .no-speech {
    font-size: 14px;
    color: var(--text-3);
    font-style: italic;
  }
</style>
