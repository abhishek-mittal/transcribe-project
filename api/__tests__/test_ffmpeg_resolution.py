"""Tests for sidecar ffmpeg/ffprobe path resolution (bundle-ffmpeg-sidecar).

Covers the three cases from openspec/changes/bundle-ffmpeg-sidecar/specs/python-sidecar/spec.md:
- bundled path found (frozen PyInstaller build)
- bundled path missing, falls back to PATH (dev mode)
- neither found -> FFMPEG_MISSING
"""

from __future__ import annotations

import sys
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from api.sidecar import ValidationError, resolve_ffmpeg


def test_resolve_ffmpeg_uses_bundled_path_when_frozen(tmp_path):
    bundled_ffmpeg = tmp_path / "ffmpeg"
    bundled_ffmpeg.write_text("#!/bin/sh\necho fake-ffmpeg\n")
    bundled_ffmpeg.chmod(0o755)

    with mock.patch.object(sys, "frozen", True, create=True), mock.patch.object(
        sys, "executable", str(tmp_path / "transcribe-sidecar")
    ):
        resolved = resolve_ffmpeg()

    assert resolved == str(bundled_ffmpeg)


def test_resolve_ffmpeg_falls_back_to_path_when_not_frozen():
    with mock.patch("shutil.which", return_value="/opt/homebrew/bin/ffmpeg"):
        resolved = resolve_ffmpeg()

    assert resolved == "/opt/homebrew/bin/ffmpeg"


def test_resolve_ffmpeg_raises_when_neither_found():
    with mock.patch("shutil.which", return_value=None):
        with mock.patch.object(sys, "frozen", False, create=True):
            try:
                resolve_ffmpeg()
                assert False, "expected ValidationError"
            except ValidationError as exc:
                assert exc.code == "FFMPEG_MISSING"
