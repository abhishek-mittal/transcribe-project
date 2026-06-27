"""Tauri desktop sidecar for the transcribe-project.

Reads a transcription request from CLI arguments (since the Tauri 2 shell
plugin doesn't pipe stdin), downloads audio via yt-dlp, transcribes via
faster-whisper, and writes newline-delimited JSON events to stdout. See
``openspec/changes/tauri-desktop-app/design.md`` (D3, D4, D5, D6) for the
protocol and error code definitions.

Run standalone for testing::

    python -m api.sidecar --url 'https://www.youtube.com/watch?v=...' \\
        --model tiny --timestamps true

Or via stdin (legacy/dev mode)::

    echo '{"url":"...","model":"tiny","timestamps":true}' \\
        | python -m api.sidecar

Signals:
    SIGTERM — clean up temp directory, exit with status 130.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import signal
import sys
import tempfile
import threading
import time
import traceback
from typing import Any

import yt_dlp

from api.transcribe_core import (
    DEFAULT_MODEL,
    VALID_MODELS,
    _get_model,
    _is_youtube_search_results_url,
    _search_query_from_url,
    download_audio,
    transcribe_audio,
)
from huggingface_hub import snapshot_download


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
SUPPORTED_MODELS_V1 = {"tiny", "base", "small"}
MAX_URL_LENGTH = 2048
HF_PROGRESS_INTERVAL_S = 0.5

# yt-dlp error messages to classify as bot challenges.
_BOT_CHALLENGE_PATTERNS = [
    re.compile(p, re.IGNORECASE)
    for p in (
        r"sign in to confirm you're not a bot",
        r"sign in to confirm you.re not a bot",
        r"confirm you.re not a bot",
        r"not a bot",
        r"http error 429",
    )
]

# yt-dlp error messages to classify as unsupported platform.
_UNSUPPORTED_PATTERNS = [
    re.compile(p, re.IGNORECASE)
    for p in (
        r"no extractor",
        r"unsupported url",
        r"no suitable extractor",
    )
]


# ---------------------------------------------------------------------------
# Event emission
# ---------------------------------------------------------------------------
def emit(event: dict[str, Any]) -> None:
    """Write one JSON event line to stdout and flush immediately."""
    sys.stdout.write(json.dumps(event, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def emit_phase(phase: str, **extra: Any) -> None:
    payload: dict[str, Any] = {"event": "phase", "phase": phase}
    payload.update(extra)
    emit(payload)


def emit_error(code: str, message: str) -> None:
    emit({"event": "error", "code": code, "message": message})


def emit_result(result: dict[str, Any]) -> None:
    emit({"event": "result", **result})


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------
class ValidationError(Exception):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code
        self.message = message


def validate_request(data: dict[str, Any]) -> tuple[str, str, bool]:
    """Validate and return (url, model, use_timestamps). Raise ValidationError on failure."""
    url = (data.get("url") or "").strip()
    if not url:
        raise ValidationError("INVALID_URL", "url is required")
    if len(url) > MAX_URL_LENGTH:
        raise ValidationError(
            "INVALID_URL", f"url is too long (max {MAX_URL_LENGTH} chars)"
        )
    if not url.startswith(("http://", "https://")):
        raise ValidationError(
            "INVALID_URL", "url must start with http:// or https://"
        )

    model = (data.get("model") or DEFAULT_MODEL).strip()
    if model not in SUPPORTED_MODELS_V1:
        raise ValidationError(
            "INVALID_MODEL",
            f"only {sorted(SUPPORTED_MODELS_V1)} models are supported; got '{model}'",
        )
    if model not in VALID_MODELS:
        raise ValidationError(
            "INVALID_MODEL", f"unknown model: {model}"
        )

    use_timestamps = bool(data.get("timestamps", True))
    return url, model, use_timestamps


# ---------------------------------------------------------------------------
# FFmpeg detection
# ---------------------------------------------------------------------------
def check_ffmpeg() -> None:
    if shutil.which("ffmpeg") is None:
        raise ValidationError(
            "FFMPEG_MISSING",
            "FFmpeg is required. Install with `brew install ffmpeg` (macOS) or "
            "see https://ffmpeg.org/download.html",
        )


# ---------------------------------------------------------------------------
# Model pre-download with progress
# ---------------------------------------------------------------------------
class _ProgressReporter:
    """Thread that periodically emits the latest download progress.

    snapshot_download exposes progress via a callback; we coalesce to avoid
    spamming the IPC channel (one event every HF_PROGRESS_INTERVAL_S).
    """

    def __init__(self) -> None:
        self._latest: float = 0.0
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def update(self, progress: float) -> None:
        with self._lock:
            self._latest = max(0.0, min(1.0, progress))

    def _run(self) -> None:
        while not self._stop.wait(HF_PROGRESS_INTERVAL_S):
            with self._lock:
                progress = self._latest
            emit_phase("downloading-model", progress=round(progress, 3))

    def start(self) -> None:
        self._thread = threading.Thread(
            target=self._run, name="hf-progress", daemon=True
        )
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()


def _hf_progress_callback(reporter: _ProgressReporter):  # pragma: no cover
    """Returns a huggingface_hub-compatible progress callback."""

    def cb(progress: float, *args: Any, **kwargs: Any) -> None:
        # The exact signature varies across huggingface_hub versions; the
        # first positional is the progress fraction (0.0-1.0).
        reporter.update(float(progress))

    return cb


def ensure_model_downloaded(model_size: str) -> None:
    """Pre-download the model via huggingface_hub with progress events.

    The first call downloads to the user's HF cache (~75 MB for ``tiny``);
    subsequent calls find the cached snapshot and return immediately.
    """
    from api.transcribe_core import _resolve_model_source

    source = _resolve_model_source(model_size)
    if source != model_size:
        # Local pre-bundled model present — no download needed.
        return

    reporter = _ProgressReporter()
    reporter.start()
    try:
        # snapshot_download returns the local path; pass allow_patterns to
        # avoid downloading tokenizer/config we don't need for inference.
        snapshot_download(
            repo_id=f"Systran/faster-whisper-{model_size}",
            allow_patterns=["*.json", "*.bin", "*.txt"],
            tqdm_class=None,
            etag_timeout=10,
        )
        emit_phase("downloading-model", progress=1.0)
    except Exception as e:
        raise ValidationError(
            "MODEL_LOAD_FAILED", f"failed to download model: {e}"
        ) from e
    finally:
        reporter.stop()


# ---------------------------------------------------------------------------
# yt-dlp error classification
# ---------------------------------------------------------------------------
def classify_ydl_error(exc: Exception) -> str:
    msg = str(exc)
    for pat in _BOT_CHALLENGE_PATTERNS:
        if pat.search(msg):
            return "BOT_CHALLENGE"
    for pat in _UNSUPPORTED_PATTERNS:
        if pat.search(msg):
            return "UNSUPPORTED_PLATFORM"
    # Network-ish errors
    if any(
        tok in msg.lower()
        for tok in (
            "name resolution",
            "temporary failure in name resolution",
            "connection refused",
            "connection reset",
            "network is unreachable",
        )
    ):
        return "NETWORK"
    return "INTERNAL"


# ---------------------------------------------------------------------------
# Cancellation
# ---------------------------------------------------------------------------
class Cancelled(Exception):
    """Raised when the sidecar receives SIGTERM mid-transcription."""


_cancel_requested = threading.Event()


def _install_signal_handlers() -> None:
    """Install SIGTERM handler that sets the cancel flag and cleans up."""
    cleanup_state = {"tmp_dir": None}

    def handle(signum: int, frame: Any) -> None:  # pragma: no cover
        _cancel_requested.set()
        tmp = cleanup_state.get("tmp_dir")
        if tmp is not None:
            shutil.rmtree(tmp, ignore_errors=True)
        # Exit immediately so the parent doesn't wait on the stdout pipe.
        os._exit(130)

    signal.signal(signal.SIGTERM, handle)
    # Also expose the cleanup_state via a global so handle_transcription
    # can update it as the tmp dir is created.
    global _signal_state
    _signal_state = cleanup_state  # type: ignore[name-defined]


_signal_state: dict[str, Any] = {}


# ---------------------------------------------------------------------------
# Main transcription flow
# ---------------------------------------------------------------------------
def run_transcription(url: str, model_size: str, use_timestamps: bool) -> None:
    """Run the full download + transcribe flow, emitting events to stdout."""
    check_ffmpeg()
    ensure_model_downloaded(model_size)

    tmp_dir = tempfile.mkdtemp()
    _signal_state["tmp_dir"] = tmp_dir
    try:
        emit_phase("downloading")

        def _on_download_progress(p: dict) -> None:
            emit_phase(
                "downloading-audio",
                percent=p.get("percent"),
                downloaded_bytes=p.get("downloaded_bytes"),
                total_bytes=p.get("total_bytes"),
                speed_bps=p.get("speed_bps"),
                eta_secs=p.get("eta_secs"),
            )

        audio_path = download_audio(
            url,
            tmp_dir,
            desktop_mode=True,
            progress_callback=_on_download_progress,
        )

        # Final 100% tick — yt-dlp's hook fires for the merged file but
        # not always for the post-processed mp3, so guarantee it.
        emit_phase("downloading-audio", percent=100.0)

        if _cancel_requested.is_set():
            raise Cancelled()

        emit_phase("transcribing")
        result = transcribe_audio(audio_path, model_size, use_timestamps)
        emit_result(result)
    finally:
        _signal_state["tmp_dir"] = None
        # ignore_errors=True prevents a file-lock or Spotlight race from
        # propagating an OSError after a successful transcription.
        shutil.rmtree(tmp_dir, ignore_errors=True)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------
# Default cap on search-results entries. Matches YouTube's own ~20-per-page
# granularity and keeps the probe under a couple of seconds.
SEARCH_RESULTS_LIMIT = 50


def _normalise_entries(raw_entries: list[Any]) -> list[dict[str, Any]]:
    """Project yt-dlp's playlist / search entries into the shape the frontend expects.

    Each entry becomes ``{id, title, thumbnail, duration, url}`` with absolute
    `youtube.com/watch?v=...` URLs even when yt-dlp returned a bare id. Used
    by both the playlist and search-results branches.
    """
    out: list[dict[str, Any]] = []
    for entry in raw_entries or []:
        if entry is None:
            continue
        entry_url = entry.get("url") or entry.get("webpage_url") or ""
        if entry_url and not entry_url.startswith("http"):
            vid_id = entry.get("id") or entry.get("url", "")
            entry_url = f"https://www.youtube.com/watch?v={vid_id}"
        out.append({
            "id": entry.get("id") or "",
            "title": entry.get("title") or entry.get("id") or "Untitled",
            "thumbnail": entry.get("thumbnail") or "",
            "duration": int(entry.get("duration") or 0),
            "url": entry_url,
        })
    return out


def probe_url(url: str) -> dict[str, Any]:
    """Probe a URL for metadata without downloading audio.

    Returns a dict with ``type='video'``, ``'playlist'``, ``'search'`` or
    ``'error'``. The search-results branch recognises YouTube
    ``/results?search_query=...`` URLs and resolves them via yt-dlp's
    ``ytsearch[N]:<query>`` extractor so the user can pick from a list
    instead of transcribing one arbitrary entry.
    """
    # Search-results detection runs BEFORE the generic extractor so we can
    # rewrite the URL into ytsearch[N]:<query> before yt-dlp sees the bare
    # /results page (which it would otherwise fail to expand).
    if _is_youtube_search_results_url(url):
        query = _search_query_from_url(url)
        if not query:
            return {
                "type": "error",
                "code": "INVALID_URL",
                "message": "search_query is empty",
            }
        # Cap the search depth so a runaway query can't stall the probe.
        n = max(1, min(SEARCH_RESULTS_LIMIT, 100))
        # yt-dlp's URL parser rejects `ytsearch[N]:` (brackets) and the bare
        # `ytsearch:` form is uncapped. The supported form for a bounded
        # count is `ytsearch<N>:<query>` — no brackets, count glued to the
        # prefix. `ytsearchall:` is also supported but unconstrained.
        synthetic_url = f"ytsearch{n}:{query}"
        ydl_opts = {
            "quiet": True,
            "no_warnings": True,
            "skip_download": True,
            "extract_flat": "in_playlist",
            "socket_timeout": 8,
        }
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(synthetic_url, download=False, process=False)
        except Exception as exc:
            code = classify_ydl_error(exc)
            return {"type": "error", "code": code, "message": str(exc)}

        if info is None:
            return {
                "type": "error",
                "code": "INVALID_URL",
                "message": "no metadata returned for search query",
            }

        # ytsearch[N]: returns a playlist-shaped result; reuse the same
        # entry normalisation so the frontend can render playlist and
        # search results with the same VideoPicker component.
        raw_entries = info.get("entries") or []
        entries = _normalise_entries(raw_entries)
        return {
            "type": "search",
            "kind": "search",
            "query": query,
            "url": url,
            "count": len(entries),
            "entries": entries,
        }

    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "skip_download": True,
        "extract_flat": "in_playlist",
        "socket_timeout": 8,
    }

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False, process=False)
    except Exception as exc:
        code = classify_ydl_error(exc)
        return {"type": "error", "code": code, "message": str(exc)}

    if info is None:
        return {"type": "error", "code": "INVALID_URL", "message": "no metadata returned"}

    is_playlist = info.get("_type") == "playlist" or "entries" in info

    if is_playlist:
        entries = _normalise_entries(info.get("entries") or [])
        return {
            "type": "playlist",
            "kind": "playlist",
            "url": info.get("webpage_url") or url,
            "title": info.get("title") or "Playlist",
            "uploader": info.get("uploader") or info.get("channel") or "",
            "count": len(entries),
            "entries": entries,
        }
    else:
        thumbnail = info.get("thumbnail") or ""
        thumbnails = info.get("thumbnails") or []
        if thumbnails:
            best = max(thumbnails, key=lambda t: (t.get("width") or 0) * (t.get("height") or 0))
            thumbnail = best.get("url") or thumbnail
        return {
            "type": "video",
            "url": info.get("webpage_url") or url,
            "title": info.get("title") or "Video",
            "thumbnail": thumbnail,
            "duration": int(info.get("duration") or 0),
            "uploader": info.get("uploader") or info.get("channel") or "",
        }


def parse_args() -> dict[str, Any]:
    """Parse request from CLI args (Tauri) or stdin JSON (dev mode)."""
    args = sys.argv[1:]
    if args:
        # CLI mode: --mode probe --url <url>
        #       or: --url <url> --model <model> --timestamps true|false
        out: dict[str, Any] = {}
        i = 0
        while i < len(args):
            k = args[i]
            v = args[i + 1] if i + 1 < len(args) else ""
            if k == "--url":
                out["url"] = v
            elif k == "--model":
                out["model"] = v
            elif k == "--timestamps":
                out["timestamps"] = v.lower() in ("true", "1", "yes")
            elif k == "--mode":
                out["mode"] = v
            i += 2
        return out
    # Stdin mode (legacy/dev).
    raw = sys.stdin.read()
    return json.loads(raw) if raw.strip() else {}


def main() -> int:
    _install_signal_handlers()

    try:
        data = parse_args()
    except json.JSONDecodeError as e:
        emit_error("INVALID_URL", f"invalid JSON: {e}")
        return 2
    except Exception as e:
        emit_error("INTERNAL", f"failed to read request: {e}")
        return 2

    mode = data.get("mode", "transcribe")

    if mode == "probe":
        url = (data.get("url") or "").strip()
        if not url or not url.startswith(("http://", "https://")):
            sys.stdout.write(json.dumps({"type": "error", "code": "INVALID_URL", "message": "url is required and must start with http(s)://"}) + "\n")
            sys.stdout.flush()
            return 2
        result = probe_url(url)
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()
        return 0

    try:
        url, model_size, use_timestamps = validate_request(data)
    except ValidationError as e:
        emit_error(e.code, e.message)
        return 2

    try:
        run_transcription(url, model_size, use_timestamps)
    except ValidationError as e:
        emit_error(e.code, e.message)
        return 2
    except Cancelled:
        # Signal handler already cleaned up and exited; we shouldn't get
        # here, but guard anyway.
        return 130
    except Exception as e:
        code = classify_ydl_error(e)
        emit_error(code, str(e))
        # Also log to stderr for debugging (visible in `tauri dev`).
        print(f"sidecar: {code}: {e}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        return 1
    finally:
        if not _cancel_requested.is_set():
            emit({"event": "done"})

    return 0


if __name__ == "__main__":
    # Required for PyInstaller --onefile on macOS (spawn start method in
    # Python 3.8+): worker processes re-execute the frozen binary, so
    # freeze_support() intercepts them before main() runs.
    import multiprocessing
    multiprocessing.freeze_support()
    sys.exit(main())