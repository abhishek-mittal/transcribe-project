<script>
  import { createEventDispatcher } from 'svelte';
  export let activeView = 'transcribe';
  export let recentCount = 0;
  export let queueActive = false;
  export let darkMode = false;

  const dispatch = createEventDispatcher();

  const items = [
    { id: 'transcribe', label: 'Transcribe', icon: 'transcribe' },
    { id: 'queue', label: 'Queue', icon: 'queue' },
    { id: 'history', label: 'History', icon: 'history' },
    { id: 'settings', label: 'Settings', icon: 'settings' },
  ];

  function handleClick(id) {
    dispatch('navigate', id);
  }

  function handleThemeToggle() {
    dispatch('toggle-theme');
  }
</script>

<aside class="sidebar-nav">
  <div class="brand">
    <div class="brand-mark">
      <svg width="18" height="18" viewBox="0 0 22 22" fill="none" aria-hidden="true">
        <path d="M5 9V13M8 7V15M11 5V17M14 8V14M17 10V12" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
      </svg>
    </div>
    <div class="brand-text">
      <span class="brand-name">Transcribe</span>
      <span class="brand-tag">Local · Whisper</span>
    </div>
  </div>

  <nav class="nav-items">
    {#each items as item}
      <button
        type="button"
        class="nav-item"
        class:active={activeView === item.id}
        on:click={() => handleClick(item.id)}
      >
        <span class="nav-icon">
          {#if item.icon === 'transcribe'}
            <svg width="16" height="16" viewBox="0 0 22 22" fill="none" aria-hidden="true">
              <path d="M5 9V13M8 7V15M11 5V17M14 8V14M17 10V12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
            </svg>
          {:else if item.icon === 'queue'}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M3 6h18M3 12h18M3 18h18" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
            </svg>
          {:else if item.icon === 'history'}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M3 12a9 9 0 109-9 9.75 9.75 0 00-6.74 2.74L3 8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M3 3v5h5M12 7v5l3 2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {:else if item.icon === 'settings'}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.6"/>
              <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 11-4 0v-.09A1.65 1.65 0 008 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 11-2.83-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 110-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 114 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 110 4h-.09a1.65 1.65 0 00-1.51 1z" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {/if}
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.id === 'queue' && queueActive}
          <span class="queue-dot" aria-label="Queue active"></span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="nav-footer">
    <button class="theme-row" type="button" on:click={handleThemeToggle} aria-label="Toggle theme">
      <span class="theme-icon" aria-hidden="true">
        {#if darkMode}
          <svg width="14" height="14" viewBox="0 0 18 18" fill="none">
            <circle cx="9" cy="9" r="3.5" stroke="currentColor" stroke-width="1.5"/>
            <path d="M9 1.5V3M9 15V16.5M1.5 9H3M15 9H16.5M3.7 3.7L4.75 4.75M13.25 13.25L14.3 14.3M14.3 3.7L13.25 4.75M4.75 13.25L3.7 14.3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        {:else}
          <svg width="14" height="14" viewBox="0 0 18 18" fill="none">
            <path d="M15.5 10.5A7 7 0 017.5 2.5a7 7 0 000 13 7 7 0 008-5z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        {/if}
      </span>
      <span class="theme-label">{darkMode ? 'Dark' : 'Light'}</span>
    </button>
    <div class="nav-footer-hint">
      <kbd>⌘</kbd><kbd>↩</kbd> Transcribe
    </div>
    <div class="nav-footer-hint">
      <kbd>⌘</kbd><kbd>.</kbd> Cancel
    </div>
    <div class="nav-footer-hint">
      <kbd>⌘</kbd><kbd>S</kbd> Save
    </div>
  </div>
</aside>

<style>
  .sidebar-nav {
    width: 220px;
    flex-shrink: 0;
    background: var(--surface-1);
    border-right: 1px solid var(--glass-border-soft);
    display: flex;
    flex-direction: column;
    padding: 18px 12px;
    height: 100%;
    min-height: 0;
    box-sizing: border-box;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 8px 22px;
    border-bottom: 1px solid var(--glass-border-soft);
    margin-bottom: 14px;
  }
  .brand-mark {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--accent-fg);
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }
  .brand-text {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
    min-width: 0;
  }
  .brand-name {
    font-weight: 600;
    font-size: 14px;
    color: var(--text);
  }
  .brand-tag {
    font-size: 10.5px;
    color: var(--text-3);
    font-weight: 500;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: var(--text-2);
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s, color 0.15s;
  }
  .nav-item:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav-item.active {
    background: var(--accent);
    color: var(--accent-fg);
  }
  .nav-item.active .nav-icon svg { stroke: var(--accent-fg); }
  .nav-icon {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }
  .nav-icon svg { stroke: currentColor; }
  .nav-label { flex: 1; }

  .queue-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #f59e0b;
    box-shadow: 0 0 6px rgba(245, 158, 11, 0.7);
    flex-shrink: 0;
    animation: pulse-dot 1.6s ease-in-out infinite;
  }
  .nav-item.active .queue-dot {
    background: var(--accent-fg);
    box-shadow: 0 0 6px rgba(255, 255, 255, 0.5);
  }
  @keyframes pulse-dot {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.6; transform: scale(0.8); }
  }

  .nav-footer {
    border-top: 1px solid var(--glass-border-soft);
    padding-top: 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .theme-row {
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    padding: 6px 8px;
    border-radius: 6px;
    color: var(--text-2);
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    margin-bottom: 4px;
    transition: background 0.15s, color 0.15s;
  }
  .theme-row:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .theme-icon {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }
  .theme-label { flex: 1; }
  .nav-footer-hint {
    font-size: 11px;
    color: var(--text-3);
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .nav-footer-hint kbd {
    display: inline-block;
    padding: 1px 5px;
    background: var(--surface-2);
    border: 1px solid var(--glass-border-soft);
    border-radius: 3px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 10px;
    color: var(--text-2);
  }
</style>
