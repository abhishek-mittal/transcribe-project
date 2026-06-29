"""Core transcription logic for the transcribe-project.

Pure module with no Flask or Tauri dependency. Imported by:
- ``api/transcribe.py`` (Flask server, reference / future hosted path)
- ``api/sidecar.py`` (Tauri desktop sidecar)

Provides ``download_audio``, ``transcribe_audio``, formatting helpers, and
``_get_model`` with module-level model caching.
"""

from __future__ import annotations

import logging
import os
import shutil
import time
from datetime import timedelta
from typing import TYPE_CHECKING, Callable, Optional
from urllib.parse import parse_qs, urlparse

import yt_dlp
from faster_whisper import WhisperModel

if TYPE_CHECKING:
    pass


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
# Model is controlled by the WHISPER_MODEL env var on the VPS.
# 2 GB VPS: tiny (~75 MB) or base (~141 MB) are safe.
# 4 GB+ VPS: small (~480 MB) is viable.
VALID_MODELS = {"tiny", "base", "small"}
DEFAULT_MODEL = os.environ.get("WHISPER_MODEL", "tiny")

# Pre-downloaded model weights live here (populated by scripts/predownload_model.py).
# Each model lives in its own subdirectory: api/_models/<model_name>/.
MODELS_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "_models")


# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------
_LOG_LEVEL = os.environ.get("LOG_LEVEL", "INFO").upper()
logging.basicConfig(
    level=_LOG_LEVEL,
    format="%(asctime)s %(levelname)s [%(name)s] %(message)s",
    datefmt="%Y-%m-%dT%H:%M:%S",
)
logger = logging.getLogger("transcribe")


# ---------------------------------------------------------------------------
# Model management
# ---------------------------------------------------------------------------
# Cache loaded WhisperModel instances by (size, device, compute_type).
# Sidecar is single-flight (Rust layer enforces it) and Gunicorn workers are
# long-lived, so this avoids reloading on every request.
_MODEL_CACHE: dict[tuple[str, str, str], WhisperModel] = {}


def _resolve_model_source(model_size: str) -> str:
    """Return a local directory if pre-downloaded, else the bare model name."""
    local = os.path.join(MODELS_DIR, model_size)
    if os.path.isfile(os.path.join(local, "model.bin")):
        return local
    return model_size


def _get_model(
    model_size: str, device: str = "cpu", compute_type: str = "int8"
) -> WhisperModel:
    """Load a WhisperModel, using a module-level cache.

    Sidecar callers should pre-download the model via
    :func:`ensure_model_downloaded` so this call finds the local path.
    """
    key = (model_size, device, compute_type)
    model = _MODEL_CACHE.get(key)
    if model is None:
        logger.info(
            "Loading WhisperModel size=%s device=%s compute_type=%s (cache miss)",
            model_size,
            device,
            compute_type,
        )
        load_start = time.perf_counter()
        source = _resolve_model_source(model_size)
        model = WhisperModel(source, device=device, compute_type=compute_type)
        _MODEL_CACHE[key] = model
        logger.info(
            "WhisperModel loaded size=%s source=%s in %.2fs",
            model_size,
            "bundled" if source != model_size else "hf-hub",
            time.perf_counter() - load_start,
        )
    else:
        logger.debug("WhisperModel cache hit size=%s", model_size)
    return model


# ---------------------------------------------------------------------------
# Formatting helpers
# ---------------------------------------------------------------------------
def format_timestamp(seconds: float) -> str:
    """MM:SS format for timestamped transcripts."""
    return str(timedelta(seconds=int(seconds)))[2:]


def format_srt_timestamp(seconds: float) -> str:
    """HH:MM:SS,mmm format required by SRT spec."""
    total_ms = int(seconds * 1000)
    hours, remainder = divmod(total_ms, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    secs, ms = divmod(remainder, 1000)
    return f"{hours:02d}:{minutes:02d}:{secs:02d},{ms:03d}"


# ---------------------------------------------------------------------------
# Host classification
# ---------------------------------------------------------------------------
# Hosts whose downloads need the YouTube-specific anti-bot tweaks (cookies,
# player_client fallback chain, PO-token plugin). Anything else uses plain
# yt-dlp defaults so we don't leak YouTube session cookies to unrelated hosts.
_YOUTUBE_HOSTS = {
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtu.be",
    "youtube-nocookie.com",
    "www.youtube-nocookie.com",
}

# Hosts whose `/results?search_query=...` path is a YouTube search results
# page. Same host set as _YOUTUBE_HOSTS but kept separate so the search-results
# detection stays opt-in and we don't accidentally reroute regular watch URLs.
_YOUTUBE_SEARCH_RESULTS_HOSTS = _YOUTUBE_HOSTS

# Instagram also rate-limits / requires login for most content from datacenter
# IPs. It needs its OWN cookies jar -- never share with YouTube.
_INSTAGRAM_HOSTS = {
    "instagram.com",
    "www.instagram.com",
    "m.instagram.com",
}


def _host_of(url: str) -> str:
    try:
        return (urlparse(url).hostname or "").lower()
    except ValueError:
        return ""


def _is_youtube_url(url: str) -> bool:
    return _host_of(url) in _YOUTUBE_HOSTS


def _is_youtube_search_results_url(url: str) -> bool:
    """True only for YouTube search-results pages (`/results?search_query=...`).

    Regular watch/playlist URLs return False even though they live on the
    same hosts. The probe layer relies on this being strictly opt-in.
    """
    if _host_of(url) not in _YOUTUBE_SEARCH_RESULTS_HOSTS:
        return False
    try:
        path = urlparse(url).path
    except ValueError:
        return False
    return path == "/results" or path == "/results/"


def _search_query_from_url(url: str) -> str | None:
    """Extract the `search_query` value from a YouTube results URL.

    Returns the decoded query string, or None when the parameter is missing or
    empty. Whitespace is trimmed so callers can detect "no query" cleanly.
    """
    try:
        parsed = urlparse(url)
    except ValueError:
        return None
    qs = parse_qs(parsed.query)
    values = qs.get("search_query") or []
    raw = values[0] if values else ""
    raw = raw.strip()
    return raw or None


def _is_instagram_url(url: str) -> bool:
    return _host_of(url) in _INSTAGRAM_HOSTS


_INSTAGRAM_POST_KEYWORDS = {"reel", "reels", "p", "tv", "stories"}


def _is_instagram_profile_url(url: str) -> bool:
    """Return True for IG profile/listing pages, False for individual posts/reels.

    Profile/listing: instagram.com/username/, instagram.com/username/reels (no id).
    Individual:      instagram.com/reel/<id>, instagram.com/p/<id>,
                      instagram.com/<username>/reel/<id> (IG's own share links
                      now prefix individual post URLs with the username, so the
                      post keyword isn't always segment[0]).
    """
    if not _is_instagram_url(url):
        return False
    try:
        path = urlparse(url).path.strip("/")
    except ValueError:
        return False
    segments = [s for s in path.split("/") if s]
    if not segments:
        return True
    # An individual post/reel/story always has a known keyword segment
    # immediately followed by an id segment, whether or not it's prefixed
    # by the username (instagram.com/reel/<id> or instagram.com/<user>/reel/<id>).
    for segment in segments[:-1]:
        if segment.lower() in _INSTAGRAM_POST_KEYWORDS:
            return False
    return True


def _ig_cookies_file_path() -> str:
    """Return the well-known per-platform path for the Instagram cookies file.

    Users export a Netscape cookies file (e.g. via the 'Get cookies.txt LOCALLY'
    Chrome extension) and drop it here.  The app picks it up automatically —
    same pattern as the server-side IG_DLP_COOKIES_FILE env var.

    macOS:   ~/Library/Application Support/com.shuhari.transcribe/instagram_cookies.txt
    Windows: %APPDATA%\\com.shuhari.transcribe\\instagram_cookies.txt
    Linux:   ~/.local/share/com.shuhari.transcribe/instagram_cookies.txt
    """
    import platform

    home = os.path.expanduser("~")
    app_id = "com.shuhari.transcribe"
    system = platform.system()
    if system == "Darwin":
        return os.path.join(home, "Library", "Application Support", app_id, "instagram_cookies.txt")
    if system == "Windows":
        appdata = os.environ.get("APPDATA", os.path.join(home, "AppData", "Roaming"))
        return os.path.join(appdata, app_id, "instagram_cookies.txt")
    xdg = os.environ.get("XDG_DATA_HOME", os.path.join(home, ".local", "share"))
    return os.path.join(xdg, app_id, "instagram_cookies.txt")


def _inject_ig_browser_cookies(ydl_opts: dict) -> None:
    """Inject Instagram session cookies for desktop mode.

    Strategy (in priority order):
    1. User-placed Netscape cookies file at the standard app-data path —
       written by the Settings → Instagram "paste cookie header" flow (Rust
       `save_instagram_cookies`), or manually via a browser extension.
       Primary, recommended path: always wins if present.
    2. Live browser session cookies (Safari → Chrome → Firefox on macOS).
       Best-effort fallback for users who skipped step 1 but happen to be
       signed into Instagram in one of those browsers. Each candidate's
       cookie jar is read here (not just configured) so a jar we can't
       access — e.g. Safari's, which macOS sandboxes behind a TCC
       permission a bundled PyInstaller binary doesn't have — is skipped
       instead of crashing the whole download. Without this try/except,
       `cookiesfrombrowser` would only be *set*, not *validated*, here:
       yt-dlp would hit the same PermissionError later inside
       `extract_info`, too late to fall back to the next browser.

    Instagram requires authentication even for "public" reels (server-side
    enforcement since late 2024) — there is no cookie-free path.
    """
    # Strategy 1: explicit cookies file (most reliable)
    cookies_path = _ig_cookies_file_path()
    if _is_valid_netscape_cookies(cookies_path):
        ydl_opts["cookiefile"] = cookies_path
        logger.info("instagram desktop: using cookies file %s", cookies_path)
        return

    # Strategy 2: live browser session — actually try each candidate so a
    # read failure (permission denied, browser not installed, no profile)
    # falls through to the next one instead of surfacing later as a crash.
    from yt_dlp.cookies import extract_cookies_from_browser

    import platform

    system = platform.system()
    candidates: list[str] = {
        "Darwin":  ["safari", "chrome", "firefox"],
        "Windows": ["chrome", "edge", "firefox"],
    }.get(system, ["chrome", "firefox"])

    for browser in candidates:
        try:
            jar = extract_cookies_from_browser(browser)
        except Exception as exc:
            logger.info(
                "instagram desktop: could not read %s cookie jar (%s); trying next",
                browser,
                exc,
            )
            continue
        if not jar:
            continue
        ydl_opts["cookiesfrombrowser"] = (browser,)
        logger.info("instagram desktop: using live %s session cookies", browser)
        return

    logger.info(
        "instagram desktop: no cookies file and no readable browser session; "
        "proceeding without auth — Instagram will likely reject this request"
    )


def _is_valid_netscape_cookies(path: str) -> bool:
    """Return True only if *path* exists and starts with the Netscape cookie header.

    yt-dlp raises an error if it is handed a cookies file that lacks this
    header (empty placeholder files, wrong format, etc.).  Checking here lets
    us silently skip bad files instead of letting the error surface to users.
    """
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                return "Netscape HTTP Cookie File" in line
    except OSError:
        pass
    return False


# ---------------------------------------------------------------------------
# Public API: download + transcribe
# ---------------------------------------------------------------------------
def download_audio(
    url: str,
    output_dir: str,
    *,
    desktop_mode: bool = False,
    progress_callback: Optional[Callable[[dict], None]] = None,
    ffmpeg_location: Optional[str] = None,
) -> str:
    """Download audio from *url* into *output_dir* and return the audio file path.

    When ``desktop_mode=True`` (the Tauri sidecar), all server-side anti-
    blocking workarounds are disabled: cookies, player-client fallback chain,
    PO-token plugin, and proxy. Requests originate from the user's residential
    IP so these workarounds are dead code and a maintenance burden.

    When ``desktop_mode=False`` (the Flask server), the full anti-blocking
    stack is applied: YouTube gets cookies + player-client fallback + PO-token,
    Instagram gets its own cookies jar, and an outbound proxy is honored if
    ``YT_DLP_PROXY`` is set.

    If ``progress_callback`` is supplied it is invoked from yt-dlp's
    ``progress_hooks`` with a dict::

        {
            "phase": "downloading-audio" | "finished",
            "percent": float,            # 0.0–100.0, may exceed 100 for
                                         # multi-stream merges
            "downloaded_bytes": int,
            "total_bytes": int | None,   # None when unknown
            "speed_bps": int | None,     # bytes/sec, None when unknown
            "eta_secs": int | None,      # seconds, None when unknown
        }

    Callbacks must be cheap and non-blocking — yt-dlp invokes them on the
    download thread for every progress tick.

    ``ffmpeg_location``, when supplied, is passed straight through to
    yt-dlp's own ``ffmpeg_location`` option so stream merging/remuxing uses
    that exact binary instead of yt-dlp's own implicit `PATH` lookup. The
    Tauri sidecar (``api/sidecar.py``) resolves this via ``resolve_ffmpeg()``
    — bundled binary when frozen, `PATH` otherwise — and passes the result
    here so the resolution and the actual usage never diverge. The Flask
    server path doesn't pass this; it relies on yt-dlp's own PATH lookup
    exactly as before.
    """
    logger.info(
        "download_audio start url=%s desktop_mode=%s", url, desktop_mode
    )
    download_start = time.perf_counter()

    is_youtube = _is_youtube_url(url)
    is_instagram = _is_instagram_url(url)

    # YouTube's n-challenge (URL deobfuscation) requires a JS runtime since
    # yt-dlp 2025. Node.js is installed on the VPS so this will always be
    # available. Locally we fall through gracefully if absent.
    node_path = shutil.which("node")
    ydl_opts: dict = {
        "format": "bestaudio/best",
        "outtmpl": os.path.join(output_dir, "%(id)s.%(ext)s"),
        "quiet": True,
        "no_warnings": True,
        "postprocessors": [
            {
                "key": "FFmpegExtractAudio",
                "preferredcodec": "mp3",
                "preferredquality": "128",
            }
        ],
    }
    if node_path:
        ydl_opts["js_runtimes"] = {"node": {"path": node_path}}
    if ffmpeg_location:
        ydl_opts["ffmpeg_location"] = ffmpeg_location

    if not desktop_mode:
        # The cookies, player-client fallback chain, and PO-token plugin are
        # all YouTube-specific workarounds for its datacenter-IP bot
        # challenge. Do not apply them to other hosts: a YouTube cookies.txt
        # would leak Google session cookies to e.g. Vimeo / SoundCloud /
        # podcast hosts on every request, and the extractor_args /
        # player_client knobs are no-ops there.
        if is_youtube:
            cookies_file = os.environ.get("YT_DLP_COOKIES_FILE")
            if cookies_file and _is_valid_netscape_cookies(cookies_file):
                ydl_opts["cookiefile"] = cookies_file

            # Trying multiple player clients lets yt-dlp fall back when one
            # client is bot-challenged. The bgutil PO-token plugin (installed
            # via pip on the VPS) auto-engages when present and provides a
            # Proof-of-Origin token that bypasses many YouTube bot checks.
            # YT_DLP_PLAYER_CLIENTS (comma-separated) overrides the default
            # list.
            player_clients = [
                c.strip()
                for c in os.environ.get(
                    "YT_DLP_PLAYER_CLIENTS", "android,ios,tv"
                ).split(",")
                if c.strip()
            ]
            ydl_opts["extractor_args"] = {
                "youtube": {
                    "player_client": player_clients,
                    "fetch_pot": ["always"],
                    "player_skip": ["webpage"],
                }
            }

        # Instagram requires login for most posts/reels from datacenter IPs.
        if is_instagram:
            ig_cookies_file = os.environ.get("IG_DLP_COOKIES_FILE")
            if ig_cookies_file and _is_valid_netscape_cookies(ig_cookies_file):
                ydl_opts["cookiefile"] = ig_cookies_file

        # Optional outbound proxy (residential / mobile) to dodge
        # datacenter-IP gates. Applies to all hosts because a flagged Vultr
        # IP affects every extractor, not just YouTube.
        proxy = os.environ.get("YT_DLP_PROXY")
        if proxy:
            ydl_opts["proxy"] = proxy
    else:
        logger.info(
            "download_audio desktop_mode=True: YouTube workarounds skipped; "
            "using user's residential IP"
        )
        # Instagram requires session cookies even for public reels on residential
        # IPs (server-side change, late 2024). Borrow the user's browser session.
        if is_instagram:
            _inject_ig_browser_cookies(ydl_opts)

    if progress_callback is not None:
        # yt-dlp calls progress_hooks for both the video+audio download and
        # the post-processed audio-only download; we filter to the audio
        # stream (status=="downloading" with a known total) and coalesce
        # the raw dict into the queue-row contract used by the desktop UI.
        def _hook(d: dict) -> None:
            status = d.get("status")
            if status == "finished":
                try:
                    progress_callback({
                        "phase": "finished",
                        "percent": 100.0,
                        "downloaded_bytes": d.get("downloaded_bytes") or 0,
                        "total_bytes": d.get("total_bytes"),
                        "speed_bps": None,
                        "eta_secs": 0,
                    })
                except Exception:  # pragma: no cover
                    logger.exception("download_audio progress_callback failed")
                return

            if status != "downloading":
                return

            total = d.get("total_bytes") or d.get("total_bytes_estimate")
            downloaded = d.get("downloaded_bytes")
            if total is None or downloaded is None:
                return

            try:
                percent = max(0.0, min(100.0, (downloaded / total) * 100.0))
                progress_callback({
                    "phase": "downloading-audio",
                    "percent": round(percent, 1),
                    "downloaded_bytes": int(downloaded),
                    "total_bytes": int(total),
                    "speed_bps": d.get("speed"),
                    "eta_secs": d.get("eta"),
                })
            except Exception:  # pragma: no cover
                logger.exception("download_audio progress_callback failed")

        ydl_opts["progress_hooks"] = [_hook]

    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(url, download=True)
        audio_file = ydl.prepare_filename(info)
        if not audio_file.endswith(".mp3"):
            audio_file = os.path.splitext(audio_file)[0] + ".mp3"

    size_bytes = os.path.getsize(audio_file) if os.path.exists(audio_file) else 0
    logger.info(
        "download_audio done id=%s elapsed_s=%.2f size_kb=%.1f",
        info.get("id", "?"),
        time.perf_counter() - download_start,
        size_bytes / 1024,
    )
    return audio_file


def transcribe_audio(
    audio_path: str, model_size: str, use_timestamps: bool
) -> dict:
    """Transcribe *audio_path* using faster-whisper and return the result dict.

    The result dict has the shape::

        {
            "language": <str>,
            "plain": <str>,
            "timestamped": <str|None>,
            "srt": <str>,
        }

    ``timestamped`` is None when ``use_timestamps`` is False.
    """
    logger.info(
        "transcribe_audio start path=%s model=%s timestamps=%s",
        audio_path,
        model_size,
        use_timestamps,
    )
    transcribe_start = time.perf_counter()

    model = _get_model(model_size)
    segments, info = model.transcribe(
        audio_path,
        beam_size=1,
        word_timestamps=use_timestamps,
        vad_filter=True,
        language=None,
    )

    plain_lines = []
    timestamped_lines = []
    srt_parts = []

    for i, segment in enumerate(segments, 1):
        text = segment.text.strip()
        if not text:
            continue

        plain_lines.append(text)

        if use_timestamps:
            ts = format_timestamp(segment.start)
            timestamped_lines.append(f"[{ts}] {text}")

        start_str = format_srt_timestamp(segment.start)
        end_str = format_srt_timestamp(segment.end)
        srt_parts.append(f"{i}\n{start_str} --> {end_str}\n{text}\n")

    logger.info(
        "transcribe_audio done language=%s segments=%d elapsed_s=%.2f",
        info.language,
        len(plain_lines),
        time.perf_counter() - transcribe_start,
    )

    return {
        "language": info.language,
        "plain": "\n".join(plain_lines),
        "timestamped": "\n".join(timestamped_lines) if use_timestamps else None,
        "srt": "\n".join(srt_parts),
    }


def transcribe_url(
    url: str,
    model_size: str,
    use_timestamps: bool,
    output_dir: str,
    *,
    desktop_mode: bool = False,
) -> dict:
    """Convenience: download audio from *url* into *output_dir* then transcribe.

    Caller owns *output_dir* lifecycle (use ``tempfile.TemporaryDirectory`` or
    a sidecar-managed temp dir). Both audio download and transcription use
    ``desktop_mode`` to decide whether to apply server-side anti-blocking
    workarounds.
    """
    audio_path = download_audio(url, output_dir, desktop_mode=desktop_mode)
    return transcribe_audio(audio_path, model_size, use_timestamps)