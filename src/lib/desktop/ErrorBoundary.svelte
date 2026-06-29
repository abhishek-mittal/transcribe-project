<script>
  import { onMount, onDestroy } from 'svelte';

  // True when Vite is running in development mode (tauri dev / npm run dev).
  const IS_DEV = import.meta.env.DEV;

  let hasError = false;
  let errorMessage = '';
  let countdown = 2;

  /** @type {ReturnType<typeof setInterval> | null} */
  let tickTimer = null;

  /**
   * Central handler: log the error to disk via Tauri (sidecar.log +
   * crash-log.json), show the overlay, and — in production only — force-quit
   * the process after 2 seconds. In dev mode the overlay stays visible so the
   * developer can read the error without the window disappearing.
   * @param {string} message
   */
  async function handleCrash(message) {
    if (hasError) return; // only handle the first error
    hasError = true;
    errorMessage = message;

    // Always log to sidecar.log and crash-log.json regardless of mode.
    await persistError(message);

    if (IS_DEV) {
      // Dev mode: keep the window open so the developer can read the error.
      // The overlay clearly labels itself as dev mode.
      console.error('[ErrorBoundary DEV] Caught error — app kept open for debugging:', message);
      return;
    }

    // Production mode: countdown and exit.
    tickTimer = setInterval(() => {
      countdown -= 1;
      if (countdown <= 0) {
        clearInterval(tickTimer);
        tickTimer = null;
        forceQuit();
      }
    }, 1000);
  }

  /**
   * Write the crash details to:
   *   1. ~/Library/Logs/com.shuhari.transcribe/sidecar.log  (via log_error command)
   *   2. <app-data-dir>/crash-log.json                       (via write_crash_log command)
   *      readable by agents at a predictable path
   * @param {string} message
   */
  async function persistError(message) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      // Fire both in parallel; failures are swallowed inside Rust — never crash here.
      await Promise.allSettled([
        invoke('log_error', { message }),
        invoke('write_crash_log', { message }),
      ]);
    } catch (e) {
      console.error('[ErrorBoundary] persistError failed:', e);
    }
  }

  async function forceQuit() {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('force_quit');
    } catch {
      window.close();
    }
  }

  // ── Global error handlers ──────────────────────────────────────────────

  /** @type {OnErrorEventHandler | null} */
  let prevOnerror = null;
  /** @type {((e: PromiseRejectionEvent) => void) | null} */
  let prevOnunhandledrejection = null;

  onMount(() => {
    prevOnerror = window.onerror;
    window.onerror = (msg, src, line, col, err) => {
      const stack = err?.stack || '';
      const detail = stack || `${src}:${line}:${col}`;
      handleCrash(`${msg}\n${detail}`);
      return true;
    };

    prevOnunhandledrejection = window.onunhandledrejection;
    window.onunhandledrejection = (event) => {
      const reason = event?.reason;
      const msg =
        reason instanceof Error
          ? `${reason.message}\n${reason.stack || ''}`
          : String(reason ?? 'Unhandled promise rejection');
      handleCrash(msg);
    };
  });

  onDestroy(() => {
    window.onerror = prevOnerror;
    window.onunhandledrejection = prevOnunhandledrejection;
    if (tickTimer) {
      clearInterval(tickTimer);
      tickTimer = null;
    }
  });
</script>

{#if hasError}
  <div class="crash-overlay" role="alert" aria-live="assertive">
    <div class="crash-card">
      <div class="crash-icon">✕</div>
      <h1 class="crash-heading">Something went wrong</h1>
      {#if IS_DEV}
        <p class="crash-sub dev-badge">DEV MODE — window kept open for debugging</p>
        <p class="crash-sub">Error logged to <code>sidecar.log</code> and <code>crash-log.json</code>.</p>
      {:else}
        <p class="crash-sub">An unexpected error occurred. The app will close in {countdown}s.</p>
      {/if}
      {#if errorMessage}
        <pre class="crash-detail">{errorMessage}</pre>
      {/if}
    </div>
  </div>
{:else}
  <slot />
{/if}

<style>
  .crash-overlay {
    position: fixed;
    inset: 0;
    z-index: 99999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #0f0f0f;
    color: #f5f5f5;
    font-family: system-ui, sans-serif;
  }

  .crash-card {
    max-width: 560px;
    padding: 2.5rem 2rem;
    text-align: center;
  }

  .crash-icon {
    width: 3rem;
    height: 3rem;
    border-radius: 50%;
    background: #7f1d1d;
    color: #fca5a5;
    font-size: 1.5rem;
    line-height: 3rem;
    margin: 0 auto 1.25rem;
  }

  .crash-heading {
    font-size: 1.375rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
    color: #f5f5f5;
  }

  .crash-sub {
    font-size: 0.9375rem;
    color: #a3a3a3;
    margin: 0 0 0.5rem;
    line-height: 1.5;
  }

  .crash-sub code {
    font-family: 'Menlo', 'Consolas', monospace;
    font-size: 0.875rem;
    color: #d4d4d4;
    background: #1e1e1e;
    border-radius: 3px;
    padding: 1px 4px;
  }

  .dev-badge {
    color: #fbbf24;
    font-weight: 600;
    margin-bottom: 0.25rem;
  }

  .crash-detail {
    text-align: left;
    font-family: 'Menlo', 'Consolas', monospace;
    font-size: 0.75rem;
    color: #a3a3a3;
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    padding: 0.875rem 1rem;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 240px;
    overflow-y: auto;
    margin: 0.75rem 0 0;
  }
</style>
