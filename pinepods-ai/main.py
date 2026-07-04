"""PinePods AI sidecar — optional, stateless transcription service.

Modeled on Immich's machine-learning container: a separate, optional service the
main PinePods API talks to over HTTP. If it isn't running (or PINEPODS_AI_URL is
unset), the API simply disables AI-based features. It holds no database access.

Currently exposes speech-to-text via faster-whisper (#726). Audio is read from a
shared, read-only mount of the downloads directory; the API sends a file path and
gets back the transcript text plus segment-level timestamps.

Endpoints:
  GET  /health      -> readiness + loaded model info
  POST /transcribe  -> { file_path, language? } -> { language, text, segments[], model, duration }
"""

import json
import logging
import os
import threading
import time
from typing import Optional

from fastapi import FastAPI, HTTPException, Header
from fastapi.responses import StreamingResponse
from pydantic import BaseModel

logging.basicConfig(level=logging.INFO)
log = logging.getLogger("pinepods-ai")

# --- Configuration (all via env, sane defaults for CPU-only hosts) ---
MODEL_NAME = os.getenv("WHISPER_MODEL", "base")
DEVICE = os.getenv("WHISPER_DEVICE", "cpu")
COMPUTE_TYPE = os.getenv("WHISPER_COMPUTE_TYPE", "int8")
BEAM_SIZE = int(os.getenv("WHISPER_BEAM_SIZE", "5"))
# Only files under this base may be transcribed (prevents path-traversal reads).
ALLOWED_BASE = os.path.realpath(os.getenv("PINEPODS_AI_MEDIA_BASE", "/opt/pinepods/downloads"))
# Optional shared secret; when set, callers must send it as the X-AI-Token header.
AUTH_TOKEN = os.getenv("PINEPODS_AI_TOKEN")

app = FastAPI(title="PinePods AI", version="0.1.0")

# The Whisper model is loaded lazily on first use and cached process-wide. Loading
# can take several seconds and pull model weights, so we never do it at import time.
_model = None
_model_lock = threading.Lock()


def _get_model():
    global _model
    if _model is None:
        with _model_lock:
            if _model is None:
                from faster_whisper import WhisperModel  # imported lazily

                log.info(
                    "Loading Whisper model '%s' (device=%s, compute_type=%s)",
                    MODEL_NAME, DEVICE, COMPUTE_TYPE,
                )
                _model = WhisperModel(MODEL_NAME, device=DEVICE, compute_type=COMPUTE_TYPE)
                log.info("Whisper model loaded")
    return _model


class TranscribeRequest(BaseModel):
    file_path: str
    # ISO language code to force; None = auto-detect.
    language: Optional[str] = None


class Segment(BaseModel):
    start: float
    end: float
    text: str


class TranscribeResponse(BaseModel):
    language: str
    text: str
    segments: list[Segment]
    model: str
    duration: float


def _check_auth(token: Optional[str]):
    if AUTH_TOKEN and token != AUTH_TOKEN:
        raise HTTPException(status_code=401, detail="Invalid AI token")


def _resolve_media_path(file_path: str) -> str:
    """Resolve and confine the requested path to ALLOWED_BASE."""
    real = os.path.realpath(file_path)
    if not (real == ALLOWED_BASE or real.startswith(ALLOWED_BASE + os.sep)):
        raise HTTPException(status_code=400, detail="file_path outside allowed media directory")
    if not os.path.isfile(real):
        raise HTTPException(status_code=404, detail="file not found")
    return real


@app.get("/health")
def health():
    return {
        "status": "ok",
        "model": MODEL_NAME,
        "device": DEVICE,
        "compute_type": COMPUTE_TYPE,
        "model_loaded": _model is not None,
    }


@app.post("/transcribe")
def transcribe(req: TranscribeRequest, x_ai_token: Optional[str] = Header(default=None)):
    """Stream transcription progress as newline-delimited JSON (NDJSON).

    Whisper decodes lazily as the segment generator is consumed, so we can emit a
    `{"type":"progress","progress":<0..1>}` line after each segment and a final
    `{"type":"result", ...}` line with the full transcript. This lets the caller show a
    live percentage for long episodes instead of a single blocking request.
    """
    _check_auth(x_ai_token)
    path = _resolve_media_path(req.file_path)

    def stream():
        model = _get_model()
        started = time.time()
        log.info("Transcribing %s", path)
        try:
            segments_iter, info = model.transcribe(path, language=req.language, beam_size=BEAM_SIZE)
            total = float(info.duration or 0.0)
            segments: list[dict] = []
            text_parts: list[str] = []
            for seg in segments_iter:
                piece = seg.text.strip()
                segments.append({"start": round(seg.start, 3), "end": round(seg.end, 3), "text": piece})
                text_parts.append(piece)
                progress = min(seg.end / total, 1.0) if total > 0 else 0.0
                yield json.dumps({"type": "progress", "progress": round(progress, 4)}) + "\n"

            log.info(
                "Transcribed %s in %.1fs (%d segments, lang=%s)",
                path, time.time() - started, len(segments), info.language,
            )
            yield json.dumps({
                "type": "result",
                "language": info.language or (req.language or "unknown"),
                "text": " ".join(text_parts).strip(),
                "segments": segments,
                "model": MODEL_NAME,
                "duration": round(total, 3),
            }) + "\n"
        except Exception as e:  # surface mid-stream failures to the caller
            log.exception("Transcription failed for %s", path)
            yield json.dumps({"type": "error", "error": str(e)}) + "\n"

    return StreamingResponse(stream(), media_type="application/x-ndjson")
