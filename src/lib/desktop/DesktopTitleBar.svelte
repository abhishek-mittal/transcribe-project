<script>
  /**
   * DesktopTitleBar — updates the native window title based on the current
   * transcription phase. Falls back to a no-op outside Tauri.
   */
  import { isTauri, loadTauriCore } from './tauri';

  export let phase = 'idle';

  const TITLES = {
    idle: 'Transcribe',
    'downloading-model': 'Transcribe — Downloading model…',
    downloading: 'Transcribe — Downloading audio…',
    transcribing: 'Transcribe — Transcribing…',
    done: 'Transcribe — Done',
  };

  $: title = TITLES[phase] ?? 'Transcribe';

  let currentTitle = '';

  $: if (typeof document !== 'undefined' && title !== currentTitle) {
    currentTitle = title;
    document.title = title;
    if (isTauri()) {
      import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
        getCurrentWindow().setTitle(title).catch(() => {});
      }).catch(() => {});
    }
  }
</script>

<!-- Renderless: just syncs phase → window.title. -->