<script>
  /**
   * UrlDropZone wraps a URL input with HTML5 drag-and-drop support.
   * On a valid http(s) URL drop, populates the bound `value` (parent's `url`).
   * Rejects non-URL text with a brief shake animation.
   */
  export let value = '';
  export let placeholder = 'Paste or drop a video URL…';
  export let disabled = false;

  let shaking = false;
  let dragOver = false;

  function isValidUrl(text) {
    const t = text.trim();
    return /^https?:\/\/[^\s]+$/i.test(t);
  }

  /** Extract the first http(s) URL from arbitrary drag payload text. */
  function extractUrl(text) {
    const match = text.match(/https?:\/\/[^\s]+/i);
    return match ? match[0] : null;
  }

  /** @param {DragEvent} e */
  function handleDragOver(e) {
    if (disabled) return;
    e.preventDefault();
    dragOver = true;
  }

  /** @param {DragEvent} e */
  function handleDragLeave() {
    dragOver = false;
  }

  /** @param {DragEvent} e */
  function handleDrop(e) {
    if (disabled) return;
    e.preventDefault();
    dragOver = false;
    const text = e.dataTransfer?.getData('text/plain') ?? '';
    const candidate = extractUrl(text) ?? text.trim();
    if (isValidUrl(candidate)) {
      value = candidate;
    } else {
      triggerShake();
    }
  }

  function triggerShake() {
    shaking = true;
    setTimeout(() => (shaking = false), 400);
  }

  /** @param {KeyboardEvent} e */
  function handleKeydown(e) {
    if (e.key === 'Enter' && !disabled) {
      // Let the parent component decide what to do with the value.
      e.target?.dispatchEvent(new CustomEvent('submit', { bubbles: true }));
    }
  }
</script>

<div
  class="drop-zone"
  class:drag-over={dragOver}
  class:shaking
  class:disabled
  on:dragover={handleDragOver}
  on:dragleave={handleDragLeave}
  on:drop={handleDrop}
  role="region"
  aria-label="URL input with drag-and-drop support"
>
  <input
    type="url"
    bind:value
    on:keydown={handleKeydown}
    {placeholder}
    {disabled}
    autocomplete="off"
    spellcheck="false"
  />
  {#if dragOver}
    <div class="overlay">Drop the URL here</div>
  {/if}
</div>

<style>
  .drop-zone {
    position: relative;
    width: 100%;
    border-radius: 8px;
    transition: transform 120ms ease;
  }
  .drop-zone input {
    width: 100%;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-family: inherit;
    font-size: 15px;
    padding: 13px 14px;
    outline: none;
    transition: border-color 0.2s, background 0.2s, box-shadow 0.2s;
  }
  .drop-zone input:focus {
    border-color: var(--text-3);
    background: var(--glass-bg-strong);
    box-shadow: 0 0 0 4px rgba(28, 28, 50, 0.05);
  }
  .drop-zone input::placeholder { color: var(--text-3); }
  .drop-zone input:disabled { opacity: 0.5; cursor: not-allowed; }
  .drop-zone.drag-over input {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px rgba(42, 37, 32, 0.1);
  }
  .drop-zone.shaking {
    animation: shake 350ms ease-in-out;
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    font-weight: 500;
    opacity: 0.8;
    color: var(--accent);
  }
  @keyframes shake {
    0%, 100% { transform: translateX(0); }
    20%, 60% { transform: translateX(-6px); }
    40%, 80% { transform: translateX(6px); }
  }
</style>