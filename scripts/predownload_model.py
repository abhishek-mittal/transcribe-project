"""Pre-download faster-whisper weights into api/_models/<model>/.

Stores files flat (no HuggingFace blobs/snapshots/symlinks structure) so the
VPS deployment can load the model from disk without any cold-start download.
Run once on the VPS during deploy (see deploy/setup.sh).

Usage:
    python scripts/predownload_model.py            # downloads DEFAULT model
    MODEL=tiny python scripts/predownload_model.py # override
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODELS_ROOT = ROOT / "api" / "_models"
MODEL = os.environ.get("MODEL", "tiny")

# faster-whisper publishes CTranslate2-converted checkpoints under Systran/.
HF_REPOS = {
    "tiny": "Systran/faster-whisper-tiny",
    "base": "Systran/faster-whisper-base",
    "small": "Systran/faster-whisper-small",
}


def main() -> int:
    if MODEL not in HF_REPOS:
        print(
            f"[predownload] ERROR: unknown MODEL={MODEL!r}. Known: {sorted(HF_REPOS)}",
            file=sys.stderr,
        )
        return 1

    target_dir = MODELS_ROOT / MODEL
    target_dir.mkdir(parents=True, exist_ok=True)
    print(f"[predownload] model={MODEL} repo={HF_REPOS[MODEL]} -> {target_dir}")

    try:
        from huggingface_hub import snapshot_download
    except ImportError as e:
        print(f"[predownload] ERROR: {e}. Install requirements first.", file=sys.stderr)
        return 1

    # Flat layout (no symlinks/blobs) keeps the on-disk footprint minimal.
    snapshot_download(
        repo_id=HF_REPOS[MODEL],
        local_dir=str(target_dir),
        # Only the files faster-whisper actually needs at runtime.
        allow_patterns=[
            "model.bin",
            "config.json",
            "tokenizer.json",
            "vocabulary.*",
            "preprocessor_config.json",
        ],
    )

    # snapshot_download leaves a `.cache/` dir with download metadata. Strip
    # it so the deployed footprint is minimal.
    cache_dir = target_dir / ".cache"
    if cache_dir.exists():
        import shutil

        shutil.rmtree(cache_dir)

    total = sum(p.stat().st_size for p in target_dir.rglob("*") if p.is_file())
    print(f"[predownload] done. size={total / 1e6:.1f} MB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
