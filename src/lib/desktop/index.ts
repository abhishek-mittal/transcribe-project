// Re-export public API for the desktop UI layer.
export { default as DesktopTitleBar } from './DesktopTitleBar.svelte';
export { default as UrlDropZone } from './UrlDropZone.svelte';
export { default as SaveTranscriptButton } from './SaveTranscriptButton.svelte';
export { default as KeyboardShortcuts } from './KeyboardShortcuts.svelte';
export { default as SidebarNav } from './SidebarNav.svelte';
export { default as UrlInputPanel } from './UrlInputPanel.svelte';
export { default as TranscriptPanel } from './TranscriptPanel.svelte';
export { default as ActivityLogPanel } from './ActivityLogPanel.svelte';
export {
  isTauri,
  loadTauriCore,
  loadTauriEvent,
  loadTauriDialog,
  loadTauriFs,
} from './tauri';
export { errorMessageFor } from './errors';
export type { ErrorCode } from './errors';