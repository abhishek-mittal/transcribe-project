// Tauri environment detection helpers.
// True when running inside the Tauri 2.x webview.

export function isTauri(): boolean {
  return typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
}

/** Lazily load the Tauri core API; returns null if not in Tauri. */
export async function loadTauriCore(): Promise<typeof import('@tauri-apps/api/core') | null> {
  if (!isTauri()) return null;
  try {
    return await import('@tauri-apps/api/core');
  } catch {
    return null;
  }
}

/** Lazily load the Tauri event API; returns null if not in Tauri. */
export async function loadTauriEvent(): Promise<typeof import('@tauri-apps/api/event') | null> {
  if (!isTauri()) return null;
  try {
    return await import('@tauri-apps/api/event');
  } catch {
    return null;
  }
}

/** Lazily load the Tauri dialog plugin; returns null if not in Tauri. */
export async function loadTauriDialog(): Promise<typeof import('@tauri-apps/plugin-dialog') | null> {
  if (!isTauri()) return null;
  try {
    return await import('@tauri-apps/plugin-dialog');
  } catch {
    return null;
  }
}

/** Lazily load the Tauri fs plugin; returns null if not in Tauri. */
export async function loadTauriFs(): Promise<typeof import('@tauri-apps/plugin-fs') | null> {
  if (!isTauri()) return null;
  try {
    return await import('@tauri-apps/plugin-fs');
  } catch {
    return null;
  }
}