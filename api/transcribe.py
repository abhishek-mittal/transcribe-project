"""Flask API for the transcribe-project.

Runs under Gunicorn on the Vultr VPS (see deploy/transcribe-api.service).
Local dev: `python scripts/dev_api.py` (Vite proxies /api/* to it).
"""

import logging
import os
import shutil
import tempfile
import time
import traceback
import uuid
from datetime import timedelta
from urllib.parse import urlparse

import yt_dlp
from faster_whisper import WhisperModel
from flask import Flask, jsonify, request


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
# Flask app
# ---------------------------------------------------------------------------
app = Flask(__name__)

# Cache loaded WhisperModel instances by (size, device, compute_type).
# Gunicorn workers are long-lived, so this avoids reloading on every request.
_MODEL_CACHE: dict[tuple[str, str, str], WhisperModel] = {}


def _resolve_model_source(model_size: str) -> str:
    """Return a local directory if pre-downloaded, else the bare model name."""
    local = os.path.join(MODELS_DIR, model_size)
    if os.path.isfile(os.path.join(local, "model.bin")):
        return local
    return model_size


def _get_model(model_size: str, device: str = "cpu", compute_type: str = "int8") -> WhisperModel:
    key = (model_size, device, compute_type)
    model = _MODEL_CACHE.get(key)
    if model is None:
        logger.info(
            "Loading WhisperModel size=%s device=%s compute_type=%s (cache miss)",
            model_size, device, compute_type,
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


def _is_youtube_url(url: str) -> bool:
    try:
        host = (urlparse(url).hostname or "").lower()
    except ValueError:
        return False
    return host in _YOUTUBE_HOSTS


def download_audio(url: str, output_dir: str) -> str:
    logger.info("download_audio start url=%s", url)
    download_start = time.perf_counter()

    is_youtube = _is_youtube_url(url)

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

    # The cookies, player-client fallback chain, and PO-token plugin are all
    # YouTube-specific workarounds for its datacenter-IP bot challenge. Do not
    # apply them to other hosts: a YouTube cookies.txt would leak Google
    # session cookies to e.g. Vimeo / SoundCloud / podcast hosts on every
    # request, and the extractor_args/player_client knobs are no-ops there.
    if is_youtube:
        cookies_file = os.environ.get("YT_DLP_COOKIES_FILE")
        if cookies_file and os.path.exists(cookies_file):
            ydl_opts["cookiefile"] = cookies_file

        # Trying multiple player clients lets yt-dlp fall back when one client
        # is bot-challenged. The bgutil PO-token plugin (installed via pip on
        # the VPS) auto-engages when present and provides a Proof-of-Origin
        # token that bypasses many YouTube bot checks.
        # YT_DLP_PLAYER_CLIENTS (comma-separated) overrides the default list.
        player_clients = [
            c.strip()
            for c in os.environ.get(
                "YT_DLP_PLAYER_CLIENTS", "default,tv,ios"
            ).split(",")
            if c.strip()
        ]
        ydl_opts["extractor_args"] = {"youtube": {"player_client": player_clients}}

    # Optional outbound proxy (residential / mobile) to dodge datacenter-IP
    # gates. Applies to all hosts because a flagged Vultr IP affects every
    # extractor, not just YouTube.
    proxy = os.environ.get("YT_DLP_PROXY")
    if proxy:
        ydl_opts["proxy"] = proxy

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


def transcribe_audio(audio_path: str, model_size: str, use_timestamps: bool) -> dict:
    logger.info(
        "transcribe_audio start path=%s model=%s timestamps=%s",
        audio_path, model_size, use_timestamps,
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
        info.language, len(plain_lines), time.perf_counter() - transcribe_start,
    )

    return {
        "language": info.language,
        "plain": "\n".join(plain_lines),
        "timestamped": "\n".join(timestamped_lines) if use_timestamps else None,
        "srt": "\n".join(srt_parts),
    }


@app.after_request
def add_cors_headers(response):
    """Allow cross-origin requests in local dev (Vite on :5173, API on :8787).
    In production both are served from the same Nginx origin so CORS is a no-op."""
    response.headers["Access-Control-Allow-Origin"] = "*"
    response.headers["Access-Control-Allow-Methods"] = "POST, OPTIONS"
    response.headers["Access-Control-Allow-Headers"] = "Content-Type"
    return response


@app.route("/api/transcribe", methods=["POST", "OPTIONS"])
def handle_transcribe():
    if request.method == "OPTIONS":
        return "", 204

    request_id = uuid.uuid4().hex[:8]
    request_start = time.perf_counter()
    url = ""
    model_size = "?"

    try:
        data = request.get_json(force=True, silent=True) or {}

        url = (data.get("url") or "").strip()
        model_size = data.get("model", DEFAULT_MODEL)
        use_timestamps = bool(data.get("timestamps", True))

        logger.info(
            "POST /api/transcribe rid=%s model=%s timestamps=%s url=%s",
            request_id, model_size, use_timestamps, url or "<missing>",
        )

        if not url:
            return jsonify({"error": "url is required"}), 400

        if model_size not in VALID_MODELS:
            return jsonify(
                {"error": f"Invalid model. Valid options: {sorted(VALID_MODELS)}"}
            ), 400

        with tempfile.TemporaryDirectory() as tmp_dir:
            audio_path = download_audio(url, tmp_dir)
            result = transcribe_audio(audio_path, model_size, use_timestamps)

        logger.info(
            "rid=%s completed status=200 total_s=%.2f language=%s",
            request_id, time.perf_counter() - request_start, result.get("language"),
        )
        return jsonify(result), 200

    except Exception as e:
        logger.exception(
            "rid=%s failed model=%s url=%s total_s=%.2f",
            request_id, model_size, url or "<missing>",
            time.perf_counter() - request_start,
        )
        return jsonify({"error": str(e), "trace": traceback.format_exc()}), 500


@app.route("/api/health", methods=["GET"])
def handle_health():
    return jsonify({"status": "ok", "model": DEFAULT_MODEL}), 200


if __name__ == "__main__":
    app.run(host="127.0.0.1", port=8000, debug=False)
