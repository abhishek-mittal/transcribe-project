// Sidecar error code → user-facing message mapping.
// See openspec/changes/tauri-desktop-app/specs/python-sidecar/spec.md for codes.

export type ErrorCode =
  | 'INVALID_URL'
  | 'NETWORK'
  | 'BOT_CHALLENGE'
  | 'UNSUPPORTED_PLATFORM'
  | 'FFMPEG_MISSING'
  | 'MODEL_LOAD_FAILED'
  | 'INTERNAL'
  | string;

export function errorMessageFor(code: string | null | undefined, fallback?: string): string {
  switch (code) {
    case 'INVALID_URL':
      return 'Please enter a valid http(s) URL.';
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
    default:
      return fallback || 'Something went wrong. Please try again.';
  }
}