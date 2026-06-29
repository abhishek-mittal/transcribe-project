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
import platform
import re
import shutil
import signal
import sys
import tempfile
import threading
import time
import traceback
from typing import Any

# ---------------------------------------------------------------------------
# platform.mac_ver() workaround (FIX-10)
#
# On some macOS versions, platform.mac_ver()[0] returns a malformed release
# string (e.g. with an empty/blank dot-separated segment). yt-dlp's plugin
# discovery (yt_dlp/update.py:_get_variant_and_executable_path, called
# unconditionally the first time any YoutubeDL() is constructed) parses this
# string via version_tuple() WITHOUT yt-dlp's own lenient=True option, so
# int('') raises ValueError -- surfacing as "invalid literal for int() with
# base 10: ''" on every single transcription/download attempt, regardless
# of URL. Confirmed via a real traceback captured on an affected M2 Mac.
# Patched here, before yt_dlp is imported, so it's defensive against
# whichever code path constructs the first YoutubeDL() instance.
_real_mac_ver = platform.mac_ver


def _safe_mac_ver():
    release, versioninfo, machine = _real_mac_ver()
    if not release or any(part == "" for part in release.split(".")):
        release = "0.0.0"
    return release, versioninfo, machine


platform.mac_ver = _safe_mac_ver

import yt_dlp

from api.transcribe_core import (
    DEFAULT_MODEL,
    VALID_MODELS,
    _get_model,
    _ig_cookies_file_path,
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

# yt-dlp error messages to classify as bot challenges (YouTube-specific).
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

# Instagram login-wall errors — separate code so the UI can give a specific hint.
_INSTAGRAM_AUTH_PATTERNS = [
    re.compile(p, re.IGNORECASE)
    for p in (
        r"instagram sent an empty media response",
        r"instagram.*login",
        r"login.*instagram",
        r"you need to log in",
        r"not logged in",
        r"checkpoint required",
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


def validate_model_and_timestamps(data: dict[str, Any]) -> tuple[str, bool]:
    """Validate and return (model, use_timestamps). Raise ValidationError on failure.

    Shared by `validate_request` (which additionally requires `url`) and
    `--mode transcribe` (F13's file-only mode, which has no `url` at all —
    it transcribes an already-downloaded file).
    """
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
    return model, use_timestamps


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

    model, use_timestamps = validate_model_and_timestamps(data)
    return url, model, use_timestamps


# ---------------------------------------------------------------------------
# FFmpeg detection
# ---------------------------------------------------------------------------
def resolve_ffmpeg() -> str:
    """Resolve the ffmpeg binary to use, bundled-first.

    When frozen (PyInstaller build shipped in the app bundle), ffmpeg is
    bundled by `scripts/fetch_ffmpeg.py` next to the sidecar executable
    itself — use that unconditionally so the app works without requiring
    `brew install ffmpeg` on the host. When not frozen (dev mode via
    `python -m api.sidecar` or the Flask `dev:api` path), fall back to
    `PATH` exactly as before.
    """
    if getattr(sys, "frozen", False):
        bundled = os.path.join(os.path.dirname(sys.executable), "ffmpeg")
        if os.path.exists(bundled):
            return bundled

    path_ffmpeg = shutil.which("ffmpeg")
    if path_ffmpeg is None:
        raise ValidationError(
            "FFMPEG_MISSING",
            "FFmpeg is required. Install with `brew install ffmpeg` (macOS) or "
            "see https://ffmpeg.org/download.html",
        )
    return path_ffmpeg


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
    for pat in _INSTAGRAM_AUTH_PATTERNS:
        if pat.search(msg):
            return "INSTAGRAM_LOGIN_REQUIRED"
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
    ffmpeg_path = resolve_ffmpeg()
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
            ffmpeg_location=ffmpeg_path,
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
# F13: pipeline queue — download-only and transcribe-only modes
#
# Every event emitted by these two functions carries `item_id` so the Rust
# layer (and the frontend behind it) can route concurrent download events
# to the correct queue row — up to 5 `--mode download` sidecars run at once.
# ---------------------------------------------------------------------------
def run_download(url: str, out_dir: str, item_id: str) -> None:
    """Download audio only (no transcription) into the persistent
    `downloads/<job_id>/` directory the Rust layer created via `start_job`.

    Unlike `run_transcription`'s tempdir (deleted unconditionally in its
    `finally`), `out_dir` here is NOT cleaned up by this function — the
    file must survive until the separate `--mode transcribe` step (or
    until `cancel_download`/`finalize_job` clean it up on the Rust side).
    """
    ffmpeg_path = resolve_ffmpeg()
    os.makedirs(out_dir, exist_ok=True)

    def _on_download_progress(p: dict) -> None:
        emit_phase(
            "downloading-audio",
            item_id=item_id,
            percent=p.get("percent"),
            downloaded_bytes=p.get("downloaded_bytes"),
            total_bytes=p.get("total_bytes"),
            speed_bps=p.get("speed_bps"),
            eta_secs=p.get("eta_secs"),
        )

    emit_phase("downloading", item_id=item_id)
    audio_path = download_audio(
        url,
        out_dir,
        desktop_mode=True,
        progress_callback=_on_download_progress,
        ffmpeg_location=ffmpeg_path,
    )
    emit_phase("downloading-audio", item_id=item_id, percent=100.0)

    if _cancel_requested.is_set():
        raise Cancelled()

    emit({"event": "download-done", "item_id": item_id, "path": audio_path})


def run_transcribe_file(audio_path: str, item_id: str, model_size: str, use_timestamps: bool) -> None:
    """Transcribe an already-downloaded audio file (no yt-dlp involved).

    On success, deletes `audio_path` — the file's only purpose was getting
    to this point. On failure, the file is left in place so a retry can
    skip straight back to transcription instead of re-downloading.
    """
    resolve_ffmpeg()
    ensure_model_downloaded(model_size)

    emit_phase("transcribing", item_id=item_id)
    result = transcribe_audio(audio_path, model_size, use_timestamps)

    if _cancel_requested.is_set():
        raise Cancelled()

    emit({"event": "result", "item_id": item_id, **result})

    try:
        os.unlink(audio_path)
    except OSError as e:
        # Non-fatal: a stray leftover file is cleaned up later by
        # finalize_job's downloads/<job_id>/ directory removal anyway.
        print(f"run_transcribe_file: failed to delete {audio_path}: {e}", file=sys.stderr)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------
# Default cap on search-results entries. Matches YouTube's own ~20-per-page
# granularity and keeps the probe under a couple of seconds even on slow
# residential networks. Larger counts are rejected — see MAX below.
SEARCH_RESULTS_LIMIT = 20
# Hard ceiling regardless of what callers request. Stops a malformed /
# pathological query from stalling the probe for tens of seconds.
SEARCH_RESULTS_MAX = 30
# Per-socket timeout for yt-dlp HTTP calls during probe. Generous enough
# to survive a slow residential link without timing out on a single
# thumbnail fetch, but tight enough that a stuck connection doesn't
# multiply across 20 entries.
SEARCH_SOCKET_TIMEOUT = 15
# Initial page size for the FIRST probe of a playlist/channel (F14). We
# used to send PLAYLIST_PAGE_SIZE (20) here, which forced yt-dlp to walk
# the full playlist index before returning anything. For a YouTube
# channel tab like `@MillieAdrian/shorts` that walk can take 5–10s
# with zero UI feedback. Streaming the first 5 entries as they resolve
# (via the process_info callback below) gives the user something to look
# at in ~1s; the remaining entries are paged in via probe_url_page when
# they hit "Load more".
INITIAL_PAGE_SIZE = 5
# Subsequent "Load more" pages and the standalone probe_url_page still use
# this larger page size — 20 matches YouTube's per-page count and amortises
# the round-trip cost across more rows.
PLAYLIST_PAGE_SIZE = 20


def _thumbnail_for_entry(entry: dict[str, Any]) -> str:
    """Return a thumbnail URL for a yt-dlp playlist entry.

    flat-extract entries often have no thumbnail; build a YouTube default
    thumbnail URL from the video ID when available.
    """
    thumb = entry.get("thumbnail") or ""
    if thumb:
        return thumb
    vid_id = entry.get("id") or ""
    if len(vid_id) == 11 and re.match(r'^[A-Za-z0-9_-]+$', vid_id):
        return f"https://i.ytimg.com/vi/{vid_id}/mqdefault.jpg"
    return ""


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
            "thumbnail": _thumbnail_for_entry(entry),
            "duration": int(entry.get("duration") or 0),
            "url": entry_url,
        })
    return out


def _probe_ig_oembed(url: str) -> dict[str, Any]:
    """Probe an Instagram URL via the public oEmbed API (no auth required).

    yt-dlp's Instagram extractor uses a GraphQL doc_id that Instagram retired
    for anonymous access (returns ``execution error`` + ``data: null``), so we
    bypass it entirely for the probe step.  oEmbed works for any public post,
    reel, or IGTV without session cookies.  Duration is not available from
    oEmbed — the frontend handles 0 as "unknown".
    """
    import urllib.error
    import urllib.parse
    import urllib.request

    oembed_url = (
        "https://www.instagram.com/api/v1/oembed/"
        f"?url={urllib.parse.quote(url, safe='')}&hidecaption=0&maxwidth=658"
    )
    req = urllib.request.Request(
        oembed_url,
        headers={
            "User-Agent": (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                "AppleWebKit/537.36 (KHTML, like Gecko) "
                "Chrome/131.0.0.0 Safari/537.36"
            ),
            "Accept": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read().decode())
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"type": "error", "code": "INVALID_URL", "message": "Instagram post not found"}
        if exc.code in (401, 403):
            return {
                "type": "error",
                "code": "INSTAGRAM_LOGIN_REQUIRED",
                "message": f"Instagram returned {exc.code}",
            }
        return {"type": "error", "code": "NETWORK_ERROR", "message": str(exc)}
    except Exception as exc:
        return {"type": "error", "code": "NETWORK_ERROR", "message": str(exc)}

    return {
        "type": "video",
        "url": url,
        "title": data.get("title") or data.get("author_name") or "Instagram Video",
        "thumbnail": data.get("thumbnail_url") or "",
        "duration": 0,
        "uploader": data.get("author_name") or "",
    }


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
        # SEARCH_RESULTS_LIMIT (20) matches YouTube's per-page count; the
        # hard cap SEARCH_RESULTS_MAX (30) is a safety net.
        n = max(1, min(SEARCH_RESULTS_LIMIT, SEARCH_RESULTS_MAX))
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
            "socket_timeout": SEARCH_SOCKET_TIMEOUT,
        }
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(synthetic_url, download=False, process=True)
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
        # search results with the same VideoPicker component. Note that
        # thumbnails come back empty under extract_flat=in_playlist — the
        # picker renders a placeholder when this happens.
        raw_entries = list(info.get("entries") or [])
        entries = _normalise_entries(raw_entries)
        return {
            "type": "search",
            "kind": "search",
            "query": query,
            "url": url,
            "count": len(entries),
            "entries": entries,
            # Search results are fetched exactly to `n` with no further
            # pages available, so total is just what we got — no Load More.
            "total_count": len(entries),
        }

    from api.transcribe_core import _is_instagram_url, _is_instagram_profile_url

    # Instagram: bypass yt-dlp's GraphQL path (doc_id retired for anonymous
    # access since 2024) and use the public oEmbed API instead.
    if _is_instagram_url(url):
        if _is_instagram_profile_url(url):
            return {
                "type": "error",
                "code": "IG_PROFILE_UNSUPPORTED",
                "message": "Instagram profile pages aren't supported. Paste an individual reel URL — e.g. instagram.com/reel/ABC123",
            }
        return _probe_ig_oembed(url)

    # F14 progressive picker: emit one `entry` event per resolved entry so
    # the UI can render rows as yt-dlp finds them, instead of waiting for
    # the whole playlistend walk to finish before returning the list.
    # We cap the first probe at INITIAL_PAGE_SIZE (5) — the rest come via
    # probe_url_page when the user clicks "Load more".
    emitted_entry_ids: set[str] = set()

    def _process_entry(entry: dict) -> None:
        """yt-dlp per-entry callback (F14). Fires once per entry as
        yt-dlp resolves it, well before extract_info returns. We
        normalize + dedupe + emit so the frontend can stream rows in."""
        if entry is None:
            return
        eid = entry.get("id") or ""
        if eid and eid in emitted_entry_ids:
            return  # yt-dlp can fire twice for some extractors
        if eid:
            emitted_entry_ids.add(eid)
        emit({"event": "entry", "entry": _normalise_entry(entry)})

    # Tiny status heartbeat so the UI can show "fetching…" before the
    # first entry resolves (yt-dlp is silent for the first ~1–2s of a
    # channel walk while it's resolving the playlist index itself).
    emit({"event": "status", "message": "Resolving channel index…"})

    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "skip_download": True,
        "extract_flat": "in_playlist",
        "socket_timeout": 8,
        "playlistend": INITIAL_PAGE_SIZE,
        "process_info": _process_entry,
    }

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False, process=True)
    except Exception as exc:
        code = classify_ydl_error(exc)
        # Also stream an `error` event so the UI's activity strip can show
        # it without waiting for the final result line.
        emit({"event": "error", "code": code, "message": str(exc)})
        return {"type": "error", "code": code, "message": str(exc)}

    if info is None:
        emit({"event": "error", "code": "INVALID_URL", "message": "no metadata returned"})
        return {"type": "error", "code": "INVALID_URL", "message": "no metadata returned"}

    is_playlist = info.get("_type") == "playlist" or "entries" in info

    if is_playlist:
        entries = _normalise_entries(list(info.get("entries") or []))
        total_count = info.get("playlist_count")
        # F14: emit a `done` event with the totals so the UI knows the
        # initial probe finished and can show "5 of N videos · streaming
        # more in background" before the user clicks Load more.
        emit({"event": "done", "type": "playlist", "count": len(entries), "total_count": total_count})
        return {
            "type": "playlist",
            "kind": "playlist",
            "url": info.get("webpage_url") or url,
            "title": info.get("title") or "Playlist",
            "uploader": info.get("uploader") or info.get("channel") or "",
            "count": len(entries),
            "entries": entries,
            # yt-dlp populates this on the root playlist object under
            # extract_flat=in_playlist at no extra network cost. None when
            # the source doesn't expose a count (e.g. some channel tabs).
            "total_count": total_count,
        }
    else:
        thumbnail = info.get("thumbnail") or ""
        thumbnails = info.get("thumbnails") or []
        if thumbnails:
            best = max(thumbnails, key=lambda t: (t.get("width") or 0) * (t.get("height") or 0))
            thumbnail = best.get("url") or thumbnail
        # F14: also signal done for single-video probes so the UI can
        # flip out of "probing" state promptly.
        emit({"event": "done", "type": "video"})
        return {
            "type": "video",
            "url": info.get("webpage_url") or url,
            "title": info.get("title") or "Video",
            "thumbnail": thumbnail,
            "duration": int(info.get("duration") or 0),
            "uploader": info.get("uploader") or info.get("channel") or "",
        }


def probe_url_page(url: str, page_start: int, page_end: int) -> dict[str, Any]:
    """Fetch one additional page of playlist/channel entries for "Load more".

    `page_start`/`page_end` are 1-indexed and passed straight through to
    yt-dlp's `playliststart`/`playlistend` (matching yt-dlp's own
    convention, same as the initial probe's `playlistend`). Only the new
    page's entries are returned — the frontend appends them to the list it
    already has, so this never re-fetches or re-sends earlier pages.
    """
    if page_start < 1 or page_end < page_start:
        return {"type": "error", "code": "INVALID_URL", "message": "invalid page range"}

    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "skip_download": True,
        "extract_flat": "in_playlist",
        "socket_timeout": 8,
        "playliststart": page_start,
        "playlistend": page_end,
    }

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False, process=True)
    except Exception as exc:
        code = classify_ydl_error(exc)
        return {"type": "error", "code": code, "message": str(exc)}

    if info is None:
        return {"type": "error", "code": "INVALID_URL", "message": "no metadata returned"}

    entries = _normalise_entries(list(info.get("entries") or []))
    return {
        "type": "page",
        "entries": entries,
        "total_count": info.get("playlist_count"),
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
            elif k == "--page-start":
                out["page_start"] = int(v)
            elif k == "--page-end":
                out["page_end"] = int(v)
            elif k == "--out-dir":
                out["out_dir"] = v
            elif k == "--item-id":
                out["item_id"] = v
            elif k == "--audio-path":
                out["audio_path"] = v
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

    # No --mode flag at all (the legacy run_sidecar single-video path,
    # which never sets one) defaults to the full download+transcribe flow —
    # named "transcribe-full" per F13 to free up the bare "transcribe" mode
    # name for the new file-only mode below.
    mode = data.get("mode", "transcribe-full")

    if mode == "probe":
        url = (data.get("url") or "").strip()
        if not url or not url.startswith(("http://", "https://")):
            sys.stdout.write(json.dumps({"type": "error", "code": "INVALID_URL", "message": "url is required and must start with http(s)://"}) + "\n")
            sys.stdout.flush()
            return 2
        page_start = data.get("page_start")
        page_end = data.get("page_end")
        if page_start is not None and page_end is not None:
            result = probe_url_page(url, page_start, page_end)
        else:
            result = probe_url(url)
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()
        return 0

    if mode == "download":
        url = (data.get("url") or "").strip()
        out_dir = (data.get("out_dir") or "").strip()
        item_id = (data.get("item_id") or "").strip()
        if not url or not url.startswith(("http://", "https://")):
            emit_error("INVALID_URL", "url is required and must start with http(s)://")
            return 2
        if not out_dir or not item_id:
            emit_error("INVALID_URL", "out_dir and item_id are required for --mode download")
            return 2
        try:
            run_download(url, out_dir, item_id)
        except Cancelled:
            return 130
        except Exception as e:
            code = classify_ydl_error(e)
            message = str(e)
            if code == "INSTAGRAM_LOGIN_REQUIRED":
                cookies_path = _ig_cookies_file_path()
                message = (
                    "Instagram requires login to download this video. "
                    "Open Instagram in Safari and log in, then try again. "
                    f"Or place a cookies.txt file at: {cookies_path}"
                )
            emit({"event": "error", "item_id": item_id, "code": code, "message": message})
            print(f"sidecar[download item={item_id}]: {code}: {e}", file=sys.stderr)
            traceback.print_exc(file=sys.stderr)
            return 1
        return 0

    if mode == "transcribe":
        audio_path = (data.get("audio_path") or "").strip()
        item_id = (data.get("item_id") or "").strip()
        if not audio_path or not item_id:
            emit_error("INVALID_URL", "audio_path and item_id are required for --mode transcribe")
            return 2
        try:
            model_size, use_timestamps = validate_model_and_timestamps(data)
        except ValidationError as e:
            emit({"event": "error", "item_id": item_id, "code": e.code, "message": e.message})
            return 2
        try:
            run_transcribe_file(audio_path, item_id, model_size, use_timestamps)
        except Cancelled:
            return 130
        except Exception as e:
            code = classify_ydl_error(e)
            emit({"event": "error", "item_id": item_id, "code": code, "message": str(e)})
            print(f"sidecar[transcribe item={item_id}]: {code}: {e}", file=sys.stderr)
            traceback.print_exc(file=sys.stderr)
            return 1
        return 0

    # mode == "transcribe-full": existing single-video download+transcribe
    # flow, unchanged.
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
        if code == "INSTAGRAM_LOGIN_REQUIRED":
            cookies_path = _ig_cookies_file_path()
            message = (
                "Instagram requires login to download this video. "
                "Open Instagram in Safari and log in, then try again. "
                f"Or place a cookies.txt file at: {cookies_path}"
            )
        else:
            message = str(e)
        emit_error(code, message)
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