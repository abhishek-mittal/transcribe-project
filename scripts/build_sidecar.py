"""Build a standalone PyInstaller binary for the Tauri sidecar.

Usage::

    python scripts/build_sidecar.py [--arch aarch64|universal|x86_64]

Output:
    src-tauri/binaries/transcribe-sidecar-<target-triple>

The target triple matches Tauri's sidecar naming convention:
    aarch64-apple-darwin  (Apple silicon)
    x86_64-apple-darwin   (Intel macOS)
    universal2            (universal binary via lipo)
"""

from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fetch_ffmpeg import fetch_ffmpeg_binaries  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
SRC_TAURI_BINARIES = REPO_ROOT / "src-tauri" / "binaries"
SIDECAR_NAME = "transcribe-sidecar"


def detect_target_triple(arch: str | None) -> str:
    """Map --arch to a Tauri target triple."""
    if arch == "universal":
        return "universal-apple-darwin"
    if arch in ("aarch64", "arm64"):
        return "aarch64-apple-darwin"
    if arch in ("x86_64", "amd64"):
        return "x86_64-apple-darwin"
    # Auto-detect from host
    machine = platform.machine().lower()
    if machine in ("arm64", "aarch64"):
        return "aarch64-apple-darwin"
    return "x86_64-apple-darwin"


def ensure_pyinstaller() -> None:
    try:
        import PyInstaller  # noqa: F401
    except ImportError:
        print("PyInstaller not found; installing…", file=sys.stderr)
        subprocess.run(
            [sys.executable, "-m", "pip", "install", "pyinstaller"],
            check=True,
        )


def build(target_triple: str) -> Path:
    """Invoke PyInstaller and place the binary in src-tauri/binaries/."""
    ensure_pyinstaller()

    SRC_TAURI_BINARIES.mkdir(parents=True, exist_ok=True)
    output_name = f"{SIDECAR_NAME}-{target_triple}"
    output_path = SRC_TAURI_BINARIES / output_name

    # Build a .spec file or pass args directly. Args are clearer for v1.
    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        # --onedir extracts the bundle at build time, so each invocation
        # just exec()s the unpacked binary instead of paying ~25s to
        # re-extract the onefile bundle every time. The Tauri sidecar
        # resolution uses the resource path (see tauri.conf.json
        # `bundle.resources`) and points at the inner `transcribe-sidecar`
        # binary directly.
        "--onedir",
        "--name",
        SIDECAR_NAME,
        # Hidden imports: yt-dlp loads ~300 extractors dynamically.
        # faster-whisper pulls CTranslate2; av pulls FFmpeg bindings; etc.
        "--collect-all",
        "yt_dlp",
        "--collect-all",
        "faster_whisper",
        "--collect-all",
        "av",
        "--collect-all",
        "huggingface_hub",
        "--collect-all",
        "tokenizers",
        # The sidecar is launched as a Tauri external binary; its working
        # directory may not be the repo root, so add the api/ dir to the
        # path so `from api.sidecar import ...` works inside the bundle.
        "--paths",
        str(REPO_ROOT),
        # Strip debug symbols to reduce size.
        "--noconfirm",
        "--clean",
        str(REPO_ROOT / "api" / "sidecar.py"),
    ]

    print(f"Building sidecar binary for {target_triple}…", file=sys.stderr)
    print("$ " + " ".join(cmd), file=sys.stderr)
    subprocess.run(cmd, check=True, cwd=REPO_ROOT)

    # --onedir produces dist/<name>/ (a directory containing the binary
    # plus its dynamic-library dependencies). Move the whole directory
    # next to tauri.conf.json so Tauri's `bundle.resources` config can
    # bundle it into the app and the Rust sidecar code can resolve the
    # inner binary via the resource path.
    built_dir = REPO_ROOT / "dist" / SIDECAR_NAME
    if not built_dir.is_dir():
        raise FileNotFoundError(f"PyInstaller --onedir output not found at {built_dir}")

    if output_path.exists() or output_path.is_symlink():
        if output_path.is_dir() and not output_path.is_symlink():
            shutil.rmtree(output_path)
        else:
            output_path.unlink()
    shutil.move(str(built_dir), str(output_path))

    inner_binary = output_path / SIDECAR_NAME
    if sys.platform != "win32" and inner_binary.exists():
        inner_binary.chmod(0o755)

    # Bundle ffmpeg/ffprobe alongside the sidecar executable so the app works
    # without requiring `brew install ffmpeg` on the host machine. Placed
    # inside the same --onedir directory so Tauri's existing `bundle.resources`
    # entry for this directory picks them up with no separate resource entry.
    if sys.platform != "win32":
        fetch_ffmpeg_binaries(output_path)

    print(
        f"Built: {output_path}/ ({inner_binary.stat().st_size:,} byte inner binary)",
        file=sys.stderr,
    )
    return output_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--arch",
        choices=["aarch64", "arm64", "x86_64", "amd64", "universal"],
        help="Target architecture. Default: auto-detect from host.",
    )
    args = parser.parse_args()
    triple = detect_target_triple(args.arch)
    build(triple)
    return 0


if __name__ == "__main__":
    sys.exit(main())