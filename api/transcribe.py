import json
import os
import tempfile
import traceback
from http.server import BaseHTTPRequestHandler
from datetime import timedelta

import yt_dlp
from faster_whisper import WhisperModel


# ---------------------------------------------------------------------------
# NOTE ON VERCEL LIMITS
# ---------------------------------------------------------------------------
# Vercel serverless functions have a max execution time of 60s (Pro) / 10s (Hobby).
# faster-whisper models range from ~150MB (small) to ~3GB (large-v3).
# For production use with longer videos, consider:
#   - A dedicated server (Railway, Fly.io, Render)
#   - Async job queue (e.g. Vercel + upstash + worker)
#   - A managed speech API (Deepgram, AssemblyAI) as a lighter alternative
# ---------------------------------------------------------------------------

VALID_MODELS = {"small", "medium", "large-v3-turbo", "large-v3"}


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


def download_audio(url: str, output_dir: str) -> str:
    ydl_opts = {
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
        "http_headers": {
            "User-Agent": (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                "AppleWebKit/537.36 (KHTML, like Gecko) "
                "Chrome/120.0.0.0 Safari/537.36"
            )
        },
    }

    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(url, download=True)
        audio_file = ydl.prepare_filename(info)
        if not audio_file.endswith(".mp3"):
            audio_file = os.path.splitext(audio_file)[0] + ".mp3"
        return audio_file


def transcribe_audio(audio_path: str, model_size: str, use_timestamps: bool) -> dict:
    model = WhisperModel(model_size, device="cpu", compute_type="int8")
    segments, info = model.transcribe(
        audio_path,
        beam_size=5,
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

    return {
        "language": info.language,
        "plain": "\n".join(plain_lines),
        "timestamped": "\n".join(timestamped_lines) if use_timestamps else None,
        "srt": "\n".join(srt_parts),
    }


class handler(BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            data = json.loads(body)

            url = data.get("url", "").strip()
            model_size = data.get("model", "small")
            use_timestamps = bool(data.get("timestamps", True))

            if not url:
                return self._send_json(400, {"error": "url is required"})

            if model_size not in VALID_MODELS:
                return self._send_json(
                    400,
                    {"error": f"Invalid model. Valid options: {sorted(VALID_MODELS)}"},
                )

            with tempfile.TemporaryDirectory() as tmp_dir:
                audio_path = download_audio(url, tmp_dir)
                result = transcribe_audio(audio_path, model_size, use_timestamps)

            self._send_json(200, result)

        except json.JSONDecodeError:
            self._send_json(400, {"error": "Invalid JSON body"})
        except Exception as e:
            self._send_json(
                500,
                {
                    "error": str(e),
                    "trace": traceback.format_exc(),
                },
            )

    def do_OPTIONS(self):
        """Handle CORS preflight requests."""
        self.send_response(200)
        self._add_cors_headers()
        self.end_headers()

    def _send_json(self, status: int, data: dict):
        body = json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self._add_cors_headers()
        self.end_headers()
        self.wfile.write(body)

    def _add_cors_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")

    def log_message(self, format, *args):  # noqa: A002
        """Suppress default HTTP request logging."""
        pass
