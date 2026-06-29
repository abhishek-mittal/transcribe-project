"""Download pinned static ffmpeg/ffprobe binaries for bundling into the sidecar.

Source: evermeet.cx (https://evermeet.cx/ffmpeg/) — long-standing static macOS
ffmpeg builds. URLs below embed the exact version, not the floating
`/getrelease/` redirect, so this script always fetches the same bytes.

Builds are x86_64 only (evermeet.cx has no native arm64 build); they run on
Apple Silicon via Rosetta 2, which ships standard on all Apple Silicon Macs.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

FFMPEG_VERSION = "8.1.2"
_BASE_URL = "https://evermeet.cx/ffmpeg"
SOURCES = {
    "ffmpeg": f"{_BASE_URL}/ffmpeg-{FFMPEG_VERSION}.zip",
    "ffprobe": f"{_BASE_URL}/ffprobe-{FFMPEG_VERSION}.zip",
}


def _download(url: str, dest: Path) -> None:
    print(f"Downloading {url}…", file=sys.stderr)
    with urllib.request.urlopen(url, timeout=60) as resp, open(dest, "wb") as fh:
        shutil.copyfileobj(resp, fh)


def _verify_runs(binary_path: Path) -> None:
    """Fail loudly if the extracted binary doesn't actually execute."""
    result = subprocess.run(
        [str(binary_path), "-version"],
        capture_output=True,
        text=True,
        timeout=15,
    )
    if result.returncode != 0 or "ffmpeg version" not in result.stdout.lower() and "ffprobe version" not in result.stdout.lower():
        raise RuntimeError(
            f"{binary_path} did not run correctly (exit {result.returncode}): "
            f"{result.stderr.strip() or result.stdout.strip()}"
        )


def fetch_ffmpeg_binaries(dest_dir: Path) -> None:
    """Download, extract, and verify ffmpeg + ffprobe into *dest_dir*.

    Raises on any failure — a sidecar build must never silently ship without
    a working ffmpeg/ffprobe pair.
    """
    dest_dir.mkdir(parents=True, exist_ok=True)

    for name, url in SOURCES.items():
        zip_path = dest_dir / f"{name}.zip"
        try:
            _download(url, zip_path)
            with zipfile.ZipFile(zip_path) as zf:
                zf.extract(name, dest_dir)
        except Exception as exc:
            raise RuntimeError(f"failed to fetch bundled {name} from {url}: {exc}") from exc
        finally:
            zip_path.unlink(missing_ok=True)

        binary_path = dest_dir / name
        binary_path.chmod(0o755)
        _verify_runs(binary_path)
        print(f"Verified bundled {name} at {binary_path}", file=sys.stderr)


if __name__ == "__main__":
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("./ffmpeg-bundle")
    fetch_ffmpeg_binaries(target)
