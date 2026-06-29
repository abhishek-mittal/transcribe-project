"""Tests confirming download_audio threads ffmpeg_location into yt-dlp's ydl_opts.

Covers the new requirement in
openspec/changes/bundle-ffmpeg-sidecar/specs/python-sidecar/spec.md
("yt-dlp ffmpeg invocation"): every YoutubeDL(ydl_opts) call must use the
resolved ffmpeg path, not yt-dlp's own implicit PATH lookup.
"""

from __future__ import annotations

import sys
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from api import transcribe_core


def test_download_audio_sets_ffmpeg_location_in_ydl_opts(tmp_path):
    captured_opts = {}

    class _FakeYoutubeDL:
        def __init__(self, opts):
            captured_opts.update(opts)

        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False

        def extract_info(self, url, download=True):
            return {"id": "abc123", "ext": "mp3"}

        def prepare_filename(self, info):
            path = tmp_path / "abc123.mp3"
            path.write_bytes(b"fake-audio")
            return str(path)

    with mock.patch.object(transcribe_core.yt_dlp, "YoutubeDL", _FakeYoutubeDL):
        transcribe_core.download_audio(
            "https://www.youtube.com/watch?v=abc123",
            str(tmp_path),
            desktop_mode=True,
            ffmpeg_location="/bundled/path/ffmpeg",
        )

    assert captured_opts.get("ffmpeg_location") == "/bundled/path/ffmpeg"


def test_download_audio_omits_ffmpeg_location_when_not_provided(tmp_path):
    captured_opts = {}

    class _FakeYoutubeDL:
        def __init__(self, opts):
            captured_opts.update(opts)

        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False

        def extract_info(self, url, download=True):
            return {"id": "abc123", "ext": "mp3"}

        def prepare_filename(self, info):
            path = tmp_path / "abc123.mp3"
            path.write_bytes(b"fake-audio")
            return str(path)

    with mock.patch.object(transcribe_core.yt_dlp, "YoutubeDL", _FakeYoutubeDL):
        transcribe_core.download_audio(
            "https://www.youtube.com/watch?v=abc123",
            str(tmp_path),
            desktop_mode=False,
        )

    assert "ffmpeg_location" not in captured_opts
