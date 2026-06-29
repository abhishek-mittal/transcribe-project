"""Test for the platform.mac_ver() workaround (FIX-10).

Root cause (confirmed via real traceback from a user's M2 Mac, captured by
running the frozen app directly from Terminal):

    platform.mac_ver()[0] can return a malformed version string (e.g. one
    with an empty/blank segment) on some macOS versions. yt_dlp's plugin
    discovery (yt_dlp/update.py:_get_variant_and_executable_path, called
    unconditionally on every YoutubeDL() construction) calls
    version_tuple(platform.mac_ver()[0]) WITHOUT yt-dlp's own
    lenient=True option, so int('') raises ValueError, surfacing as
    "invalid literal for int() with base 10: ''" on every single
    transcription/download attempt — not specific to any URL or platform.

Fix: api/sidecar.py patches platform.mac_ver() at import time to always
return a value version_tuple can parse, regardless of what the OS reports.
"""

from __future__ import annotations

import sys
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))


def test_mac_ver_patch_normalizes_malformed_version_string():
    import platform

    import api.sidecar  # noqa: F401  (import triggers the patch)

    with mock.patch("platform.system", return_value="Darwin"):
        # Simulate the real-world malformed value seen on the affected
        # machine: a release string with an empty/blank component.
        with mock.patch(
            "api.sidecar._real_mac_ver", return_value=("", ("", "", ""), "arm64")
        ):
            release, versioninfo, machine = platform.mac_ver()

    # Must not raise, and must be safely parseable by yt-dlp's version_tuple.
    from yt_dlp.utils._utils import version_tuple

    version_tuple(release)  # must not raise


def test_mac_ver_patch_passes_through_valid_version_string():
    import platform

    import api.sidecar  # noqa: F401

    with mock.patch(
        "api.sidecar._real_mac_ver", return_value=("14.5", ("", "", ""), "arm64")
    ):
        release, _, _ = platform.mac_ver()

    assert release == "14.5"
