<script>
  /**
   * KeyboardShortcuts — global keyboard handler for the desktop app.
   *
   * Shortcuts (Cmd on macOS, Ctrl elsewhere):
   *   Cmd+Enter — trigger transcription if URL is non-empty and idle
   *   Cmd+.     — cancel in-flight transcription
   *   Cmd+S     — save transcript (only when a result is available)
   *
   * Importing this component and mounting it (svelte component with side
   * effects in onMount) is enough to activate the shortcuts.
   */
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher();

  function isMac() {
    return typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
  }

  /** @param {KeyboardEvent} e */
  function handle(e) {
    const mod = isMac() ? e.metaKey : e.ctrlKey;
    if (!mod) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      dispatch('transcribe');
    } else if (e.key === '.') {
      e.preventDefault();
      dispatch('cancel');
    } else if (e.key === 's' || e.key === 'S') {
      e.preventDefault();
      dispatch('save');
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handle);
  });

  onDestroy(() => {
    if (typeof window !== 'undefined') {
      window.removeEventListener('keydown', handle);
    }
  });
</script>

<!-- This component renders nothing; it just attaches a global keydown listener. -->