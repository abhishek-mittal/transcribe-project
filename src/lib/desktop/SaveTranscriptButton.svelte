<script>
  /**
   * SaveTranscriptButton — opens the native save dialog (Tauri) or falls
   * back to clipboard copy when running outside Tauri.
   */
  import { loadTauriDialog, loadTauriFs, isTauri } from './tauri';

  export let content = '';
  export let defaultName = 'transcript';
  /** 'plain' | 'timestamped' | 'srt' — controls file extension */
  export let format = 'plain';

  let saving = false;
  let lastResult = '';

  function defaultFilename() {
    const ext = format === 'srt' ? 'srt' : 'txt';
    const base = (defaultName || 'transcript')
      .replace(/^https?:\/\//, '')
      .replace(/[^a-zA-Z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'transcript';
    return `${base}.${ext}`;
  }

  async function handleClick() {
    if (!content || saving) return;
    saving = true;
    try {
      if (isTauri()) {
        const dialog = await loadTauriDialog();
        const fs = await loadTauriFs();
        if (dialog && fs) {
          const path = await dialog.save({
            defaultPath: defaultFilename(),
            filters: [
              {
                name: format === 'srt' ? 'SRT subtitle' : 'Text',
                extensions: [format === 'srt' ? 'srt' : 'txt'],
              },
            ],
          });
          if (path) {
            await fs.writeTextFile(path, content);
            lastResult = `Saved to ${path}`;
            setTimeout(() => (lastResult = ''), 3000);
            return;
          }
        }
      }
      // Fallback: copy to clipboard.
      await navigator.clipboard.writeText(content);
      lastResult = 'Copied to clipboard';
      setTimeout(() => (lastResult = ''), 3000);
    } catch (e) {
      console.warn('save failed:', e);
      lastResult = 'Save failed';
      setTimeout(() => (lastResult = ''), 3000);
    } finally {
      saving = false;
    }
  }
</script>

<button
  type="button"
  class="save-btn"
  on:click={handleClick}
  disabled={!content || saving}
  aria-label="Save transcript"
>
  {#if saving}
    Saving…
  {:else}
    Save
  {/if}
</button>
{#if lastResult}
  <span class="hint">{lastResult}</span>
{/if}

<style>
  .save-btn {
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
  .save-btn:hover:not(:disabled) {
    color: var(--text);
    background: var(--glass-bg-strong);
  }
  .save-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .hint {
    margin-left: 0.5rem;
    font-size: 0.85em;
    opacity: 0.7;
  }
</style>