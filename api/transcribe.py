"""Flask API for the transcribe-project (reference / future hosted path).

The transcription logic lives in ``api/transcribe_core.py`` so the new Tauri
sidecar (``api/sidecar.py``) can reuse it. This file is a thin Flask wrapper
that exposes the same SSE streaming endpoint and a JSON endpoint over HTTP.

Runs under Gunicorn on the Vultr VPS (see deploy/transcribe-api.service).
Local dev: ``python scripts/dev_api.py`` (Vite proxies /api/* to it).
"""

import json
import logging
import tempfile
import time
import traceback
import uuid

from flask import Flask, Response, jsonify, request, stream_with_context

from api.transcribe_core import (
    DEFAULT_MODEL,
    VALID_MODELS,
    download_audio,
    format_timestamp,
    logger,
    transcribe_audio,
    _get_model,  # used by the SSE stream endpoint below
)


# ---------------------------------------------------------------------------
# Flask app
# ---------------------------------------------------------------------------
app = Flask(__name__)


@app.after_request
def add_cors_headers(response):
    """Allow cross-origin requests in local dev (Vite on :5173, API on :8787).
    In production both are served from the same Nginx origin so CORS is a no-op.
    """
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
            request_id,
            model_size,
            use_timestamps,
            url or "<missing>",
        )

        if not url:
            return jsonify({"error": "url is required"}), 400

        if model_size not in VALID_MODELS:
            return jsonify(
                {"error": f"Invalid model. Valid options: {sorted(VALID_MODELS)}"}
            ), 400

        with tempfile.TemporaryDirectory() as tmp_dir:
            audio_path = download_audio(url, tmp_dir, desktop_mode=False)
            result = transcribe_audio(audio_path, model_size, use_timestamps)

        logger.info(
            "rid=%s completed status=200 total_s=%.2f language=%s",
            request_id,
            time.perf_counter() - request_start,
            result.get("language"),
        )
        return jsonify(result), 200

    except Exception as e:
        logger.exception(
            "rid=%s failed model=%s url=%s total_s=%.2f",
            request_id,
            model_size,
            url or "<missing>",
            time.perf_counter() - request_start,
        )
        return jsonify({"error": str(e), "trace": traceback.format_exc()}), 500


@app.route("/api/transcribe/stream", methods=["POST", "OPTIONS"])
def handle_transcribe_stream():
    """SSE streaming endpoint — yields transcription segments as they're produced."""
    if request.method == "OPTIONS":
        return "", 204

    request_id = uuid.uuid4().hex[:8]
    data = request.get_json(force=True, silent=True) or {}
    url_value = (data.get("url") or "").strip()
    model_size = data.get("model", DEFAULT_MODEL)
    use_timestamps = bool(data.get("timestamps", True))

    if not url_value:
        return jsonify({"error": "url is required"}), 400
    if model_size not in VALID_MODELS:
        return jsonify(
            {"error": f"Invalid model. Valid options: {sorted(VALID_MODELS)}"}
        ), 400

    def generate():
        def sse(event: str, payload: dict) -> str:
            return f"event: {event}\ndata: {json.dumps(payload)}\n\n"

        logger.info(
            "stream rid=%s model=%s url=%s", request_id, model_size, url_value
        )
        try:
            yield sse("status", {"phase": "downloading"})
            with tempfile.TemporaryDirectory() as tmp_dir:
                audio_path = download_audio(
                    url_value, tmp_dir, desktop_mode=False
                )
                yield sse("status", {"phase": "transcribing"})

                model = _get_model(model_size)
                segments, info = model.transcribe(
                    audio_path,
                    beam_size=1,
                    word_timestamps=False,
                    vad_filter=True,
                    language=None,
                )

                seg_index = 0
                for segment in segments:
                    text = segment.text.strip()
                    if not text:
                        continue
                    seg_index += 1
                    payload: dict = {
                        "index": seg_index,
                        "text": text,
                        "start": round(segment.start, 2),
                        "end": round(segment.end, 2),
                    }
                    if use_timestamps:
                        payload["ts"] = format_timestamp(segment.start)
                    yield sse("segment", payload)

                logger.info(
                    "stream rid=%s done language=%s segments=%d",
                    request_id,
                    info.language,
                    seg_index,
                )
                yield sse("done", {"language": info.language})
        except Exception as e:
            logger.exception("stream rid=%s failed", request_id)
            yield sse("error", {"error": str(e)})

    return Response(
        stream_with_context(generate()),
        content_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
        },
    )


@app.route("/api/health", methods=["GET"])
def handle_health():
    return jsonify({"status": "ok", "model": DEFAULT_MODEL}), 200


if __name__ == "__main__":
    app.run(host="127.0.0.1", port=8000, debug=False)