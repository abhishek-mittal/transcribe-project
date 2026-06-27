<script>
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { onMount } from 'svelte';
  import UrlDropZone from '$lib/desktop/UrlDropZone.svelte';
  import SaveTranscriptButton from '$lib/desktop/SaveTranscriptButton.svelte';
  import DesktopTitleBar from '$lib/desktop/DesktopTitleBar.svelte';
  import KeyboardShortcuts from '$lib/desktop/KeyboardShortcuts.svelte';
  import SidebarNav from '$lib/desktop/SidebarNav.svelte';
  import UrlInputPanel from '$lib/desktop/UrlInputPanel.svelte';
  import TranscriptPanel from '$lib/desktop/TranscriptPanel.svelte';
  import SettingsView from '$lib/desktop/SettingsView.svelte';
  import VideoPicker from '$lib/desktop/VideoPicker.svelte';
  import QueueView from '$lib/desktop/QueueView.svelte';
  import JobHistoryView from '$lib/desktop/JobHistoryView.svelte';

  let url = '';
  // Default 'tiny'; user-selectable via Settings (F06). Persisted to settings.json.
  let model = 'tiny';
  let timestamps = true;
  let maxHistoryRecords = 500;
  let settingsLoaded = false;
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

  /** @type {'idle' | 'downloading' | 'transcribing' | 'done'} */
  let phase = 'idle';
  /**
   * Segments with typewriter state. `displayed` grows char-by-char until it equals `text`.
   * @type {Array<{index: number, text: string, start: number, end: number, ts?: string, displayed: string}>}
   */
  let streamSegments = [];
  let isTyping = false;
  /** Index in streamSegments currently being typed, -1 when idle */
  let typingIdx = -1;
  /** @type {ReturnType<typeof setTimeout> | null} */
  let typewriterTimer = null;
  const CHAR_DELAY = 16; // ms per character (~60 fps)

  // ── Tauri desktop integration ──────────────────────────────────────────
  // When running inside the Tauri webview, import the Tauri APIs lazily so
  // the app still loads in a regular browser for development.
  /** @type {any} */
  let invokeFn = null;
  /** @type {any} */
  let listenFn = null;
  /** @type {any} */
  let dialogSave = null;

  // ── Queue / batch job state ──────────────────────────────
  /**
   * Single batch job. `items` are processed sequentially — one sidecar
   * spawn at a time, results streamed back via `transcribe-progress`
   * events. Status transitions: waiting → downloading → transcribing → done
   * (or failed / cancelled). Download progress fields are optional and
   * populated incrementally as yt-dlp reports each progress tick.
   * @type {{
   *   id: string,
   *   model: string,
   *   timestamps: boolean,
   *   createdAt: string,
   *   items: Array<{
   *     id: string,
   *     url: string,
   *     title: string,
   *     thumbnail: string,
   *     duration: number,
   *     status: 'waiting' | 'starting' | 'downloading' | 'transcribing' | 'done' | 'failed' | 'cancelled',
   *     error: string | null,
   *     errorCode: string | null,
   *     result: { language: string, plain: string, timestamped: string | null, srt: string } | null,
   *     startedAt: string | null,
   *     completedAt: string | null,
   *     wordCount: number | null,
   *     downloadPercent: number | null,
   *     downloadedBytes: number | null,
   *     totalBytes: number | null,
   *     speedBps: number | null,
   *     etaSecs: number | null,
   *     streamSegments?: Array<{ index: number, text: string, start: number, end: number, ts?: string, displayed: string }>,
   *   }>,
   * } | null}
   */
  let currentJob = null;
  let queueActive = false;
  /** @type {Array<any>} */
  let jobs = [];

  /**
   * Map a sidecar error code to a user-facing message. See
   * openspec/changes/tauri-desktop-app/specs/python-sidecar for codes.
   * @param {string} code
   * @param {string} fallback
   */
  function errorMessageFor(code, fallback) {
    switch (code) {
      case 'INVALID_URL':
        return 'Please enter a valid http(s) URL.';
      case 'INVALID_MODEL':
        return 'Unsupported model selected. Change it in Settings.';
      case 'NETWORK':
        return 'Network error. Check your connection and try again.';
      case 'BOT_CHALLENGE':
        return 'YouTube is blocking this video. Try a different one or open it in your browser first.';
      case 'UNSUPPORTED_PLATFORM':
        return 'This URL is not supported. Try a YouTube, Instagram, or TikTok link.';
      case 'FFMPEG_MISSING':
        return 'FFmpeg is required. Install with `brew install ffmpeg` (macOS) and restart the app.';
      case 'MODEL_LOAD_FAILED':
        return 'Failed to load the speech model. Check your internet connection and try again.';
      case 'PLACEHOLDER_SIDECAR':
        return 'This is a placeholder sidecar. Run `python3 scripts/build_sidecar.py` to build the real transcription binary, then restart the app.';
      default:
        return fallback || 'Something went wrong. Please try again.';
    }
  }

  // True if window.__TAURI_INTERNALS__ is exposed (Tauri 2.x).
  // Initialize to false on first render; onMount updates it after the
  // webview has injected Tauri internals.
  let isTauri = false;

  // Bind Tauri APIs on mount if running in the desktop app.
  onMount(async () => {
    isTauri = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
    console.log('[+page] onMount, isTauri=', isTauri);
    if (!isTauri) return;
    try {
      const core = await import('@tauri-apps/api/core');
      const event = await import('@tauri-apps/api/event');
      const dialog = await import('@tauri-apps/plugin-dialog');
      invokeFn = core.invoke;
      listenFn = event.listen;
      dialogSave = dialog.save;
      console.log('[+page] Tauri APIs loaded, invokeFn=', typeof invokeFn, 'listenFn=', typeof listenFn);
    } catch (e) {
      console.warn('Failed to load Tauri APIs:', e);
      return;
    }

    try {
      const settings = await invokeFn('load_settings');
      darkMode = settings.dark_mode;
      model = settings.model;
      timestamps = settings.timestamps;
      maxHistoryRecords = settings.max_history_records;
    } catch (e) {
      console.warn('load_settings failed:', e);
    } finally {
      settingsLoaded = true;
    }

    try {
      historyRecords = await invokeFn('load_history');
    } catch (e) {
      console.warn('load_history failed:', e);
    }

    try {
      jobs = await invokeFn('load_jobs');
    } catch (e) {
      console.warn('load_jobs failed:', e);
    }
  });

  /**
   * Persist the given partial settings (merged with current in-memory
   * values) to settings.json. Fire-and-forget.
   * @param {Partial<{model: string, timestamps: boolean, darkMode: boolean}>} changed
   */
  function persistSettings(changed = {}) {
    if (!invokeFn) return;
    const next = {
      version: 1,
      model: changed.model ?? model,
      timestamps: changed.timestamps ?? timestamps,
      dark_mode: changed.darkMode ?? darkMode,
      max_history_records: maxHistoryRecords,
    };
    invokeFn('save_settings', { settings: next }).catch((e) =>
      console.warn('save_settings failed:', e)
    );
  }

  /** @param {{ detail: { model?: string, timestamps?: boolean, darkMode?: boolean } }} e */
  function handleSettingsChange(e) {
    persistSettings(e.detail);
  }

  function handleSidebarThemeToggle() {
    darkMode = !darkMode;
    persistSettings({ darkMode });
  }

  // ── Queue runner ─────────────────────────────────────────

  /** @param {string} code @param {string} fallback */
  function queueErrorMessage(code, fallback) {
    return errorMessageFor(code, fallback);
  }

  let queueUnlisten = null;
  let queueCurrentItemId = null;

  // ── Activity log (drives the live event panel under the queue list) ────────
  /**
   * Each entry is one sidecar event formatted for human reading.
   * @type {Array<{ id: number, ts: string, severity: 'info'|'success'|'warn'|'error', message: string }>}
   */
  let activityLog = [];
  let activityLogId = 0;
  /** Hard cap to keep memory bounded during long sessions. */
  const ACTIVITY_LOG_MAX = 500;

  /**
   * Format and append a sidecar event payload to the activity log. Safe to
   * call from both the queue runner and the single-video path — they share
   * the same store so the panel shows the most recent activity regardless
   * of which flow produced it.
   * @param {any} payload
   */
  function logActivity(payload) {
    if (!payload || typeof payload !== 'object') return;
    const formatted = formatActivityEvent(payload);
    if (!formatted) return;
    // Stamp the entry with the current local time. Done here (not inside
    // formatActivityEvent) so every entry gets a `ts` even though the
    // formatter only returns `{ severity, message }`.
    const entry = {
      id: ++activityLogId,
      ts: formatLogTime(new Date()),
      severity: formatted.severity,
      message: formatted.message,
    };
    activityLog = [...activityLog, entry];
    if (activityLog.length > ACTIVITY_LOG_MAX) {
      activityLog = activityLog.slice(activityLog.length - ACTIVITY_LOG_MAX);
    }
  }

  /**
   * Short HH:MM:SS timestamp for the activity-log left column. Seconds
   * resolution is enough to read a sequence of events.
   * @param {Date} d
   */
  function formatLogTime(d) {
    const pad = (n) => String(n).padStart(2, '0');
    return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  }

  function clearActivityLog() {
    activityLog = [];
  }

  function copyActivityLog() {
    if (activityLog.length === 0) return;
    const text = activityLog
      .map((e) => `[${e.ts}] ${e.message}`)
      .join('\n');
    if (typeof navigator !== 'undefined' && navigator.clipboard) {
      navigator.clipboard.writeText(text).catch(() => {});
    }
  }

  // Stable reference so ActivityLogPanel doesn't get a new object prop on every render.
  const activityHandlers = { onClear: clearActivityLog, onCopy: copyActivityLog };

  /**
   * Map a raw `transcribe-progress` payload to a human-readable log line.
   * Returns null for payloads we don't bother surfacing (e.g. heartbeat
   * events the sidecar might add later).
   * @param {any} payload
   * @returns {{ severity: 'info'|'success'|'warn'|'error', message: string } | null}
   */
  function formatActivityEvent(payload) {
    const event = payload.event;
    if (event === 'phase') {
      const phase = payload.phase;
      if (phase === 'starting') {
        return { severity: 'info', message: 'starting · spawning sidecar' };
      }
      if (phase === 'downloading-model') {
        const pct = typeof payload.progress === 'number'
          ? ` ${Math.round(payload.progress * 100)}%`
          : '';
        return { severity: 'info', message: `downloading model${pct}` };
      }
      if (phase === 'downloading') {
        return { severity: 'info', message: 'downloading audio · resolving source' };
      }
      if (phase === 'downloading-audio') {
        if (typeof payload.percent === 'number' && payload.percent >= 100) {
          return { severity: 'info', message: 'download complete' };
        }
        if (typeof payload.percent === 'number') {
          const dl = fmtBytes(payload.downloaded_bytes);
          const tot = fmtBytes(payload.total_bytes);
          const sp = fmtSpeed(payload.speed_bps);
          const size = dl && tot ? ` ${dl} / ${tot}` : '';
          const rate = sp ? ` · ↓ ${sp}` : '';
          return {
            severity: 'info',
            message: `downloading · ${payload.percent.toFixed(1)}%${size}${rate}`,
          };
        }
        return { severity: 'info', message: 'downloading audio' };
      }
      if (phase === 'transcribing') {
        return { severity: 'info', message: 'transcribing · loaded speech model' };
      }
      return { severity: 'info', message: `phase: ${phase}` };
    }
    if (event === 'progress') {
      if (payload.phase === 'transcribing' && payload.text) {
        const snippet = String(payload.text).trim().slice(0, 80);
        const idx = typeof payload.segment === 'number' ? ` #${payload.segment}` : '';
        return { severity: 'info', message: `segment${idx} · "${snippet}${snippet.length === 80 ? '…' : ''}"` };
      }
      return null;
    }
    if (event === 'result') {
      const wc = typeof payload.word_count === 'number' ? payload.word_count : null;
      const lang = payload.language ? ` · ${payload.language.toUpperCase()}` : '';
      return {
        severity: 'success',
        message: `transcription complete${lang}${wc ? ` · ${wc.toLocaleString()} words` : ''}`,
      };
    }
    if (event === 'error') {
      return {
        severity: 'error',
        message: `error · ${payload.code || 'UNKNOWN'}${payload.message ? ` — ${payload.message}` : ''}`,
      };
    }
    if (event === 'terminated') {
      const code = payload.code;
      const sig = payload.signal;
      if (code === 0 || code == null) {
        return { severity: 'info', message: 'sidecar exited' };
      }
      const sigName = sig != null ? ` (signal ${sig})` : '';
      return { severity: 'warn', message: `sidecar terminated · exit ${code}${sigName}` };
    }
    if (event === 'done') {
      return { severity: 'info', message: 'finished' };
    }
    return null;
  }

  /** @param {unknown} bytes */
  function fmtBytes(bytes) {
    if (typeof bytes !== 'number' || !Number.isFinite(bytes) || bytes <= 0) return null;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
    return `${(kb / 1024).toFixed(kb / 1024 < 10 ? 2 : 1)} MB`;
  }

  /** @param {unknown} bps */
  function fmtSpeed(bps) {
    if (typeof bps !== 'number' || !Number.isFinite(bps) || bps <= 0) return null;
    const kbps = bps / 1024;
    if (kbps < 1024) return `${kbps.toFixed(kbps < 10 ? 1 : 0)} KB/s`;
    return `${(kbps / 1024).toFixed(1)} MB/s`;
  }

  async function runQueueLoop() {
    if (!currentJob) return;

    // Subscribe to sidecar events ONCE for the whole job, not per-item.
    // Per-item subscription raced with the invoke response: Tauri's IPC
    // channel can deliver the `terminated` event AFTER `await invokeFn(
    // 'run_sidecar')` resolves, and the next loop iteration would
    // unsubscribe before that late event drained. The listener now lives
    // for the full job lifetime and routes events to the current item.
    let currentItemIndex = -1;
    if (listenFn) {
      if (queueUnlisten) { try { queueUnlisten(); } catch {} }
      try {
        queueUnlisten = await listenFn('transcribe-progress', (event) => {
          if (currentItemIndex < 0) return;
          handleQueueItemEvent(event.payload, currentItemIndex);
          logActivity(event.payload);
        });
      } catch (e) {
        console.warn('queue: failed to subscribe to events:', e);
      }
    }

    try {
      for (let i = 0; i < currentJob.items.length; i++) {
        const item = currentJob.items[i];
        if (item.status !== 'waiting') continue;

        // Mark as downloading
        currentJob.items[i] = { ...item, status: 'downloading', startedAt: new Date().toISOString(), streamSegments: [] };
        currentJob = { ...currentJob };
        queueCurrentItemId = item.id;
        currentItemIndex = i;

        try {
          if (invokeFn) {
            await invokeFn('run_sidecar', { url: item.url, model: currentJob.model, timestamps: currentJob.timestamps });
          }
        } catch (e) {
          currentJob.items[i] = {
            ...currentJob.items[i],
            status: 'failed',
            error: queueErrorMessage(null, String(e)),
            completedAt: new Date().toISOString(),
          };
          currentJob = { ...currentJob };
        }
        currentItemIndex = -1;
      }
    } finally {
      // Tear the listener down only when the job is done, so any events
      // that arrive between the last invoke resolving and this point still
      // reach the UI.
      if (queueUnlisten) { try { queueUnlisten(); queueUnlisten = null; } catch {} }
    }

    queueCurrentItemId = null;
    queueActive = false;
    currentJob = { ...currentJob };
    await finalizeJob();
  }

  /** @param {any} payload @param {number} itemIndex */
  function handleQueueItemEvent(payload, itemIndex) {
    if (!currentJob || !payload || typeof payload !== 'object') return;
    const item = currentJob.items[itemIndex];
    if (!item) return;
    const event = payload.event;

    if (event === 'phase') {
      if (payload.phase === 'starting') {
        currentJob.items[itemIndex] = { ...item, status: 'starting' };
      } else if (payload.phase === 'downloading' || payload.phase === 'downloading-model') {
        currentJob.items[itemIndex] = { ...item, status: 'downloading' };
      } else if (payload.phase === 'downloading-audio') {
        currentJob.items[itemIndex] = {
          ...item,
          status: 'downloading',
          downloadPercent: typeof payload.percent === 'number' ? payload.percent : item.downloadPercent ?? null,
          downloadedBytes: payload.downloaded_bytes ?? item.downloadedBytes ?? null,
          totalBytes: payload.total_bytes ?? item.totalBytes ?? null,
          speedBps: payload.speed_bps ?? null,
          etaSecs: payload.eta_secs ?? null,
        };
      } else if (payload.phase === 'transcribing') {
        currentJob.items[itemIndex] = { ...item, status: 'transcribing', streamSegments: item.streamSegments || [] };
      }
      currentJob = { ...currentJob };
    } else if (event === 'progress') {
      if (payload.phase === 'transcribing' && payload.text) {
        const segs = [...(item.streamSegments || []), { index: payload.segment ?? (item.streamSegments || []).length, text: payload.text, start: payload.start ?? 0, end: payload.end ?? 0, ts: payload.ts, displayed: payload.text }];
        currentJob.items[itemIndex] = { ...item, status: 'transcribing', streamSegments: segs };
        currentJob = { ...currentJob };
      }
    } else if (event === 'result') {
      const wordCount = getWordCount(payload.plain || '');
      const transcriptResult = { language: payload.language, plain: payload.plain, timestamped: payload.timestamped, srt: payload.srt };
      currentJob.items[itemIndex] = {
        ...item,
        status: 'done',
        result: transcriptResult,
        wordCount,
        completedAt: new Date().toISOString(),
      };
      currentJob = { ...currentJob };

      // Save individual transcript to history
      if (invokeFn) {
        invokeFn('save_transcript', {
          record: {
            url: item.url,
            title: item.title,
            language: payload.language,
            plain: payload.plain,
            timestamped: payload.timestamped,
            srt: payload.srt,
            model: currentJob.model,
            word_count: wordCount,
          },
        })
          .then((id) => {
            historyRecords = [
              { id, url: item.url, title: item.title, language: payload.language, plain: payload.plain, timestamped: payload.timestamped, srt: payload.srt, model: currentJob.model, word_count: wordCount, created_at: new Date().toISOString() },
              ...historyRecords,
            ];
          })
          .catch((e) => console.warn('queue: save_transcript failed:', e));
      }
    } else if (event === 'error') {
      // Don't overwrite a completed transcription — cleanup errors that
      // arrive after the result event are non-fatal (e.g. tmpdir rmtree
      // failure, or a PyInstaller multiprocessing child running main()).
      if (item.status === 'done') return;
      // Store the raw sidecar message so History can show the actual reason.
      currentJob.items[itemIndex] = {
        ...item,
        status: 'failed',
        error: payload.message || queueErrorMessage(payload.code, ''),
        errorCode: payload.code,
        completedAt: new Date().toISOString(),
      };
      currentJob = { ...currentJob };
    } else if (event === 'terminated') {
      // Don't overwrite 'done' or 'failed' — those were set by result/error events.
      if (item.status !== 'done' && item.status !== 'failed') {
        currentJob.items[itemIndex] = { ...item, status: 'cancelled', completedAt: new Date().toISOString() };
        currentJob = { ...currentJob };
      }
    }
  }

  async function finalizeJob() {
    if (!currentJob || !invokeFn) return;
    const now = new Date().toISOString();
    const start = new Date(currentJob.createdAt).getTime();
    const elapsedMs = Date.now() - start;
    const successCount = currentJob.items.filter((i) => i.status === 'done').length;
    const failureCount = currentJob.items.filter((i) => i.status === 'failed').length;
    const cancelledCount = currentJob.items.filter((i) => i.status === 'cancelled').length;
    const totalWords = currentJob.items.reduce((s, i) => s + (i.wordCount || 0), 0);
    const totalAudioSecs = currentJob.items.filter((i) => i.status === 'done').reduce((s, i) => s + (i.duration || 0), 0);

    const jobRecord = {
      id: currentJob.id,
      model: currentJob.model,
      timestamps: currentJob.timestamps,
      created_at: currentJob.createdAt,
      completed_at: now,
      elapsed_ms: elapsedMs,
      total_items: currentJob.items.length,
      success_count: successCount,
      failure_count: failureCount,
      cancelled_count: cancelledCount,
      total_words: totalWords,
      total_audio_secs: totalAudioSecs,
      items: currentJob.items.map((item) => ({
        id: item.id,
        url: item.url,
        title: item.title,
        thumbnail: item.thumbnail,
        duration_secs: item.duration || 0,
        status: item.status,
        error_code: item.errorCode || null,
        error_message: item.error || null,
        language: item.result?.language || null,
        plain: item.result?.plain || null,
        timestamped: item.result?.timestamped || null,
        srt: item.result?.srt || null,
        word_count: item.wordCount || null,
        started_at: item.startedAt || null,
        completed_at: item.completedAt || null,
        elapsed_ms: item.startedAt && item.completedAt
          ? new Date(item.completedAt).getTime() - new Date(item.startedAt).getTime()
          : null,
        download_percent: typeof item.downloadPercent === 'number' ? item.downloadPercent : null,
        downloaded_bytes: typeof item.downloadedBytes === 'number' ? item.downloadedBytes : null,
        total_bytes: typeof item.totalBytes === 'number' ? item.totalBytes : null,
      })),
    };

    try {
      await invokeFn('save_job', { job: jobRecord });
      jobs = [jobRecord, ...jobs];
    } catch (e) {
      console.warn('queue: save_job failed:', e);
    }
  }

  async function cancelQueueJob() {
    if (!currentJob) return;
    currentJob.items = currentJob.items.map((item) =>
      item.status === 'waiting' ? { ...item, status: 'cancelled' } : item
    );
    currentJob = { ...currentJob };
    if (invokeFn) {
      try { await invokeFn('cancel_transcribe'); } catch {}
    }
    queueActive = false;
  }

  /** @param {string} itemId */
  async function retryQueueItem(itemId) {
    if (!currentJob) return;
    const idx = currentJob.items.findIndex((i) => i.id === itemId);
    if (idx < 0) return;
    currentJob.items[idx] = {
      ...currentJob.items[idx],
      status: 'waiting',
      error: null,
      errorCode: null,
      result: null,
      startedAt: null,
      completedAt: null,
      streamSegments: [],
      downloadPercent: null,
      downloadedBytes: null,
      totalBytes: null,
      speedBps: null,
      etaSecs: null,
    };
    currentJob = { ...currentJob };
    queueActive = true;
    runQueueLoop();
  }

  /** @param {string} itemId */
  async function cancelQueueItem(itemId) {
    if (!currentJob) return;
    const item = currentJob.items.find((i) => i.id === itemId);
    if (!item) return;

    if (item.status === 'waiting') {
      // Pending item — just mark cancelled, no sidecar running for it.
      currentJob.items = currentJob.items.map((it) =>
        it.id === itemId ? { ...it, status: 'cancelled', completedAt: new Date().toISOString() } : it
      );
      currentJob = { ...currentJob };
      return;
    }

    if (item.status === 'starting' || item.status === 'downloading' || item.status === 'transcribing') {
      // Active item — mark it cancelled locally, kill the sidecar, then
      // finalise the job so the cancelled status is persisted to history.
      // The sidecar's `terminated` event will also mark the item cancelled
      // when it arrives, but we don't want to wait for it to update the UI.
      currentJob.items = currentJob.items.map((it) =>
        it.id === itemId
          ? { ...it, status: 'cancelled', completedAt: new Date().toISOString() }
          : it
      );
      currentJob = { ...currentJob };

      if (invokeFn) {
        try { await invokeFn('cancel_transcribe'); } catch {}
      }
      // Note: the runner is still mid-loop waiting for run_sidecar to
      // resolve. We don't abort the loop — it will pick up the next item
      // (or hit the `terminated` event and exit cleanly).
    }
  }

  // Persist the timestamps default whenever the user flips the toggle in
  // UrlInputPanel (which binds `timestamps` directly) — skipped until the
  // initial load_settings call completes so we don't overwrite with the
  // default value before the saved one arrives.
  $: if (settingsLoaded) persistSettings({ timestamps });

  async function handleClearHistory() {
    if (!invokeFn) return;
    try {
      await invokeFn('clear_history');
      historyRecords = [];
    } catch (e) {
      console.warn('clear_history failed:', e);
    }
  }

  // ── Desktop shell state ──────────────────────────────
  /** @type {'transcribe' | 'history' | 'settings' | 'picker' | 'queue'} */
  let activeView = 'transcribe';

  // Probe state lifted to page level so it survives tab switches.
  /** @type {'idle' | 'probing' | 'preview' | 'error'} */
  let probeState = 'idle';
  /** @type {any | null} */
  let probeResult = null;
  /** @type {string | null} */
  let probeError = null;

  // Clear duplicate notice when URL changes (not on first render).
  let _prevUrlForDup = url;
  $: if (url !== _prevUrlForDup) { _prevUrlForDup = url; duplicateMatch = null; }

  /** @type {any | null} probe result when a playlist is detected */
  let playlistProbeResult = null;

  /** @type {Array<any>} */
  let historyRecords = [];
  /** @type {any | null} */
  let selectedHistoryRecord = null;

  /** Number of videos currently selected in the picker (kept in sync via bind) */
  let pickerSelectedCount = 0;

  /**
   * Duplicate match: set when the URL already has any prior record in history
   * (done, failed, or cancelled). Surfaced in UrlInputPanel so the user can
   * choose to view the existing record or transcribe again. Retrying from
   * History reuses the prior record's IDs so it doesn't create a duplicate.
   * @type {{ jobId: string, itemId: string, title: string, priorStatus: 'done' | 'failed' | 'cancelled' } | null}
   */
  let duplicateMatch = null;

  /**
   * Single-video transcription routed through the queue (like batch).
   * Replaces the old direct `transcribe()` call for the desktop app.
   */
  function handleTranscribeViaQueue() {
    const trimmed = url.trim();
    if (!trimmed) return;

    // Check if this URL already has any prior record (done/failed/cancelled).
    // Prefer a done record; if none exists, surface the most-recent
    // non-waiting one so the user is warned before stacking more retries.
    duplicateMatch = null;
    let bestMatch = null;
    for (const job of jobs) {
      const found = job.items.find((i) => i.url === trimmed);
      if (!found) continue;
      if (found.status === 'done') {
        bestMatch = { jobId: job.id, itemId: found.id, title: found.title || trimmed, priorStatus: 'done' };
        break;
      }
      if (!bestMatch && (found.status === 'failed' || found.status === 'cancelled')) {
        bestMatch = { jobId: job.id, itemId: found.id, title: found.title || trimmed, priorStatus: found.status };
      }
    }
    if (bestMatch) {
      duplicateMatch = bestMatch;
      return;
    }

    const entry = probeResult
      ? {
          id: crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2),
          url: probeResult.url || trimmed,
          title: probeResult.title || trimmed,
          thumbnail: probeResult.thumbnail || '',
          duration: probeResult.duration || 0,
        }
      : {
          id: crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2),
          url: trimmed,
          title: trimmed,
          thumbnail: '',
          duration: 0,
        };

    startBatchJob([entry]);
  }

  /** Navigate to history and highlight the duplicate's job */
  function viewDuplicate() {
    activeView = 'history';
    duplicateMatch = null;
  }

  /** Dismiss the duplicate warning and transcribe again */
  function forceTranscribe() {
    duplicateMatch = null;
    const trimmed = url.trim();
    if (!trimmed) return;
    const entry = probeResult
      ? {
          id: crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2),
          url: probeResult.url || trimmed,
          title: probeResult.title || trimmed,
          thumbnail: probeResult.thumbnail || '',
          duration: probeResult.duration || 0,
        }
      : {
          id: crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2),
          url: trimmed,
          title: trimmed,
          thumbnail: '',
          duration: 0,
        };
    startBatchJob([entry]);
  }

  /** Called when the user clicks "Transcribe X videos" in picker mode */
  function handleTranscribePicker() {
    // Primary path is via VideoPicker startJob event.
  }

  /** @param {CustomEvent} e */
  function handleStartJob(e) {
    const selected = e.detail.selected;
    if (!selected || selected.length === 0) return;
    startBatchJob(selected);
  }

  /**
   * Start a batch job. If `reuse` is provided, the new job reuses the prior
   * `jobId` and the matching `itemId` per URL. This is what makes "Retry"
   * from History replace the prior record instead of stacking a duplicate.
   * @param {Array<{ url: string, title?: string, thumbnail?: string, duration?: number }>} selectedEntries
   * @param {{ jobId: string, itemIdsByUrl: Map<string, string> } | null} [reuse]
   */
  function startBatchJob(selectedEntries, reuse = null) {
    const jobId = reuse?.jobId
      ?? (crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2));
    const newId = () => (crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2));
    currentJob = {
      id: jobId,
      model,
      timestamps,
      createdAt: new Date().toISOString(),
      items: selectedEntries.map((entry) => ({
        // Reuse the prior item id when the URL matches. Rust `save_job`
        // already retains by `job.id` and replaces the record, but reusing
        // item ids keeps stable references if the queue is mid-flight when
        // another retry is dispatched.
        id: reuse?.itemIdsByUrl?.get(entry.url) ?? newId(),
        url: entry.url,
        title: entry.title ?? entry.url,
        thumbnail: entry.thumbnail ?? '',
        duration: entry.duration || 0,
        status: 'waiting',
        error: null,
        errorCode: null,
        result: null,
        startedAt: null,
        completedAt: null,
        wordCount: null,
        downloadPercent: null,
        downloadedBytes: null,
        totalBytes: null,
        speedBps: null,
        etaSecs: null,
        streamSegments: [],
      })),
    };
    queueActive = true;
    activeView = 'queue';
    runQueueLoop();
  }

  /** @param {any} record */
  function selectHistoryRecord(record) {
    selectedHistoryRecord = record;
    result = { language: record.language, plain: record.plain, timestamped: record.timestamped, srt: record.srt };
    activeTab = 'plain';
  }

  /** @param {CustomEvent<string>} e */
  function handleSidebarNavigate(e) {
    const newView = e.detail;
    if (activeView === 'history' && newView !== 'history' && selectedHistoryRecord) {
      selectedHistoryRecord = null;
      result = null;
    }
    activeView = newView;
    if (newView === 'queue') {
      // QueueView is always mounted but was display:none. After display:flex is
      // applied (first frame), scrollHeight becomes accurate (second frame).
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const scroller = document.querySelector('.al-scroller');
          if (scroller) scroller.scrollTop = scroller.scrollHeight;
        });
      });
    }
  }

  /** @param {CustomEvent} e */
  function handlePlaylistDetected(e) {
    playlistProbeResult = e.detail;
    activeView = 'picker';
  }

  /** @param {any} item — JobItemRecord with transcript data */
  function openTranscriptFromJob(item) {
    if (!item?.plain) return;
    selectedHistoryRecord = { id: item.id, url: item.url, title: item.title, language: item.language };
    result = { language: item.language, plain: item.plain, timestamped: item.timestamped, srt: item.srt };
    activeTab = 'plain';
  }

  /** @param {string} jobId @param {string} itemId */
  async function retryFailedJobItem(jobId, itemId) {
    const job = jobs.find((j) => j.id === jobId);
    if (!job) return;
    const item = job.items.find((i) => i.id === itemId);
    if (!item) return;
    // Re-queue as a single-item batch job, reusing the prior record's
    // job_id and item_id so the finished job overwrites the existing
    // History entry instead of stacking a duplicate.
    const itemIdsByUrl = new Map();
    itemIdsByUrl.set(item.url, item.id);
    startBatchJob(
      [{
        id: item.id,
        url: item.url,
        title: item.title,
        thumbnail: item.thumbnail,
        duration: item.duration_secs || 0,
      }],
      { jobId: job.id, itemIdsByUrl },
    );
  }

  /** @param {string} jobId */
  async function handleDeleteJob(jobId) {
    if (!invokeFn) return;
    try {
      await invokeFn('delete_job', { job_id: jobId });
      jobs = jobs.filter((j) => j.id !== jobId);
    } catch (e) {
      console.warn('delete_job failed:', e);
    }
  }

  /** @param {string} id */
  async function deleteHistoryRecord(id) {
    if (!invokeFn) return;
    try {
      await invokeFn('delete_transcript', { id });
      historyRecords = historyRecords.filter((r) => r.id !== id);
      if (selectedHistoryRecord?.id === id) {
        selectedHistoryRecord = null;
        result = null;
      }
    } catch (e) {
      console.warn('delete_transcript failed:', e);
    }
  }

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
    console.log('[+page] transcribe() called, url=', JSON.stringify(url), 'isTauri=', isTauri, 'invokeFn=', typeof invokeFn);
    const trimmed = url.trim();
    if (!trimmed) {
      console.log('[+page] bail: empty url');
      return;
    }

    loading = true;
    error = null;
    result = null;
    streamSegments = [];
    isTyping = false;
    typingIdx = -1;
    if (typewriterTimer) clearTimeout(typewriterTimer);
    typewriterTimer = null;
    phase = 'idle';

    // Subscribe to sidecar progress events before invoking, so we don't
    // miss the first event in a fast network.
    let unlisten = null;
    if (invokeFn && listenFn) {
      try {
        unlisten = await listenFn('transcribe-progress', (event) => {
          handleSidecarEvent(event.payload);
          logActivity(event.payload);
        });
      } catch (e) {
        console.warn('Failed to subscribe to transcribe-progress:', e);
      }
    }

    try {
      if (isTauri && invokeFn) {
        // Desktop path: invoke the sidecar command. The Rust layer spawns
        // the sidecar process and forwards its stdout events back to us
        // via the `transcribe-progress` event we subscribed to above.
        // The Rust command resolves when the sidecar terminates.
        await invokeFn('run_sidecar', { url: trimmed, model, timestamps });
      } else {
        // Browser fallback (dev only): not supported in v1 desktop mode.
        error = 'This app must be run inside the Tauri desktop app.';
        return;
      }
    } catch (/** @type {any} */ e) {
      error = errorMessageFor(null, String(e?.message || e));
    } finally {
      if (unlisten) {
        try { unlisten(); } catch {}
      }
      loading = false;
    }
  }

  /**
   * Dispatch a single sidecar event payload to the right UI update.
   * @param {any} payload
   */
  function handleSidecarEvent(payload) {
    if (!payload || typeof payload !== 'object') return;
    const event = payload.event;
    if (event === 'phase') {
      if (payload.phase === 'downloading-model') {
        // Surface a stable "downloading-model" phase; the UI can render
        // payload.progress if it wants a progress bar.
        phase = 'downloading';
        modelDownloadProgress = typeof payload.progress === 'number' ? payload.progress : null;
      } else if (payload.phase === 'downloading') {
        phase = 'downloading';
      } else if (payload.phase === 'transcribing') {
        phase = 'transcribing';
      }
    } else if (event === 'progress') {
      // Segment-level streaming events (forwarded by the Rust bridge if the
      // sidecar emits them — see api/sidecar.py).
      if (payload.phase === 'transcribing' && payload.text) {
        enqueueSegment({
          index: payload.segment ?? streamSegments.length + 1,
          text: payload.text,
          start: payload.start ?? 0,
          end: payload.end ?? 0,
          ts: payload.ts,
        });
      }
    } else if (event === 'result') {
      if (typewriterTimer) clearTimeout(typewriterTimer);
      typewriterTimer = null;
      isTyping = false;
      typingIdx = -1;
      streamSegments = streamSegments.map(s => ({ ...s, displayed: s.text }));
      phase = 'done';
      result = {
        language: payload.language,
        plain: payload.plain,
        timestamped: payload.timestamped,
        srt: payload.srt,
      };
      activeTab = timestamps && result.timestamped ? 'timestamped' : 'plain';
      if (invokeFn) {
        const wordCount = getWordCount(result.plain);
        invokeFn('save_transcript', {
          record: {
            url,
            language: result.language,
            plain: result.plain,
            timestamped: result.timestamped,
            srt: result.srt,
            model,
            word_count: wordCount,
          },
        })
          .then((id) => {
            historyRecords = [
              {
                id,
                url,
                language: result.language,
                plain: result.plain,
                timestamped: result.timestamped,
                srt: result.srt,
                model,
                word_count: wordCount,
                created_at: new Date().toISOString(),
              },
              ...historyRecords,
            ];
          })
          .catch((e) => console.warn('save_transcript failed:', e));
      }
    } else if (event === 'error') {
      error = errorMessageFor(payload.code, payload.message);
    } else if (event === 'terminated') {
      // Sidecar process exited unexpectedly (killed by cancel or crash).
      // Only show an error if we weren't already done.
      if (phase !== 'done') {
        // Cancelled: silently return to idle.
        phase = 'idle';
      }
    } else if (event === 'done') {
      // Fallback in case the sidecar exits without an explicit `result` event.
      // (Usually result arrives before done.)
    }
  }

  /** @type {number | null} */
  let modelDownloadProgress = null;

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
  function getWordCount(text) {
    if (!text) return 0;
    return text.trim().split(/\s+/).filter(Boolean).length;
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
    // Bare Enter in the URL input → trigger transcription.
    if (e.key === 'Enter' && !loading) transcribe();
  }

  // ── Desktop actions: cancel, save ───────────────────────────────────────
  async function cancelTranscription() {
    if (!loading || !invokeFn) return;
    try {
      await invokeFn('cancel_transcribe');
    } catch (e) {
      console.warn('cancel_transcribe failed:', e);
    } finally {
      loading = false;
      phase = 'idle';
      modelDownloadProgress = null;
    }
  }

  async function saveTranscript() {
    const content = getActiveContent();
    if (!content) return;
    if (isTauri && dialogSave) {
      try {
        const ext = activeTab === 'srt' ? 'srt' : 'txt';
        const slug = (url || 'transcript')
          .replace(/^https?:\/\//, '')
          .replace(/[^a-zA-Z0-9]+/g, '-')
          .replace(/^-+|-+$/g, '')
          .slice(0, 60) || 'transcript';
        const path = await dialogSave({
          defaultPath: `${slug}.${ext}`,
          filters: [
            { name: ext === 'srt' ? 'SRT subtitle' : 'Text', extensions: [ext] }
          ],
        });
        if (!path) return;
        const fs = await import('@tauri-apps/plugin-fs');
        await fs.writeTextFile(path, content);
      } catch (e) {
        console.warn('save failed:', e);
        await copyToClipboard(content);
      }
    } else {
      await copyToClipboard(content);
    }
  }

  /** @param {number} i */
  function toggleFaq(i) {
    openFaq = openFaq === i ? null : i;
  }

  /** @param {{index: number, text: string, start: number, end: number, ts?: string}} seg */
  function enqueueSegment(seg) {
    streamSegments = [...streamSegments, { ...seg, displayed: '' }];
    if (!isTyping) startTypingAt(streamSegments.length - 1);
  }

  /** @param {number} idx */
  function startTypingAt(idx) {
    typingIdx = idx;
    isTyping = true;
    typeNextChar();
  }

  function typeNextChar() {
    if (!isTyping || typingIdx < 0 || typingIdx >= streamSegments.length) return;
    const seg = streamSegments[typingIdx];
    if (seg.displayed.length >= seg.text.length) {
      // Segment fully typed — advance to next queued segment
      const next = typingIdx + 1;
      if (next < streamSegments.length) {
        startTypingAt(next);
      } else {
        isTyping = false;
        typingIdx = -1;
      }
      return;
    }
    streamSegments[typingIdx] = { ...seg, displayed: seg.text.slice(0, seg.displayed.length + 1) };
    streamSegments = streamSegments; // trigger Svelte reactivity
    typewriterTimer = setTimeout(typeNextChar, CHAR_DELAY);
  }
</script>

<DesktopTitleBar {phase} />

<KeyboardShortcuts
  on:transcribe={() => url.trim() && !queueActive && handleTranscribeViaQueue()}
  on:cancel={cancelTranscription}
  on:save={saveTranscript}
/>

<div class="app" class:dark={darkMode} class:desktop={isTauri}>

  {#if isTauri}
  <!-- ─── Desktop app shell ─────────────────────────── -->
  <div class="desktop-shell">
    <SidebarNav {activeView} {queueActive} {darkMode} on:navigate={handleSidebarNavigate} on:toggle-theme={handleSidebarThemeToggle} />
    <div class="desktop-main">
      <div class="page-title">
        {#if activeView === 'transcribe' || activeView === 'picker'}
          New transcription
        {:else if activeView === 'queue'}
          Queue
        {:else if activeView === 'history'}
          History
        {:else}
          Settings
        {/if}
      </div>
      <main class="desktop-content">
        <!-- Queue: always mounted in the DOM so WKWebView never pays the expensive
             mount/unmount cost when clicking the Queue tab. Visibility is controlled
             by display:none — a pure CSS change with no JS overhead. -->
        <div class="full-pane" style:display={activeView === 'queue' ? 'flex' : 'none'}>
          <QueueView
            items={currentJob?.items ?? []}
            {timestamps}
            activityEntries={activityLog}
            {activityHandlers}
            on:cancelJob={cancelQueueJob}
            on:retryItem={(e) => retryQueueItem(e.detail.id)}
            on:cancelItem={(e) => cancelQueueItem(e.detail.id)}
            on:viewHistory={() => (activeView = 'history')}
          />
        </div>
        {#if activeView === 'transcribe' || activeView === 'picker'}
          <aside class="left-pane">
            <UrlInputPanel
              bind:url
              bind:timestamps
              bind:probeState
              bind:probeResult
              bind:probeError
              {loading}
              {phase}
              {model}
              {invokeFn}
              modelProgress={modelDownloadProgress}
              language={result?.language}
              errorMessage={error}
              {activeView}
              pickerMode={activeView === 'picker'}
              selectedCount={pickerSelectedCount}
              {duplicateMatch}
              on:transcribe={handleTranscribeViaQueue}
              on:cancel={cancelTranscription}
              on:playlist={handlePlaylistDetected}
              on:goSettings={() => (activeView = 'settings')}
              on:transcribePicker={handleTranscribePicker}
              on:viewDuplicate={viewDuplicate}
              on:forceTranscribe={forceTranscribe}
            />
          </aside>
          <section class="right-pane">
            {#if activeView === 'picker' && playlistProbeResult}
              <VideoPicker
                entries={playlistProbeResult.entries || []}
                playlistTitle={playlistProbeResult.title || ''}
                uploader={playlistProbeResult.uploader || ''}
                on:selectionChange={(e) => (pickerSelectedCount = e.detail.count)}
                on:startJob={handleStartJob}
              />
            {:else}
              <TranscriptPanel
                {result}
                bind:activeTab
                defaultName={url}
                {streamSegments}
                {phase}
                {timestamps}
                onTabChange={(t) => (activeTab = t)}
                onCopy={() => copyToClipboard(getActiveContent())}
              />
            {/if}
          </section>
        {:else if activeView === 'history'}
          <!-- History: full-width list by default; right-pane transcript slides in when an item is opened. -->
          <div class="full-pane" class:with-detail={!!selectedHistoryRecord}>
            <div class="history-list">
              <JobHistoryView
                {jobs}
                on:openTranscript={(e) => openTranscriptFromJob(e.detail.item)}
                on:retryFailed={(e) => retryFailedJobItem(e.detail.jobId, e.detail.itemId)}
                on:deleteJob={(e) => handleDeleteJob(e.detail.jobId)}
              />
            </div>
            {#if selectedHistoryRecord}
              <section class="history-detail">
                <button
                  type="button"
                  class="history-detail-close"
                  on:click={() => { selectedHistoryRecord = null; result = null; }}
                  aria-label="Close transcript"
                >×</button>
                <TranscriptPanel
                  {result}
                  bind:activeTab
                  defaultName={selectedHistoryRecord.title ?? selectedHistoryRecord.url ?? ''}
                  onTabChange={(t) => (activeTab = t)}
                  onCopy={() => copyToClipboard(getActiveContent())}
                />
              </section>
            {/if}
          </div>
        {:else if activeView === 'settings'}
          <!-- Settings: full-width single pane. -->
          <div class="full-pane">
            <SettingsView
              bind:model
              bind:timestamps
              bind:darkMode
              historyCount={historyRecords.length}
              historySizeBytes={historyRecords.reduce((sum, r) => sum + (r.plain?.length ?? 0) + (r.srt?.length ?? 0) + (r.timestamped?.length ?? 0), 0)}
              on:change={handleSettingsChange}
              on:clearHistory={handleClearHistory}
            />
          </div>
        {/if}
      </main>
    </div>
  </div>
  {:else}
  <!-- ─── Web app layout (marketing site) ──────────── -->

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

      {#if !isTauri}
      <!-- Hero -->
      <header>
        <h1>
          Turn any video into <span class="italic">text</span>,<br/>
          <span class="serif-italic">beautifully.</span>
        </h1>
        <p class="tagline">Paste a YouTube, Instagram, or TikTok URL and get a polished transcript — with timestamps and SRT subtitles — in seconds.</p>
      </header>
      {:else}
      <!-- Desktop app header — minimal, focused. -->
      <header class="desktop-header">
        <div class="desktop-header-row">
          <div class="desktop-logo">
            <div class="logo-mark"></div>
            <span class="logo-text">Transcribe</span>
          </div>
          <div class="desktop-meta">
            <span class="status-pill">
              <span class="dot"></span>
              Local · Whisper tiny
            </span>
          </div>
        </div>
      </header>
      {/if}

      <!-- Transcribe Card -->
      <div class="glass card transcribe-card">
        <div class="field">
          <label for="url-input">Video URL</label>
          <div class="input-row">
            <div class="input-wrapper">
              <svg class="input-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M6.5 1.5h-3A2 2 0 001.5 3.5v9A2 2 0 003.5 14.5h9A2 2 0 0014.5 12.5v-3M14.5 1.5h-4m4 0v4m0-4L8 8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <UrlDropZone
                bind:value={url}
                placeholder="Paste URL or drag-and-drop…"
                disabled={loading}
              />
            </div>
            {#if loading}
              <button
                class="btn-stop"
                on:click={cancelTranscription}
                title="Cancel (Cmd+.)"
                aria-label="Cancel transcription"
              >
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                  <rect x="2" y="2" width="8" height="8" rx="1.5" fill="currentColor"/>
                </svg>
                <span>Stop</span>
              </button>
            {:else}
              <button
                class="btn-primary"
                on:click={transcribe}
                disabled={!url.trim()}
                title="Transcribe (Cmd+Enter)"
              >
                <span>Transcribe</span>
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="M3 8L13 8M13 8L9 4M13 8L9 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>
            {/if}
          </div>
          {#if isTauri}
            <div class="desktop-hint">
              <span class="dot"></span>
              <span>Desktop mode · drag a URL · <kbd>⌘</kbd><kbd>↩</kbd> transcribe · <kbd>⌘</kbd><kbd>.</kbd> stop · <kbd>⌘</kbd><kbd>S</kbd> save</span>
            </div>
          {/if}
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
                Fetching your audio…
              {:else if phase === 'transcribing'}
                {#if streamSegments.length > 0}
                  Turning speech into text · {streamSegments.length} segment{streamSegments.length === 1 ? '' : 's'}
                {:else}
                  Turning speech into text…
                {/if}
              {:else}
                Getting things ready…
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
      {#if streamSegments.length > 0 || phase === 'done'}
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
                <SaveTranscriptButton
                  content={getActiveContent()}
                  defaultName={url}
                  format={activeTab}
                />
              {/if}
            </div>
          </div>
          <div class="result-body">
            {#if phase === 'done' && result && activeTab !== 'plain'}
              <pre class="transcript">{getActiveContent()}</pre>
            {:else if streamSegments.length === 0}
              {#if phase === 'done'}
                <p class="no-speech">No speech detected in this audio.</p>
              {/if}
            {:else}
              <div class="stream-transcript">
                {#each streamSegments as seg (seg.index)}
                  <p class="stream-segment">
                    {#if phase !== 'done' && timestamps && seg.ts}<span class="seg-ts">[{seg.ts}]</span> {/if}{seg.displayed}{#if seg.displayed.length < seg.text.length}<span class="cursor-blink" aria-hidden="true"></span>{/if}
                  </p>
                {/each}
                {#if !isTyping && loading && phase === 'transcribing'}
                  <span class="cursor-blink" aria-hidden="true"></span>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}

      {#if !isTauri}
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
      {/if}

    </div>
  </main>

  {#if !isTauri}
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
  {/if}
  {/if}

</div>

<style>
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url('$lib/fonts/inter-400.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 450;
    font-display: swap;
    src: url('$lib/fonts/inter-400.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 500;
    font-display: swap;
    src: url('$lib/fonts/inter-500.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 600;
    font-display: swap;
    src: url('$lib/fonts/inter-600.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 700;
    font-display: swap;
    src: url('$lib/fonts/inter-700.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Fraunces';
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url('$lib/fonts/fraunces-400.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Fraunces';
    font-style: italic;
    font-weight: 400;
    font-display: swap;
    src: url('$lib/fonts/fraunces-400-italic.woff2') format('woff2');
  }
  @font-face {
    font-family: 'Fraunces';
    font-style: normal;
    font-weight: 500;
    font-display: swap;
    src: url('$lib/fonts/fraunces-500.woff2') format('woff2');
  }

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

    --surface-1: #ffffff;
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

    --surface-1: #1c1815;
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

  /* ─── Desktop-mode layout overrides ───────────────── */
  .app.desktop {
    background: var(--bg-base);
  }

  .desktop-shell {
    display: flex;
    min-height: 100vh;
    width: 100%;
  }
  .desktop-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .page-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-3);
    padding: 12px 20px 0;
    flex-shrink: 0;
  }

  .desktop-content {
    flex: 1;
    display: flex;
    gap: 14px;
    padding: 14px;
    min-height: 0;
    overflow: hidden;
  }
  .full-pane {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border-radius: 12px;
    border: 1px solid var(--glass-border-soft);
    overflow: hidden;
  }
  .full-pane.with-detail {
    flex-direction: row;
    gap: 0;
  }
  .history-list {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: flex 0.2s ease;
  }
  .full-pane.with-detail .history-list {
    flex: 1 1 50%;
  }
  .history-detail {
    flex: 1 1 50%;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--glass-border-soft);
    position: relative;
    overflow: hidden;
  }
  .history-detail-close {
    position: absolute;
    top: 10px;
    right: 12px;
    z-index: 2;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
    display: grid;
    place-items: center;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .history-detail-close:hover {
    background: var(--surface-2);
    color: var(--text);
    border-color: var(--glass-border-soft);
  }
  .left-pane {
    width: 380px;
    flex-shrink: 0;
    background: var(--surface-1);
    border-radius: 12px;
    border: 1px solid var(--glass-border-soft);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-height: 0;
  }
  .right-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    min-height: 0;
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
    /* Input lives inside <UrlDropZone>; styling is in that component. */
  }

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

  .btn-stop {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: transparent;
    color: var(--text);
    border: 1.5px solid var(--text-2);
    border-radius: var(--radius-sm);
    font-family: inherit;
    font-size: 15px;
    font-weight: 500;
    padding: 11.5px 22px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.2s, color 0.2s, border-color 0.2s;
  }
  .btn-stop:hover {
    background: var(--text);
    color: var(--surface);
    border-color: var(--text);
  }

  /* ─── Desktop header ───────────────────────────────── */
  .desktop-header {
    padding: 4px 0 18px;
    border-bottom: 1px solid var(--glass-border-soft);
    margin-bottom: 22px;
  }
  .desktop-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .desktop-logo {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-weight: 600;
    font-size: 16px;
    color: var(--text);
  }
  .desktop-meta {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 999px;
    padding: 4px 10px 4px 8px;
  }
  .status-pill .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.6);
  }

  /* ─── Desktop-mode hint ────────────────────────────── */
  .desktop-hint {
    margin-top: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-2);
    opacity: 0.85;
  }
  .desktop-hint .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.6);
  }
  .desktop-hint kbd {
    display: inline-block;
    padding: 1px 5px;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 3px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    color: var(--text);
  }

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
