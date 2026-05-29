"""Local dev server for the Python API.

Runs the Flask app from api/transcribe.py so the frontend can hit
/api/transcribe during `vite dev` via the Vite proxy (see vite.config.js).
Production uses Gunicorn (see deploy/transcribe-api.service).

Usage:
    python3 scripts/dev_api.py           # listens on 127.0.0.1:8787
    PORT=9000 python3 scripts/dev_api.py # custom port
"""

from __future__ import annotations

import logging
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

from api.transcribe import app  # noqa: E402

logger = logging.getLogger("dev-api")


def main() -> None:
    port = int(os.environ.get("PORT", "8787"))
    host = os.environ.get("HOST", "127.0.0.1")
    logger.info("Listening on http://%s:%d (proxied at /api/*)", host, port)
    app.run(host=host, port=port, debug=False)


if __name__ == "__main__":
    main()
