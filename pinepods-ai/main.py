"""PinePods AI sidecar — optional, stateless AI service.

Modeled on Immich's machine-learning container: a separate, optional service the
main PinePods API talks to over HTTP. If it isn't running (or PINEPODS_AI_URL is
unset), the API simply disables AI-based features. It holds no database access.

Capabilities:
  - Speech-to-text via faster-whisper (#726). Audio is read from a shared, read-only
    mount of the downloads directory; the API sends a file path and gets back the
    transcript text plus segment-level timestamps.
  - Ad / sponsor detection via an LLM (#790). The API sends the already-generated
    transcript segments; a local GGUF model (llama.cpp) or a remote OpenAI-compatible
    endpoint labels the ad spans, which map back to audio time ranges.
  - Model management: list installed models and pull new ones.

The active models are chosen by the PinePods API (stored in its DB) and passed on each
request, so this service stays stateless and caches whatever models it's asked for.

Endpoints:
  GET  /health       -> readiness + loaded model info
  POST /transcribe   -> { file_path, language?, model? } -> NDJSON progress + result
  POST /detect_ads   -> { segments[], language?, llm{} }  -> NDJSON progress + result
  GET  /models       -> installed whisper + local GGUFs (+ remote enumerate)
  POST /models/pull  -> download a model, NDJSON progress
"""

import glob
import json
import logging
import os
import re
import shutil
import threading
import time
from typing import Optional

import requests
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
# Where downloaded model files live (GGUFs + whisper cache). Mount a volume here.
MODELS_DIR = os.path.realpath(os.getenv("PINEPODS_AI_MODELS_DIR", os.getenv("HF_HOME", "/models")))
# Default local LLM (GGUF filename under MODELS_DIR) if a request doesn't name one.
LLM_MODEL_DEFAULT = os.getenv("AI_LLM_MODEL", "")
LLM_N_CTX = int(os.getenv("AI_LLM_N_CTX", "8192"))
LLM_MAX_TOKENS = int(os.getenv("AI_LLM_MAX_TOKENS", "1024"))
# Retries for a remote LLM endpoint on 429/5xx (with exponential backoff).
LLM_MAX_RETRIES = int(os.getenv("AI_LLM_MAX_RETRIES", "4"))
# Only files under this base may be transcribed (prevents path-traversal reads).
ALLOWED_BASE = os.path.realpath(os.getenv("PINEPODS_AI_MEDIA_BASE", "/opt/pinepods/downloads"))
# Optional shared secret; when set, callers must send it as the X-AI-Token header.
AUTH_TOKEN = os.getenv("PINEPODS_AI_TOKEN")

# Whisper model sizes faster-whisper can pull on demand (surfaced in /models).
KNOWN_WHISPER_MODELS = [
    "tiny", "tiny.en", "base", "base.en", "small", "small.en",
    "medium", "medium.en", "large-v1", "large-v2", "large-v3", "distil-large-v3",
]

app = FastAPI(title="PinePods AI", version="0.2.0")

# Models are loaded lazily on first use and cached process-wide, keyed by name so the
# service can hold multiple whisper sizes / GGUFs at once. Loading pulls weights and can
# take several seconds, so we never do it at import time.
_whisper_models: dict = {}
_whisper_lock = threading.Lock()
_llama_models: dict = {}
_llama_lock = threading.Lock()


def _get_whisper(name: str, device: str, compute_type: str):
    key = (name, device, compute_type)
    m = _whisper_models.get(key)
    if m is None:
        with _whisper_lock:
            m = _whisper_models.get(key)
            if m is None:
                from faster_whisper import WhisperModel  # imported lazily

                log.info("Loading Whisper model '%s' (device=%s, compute_type=%s)", name, device, compute_type)
                m = WhisperModel(name, device=device, compute_type=compute_type, download_root=MODELS_DIR)
                _whisper_models[key] = m
                log.info("Whisper model '%s' loaded", name)
    return m


def _resolve_gguf_path(model: str) -> str:
    """Resolve a local GGUF name to an absolute path confined to MODELS_DIR."""
    candidate = model if os.path.isabs(model) else os.path.join(MODELS_DIR, model)
    real = os.path.realpath(candidate)
    if not (real == MODELS_DIR or real.startswith(MODELS_DIR + os.sep)):
        raise HTTPException(status_code=400, detail="model path outside allowed models directory")
    if not os.path.isfile(real):
        raise HTTPException(status_code=404, detail=f"local model not found: {model}")
    return real


def _get_llama(model: str):
    m = _llama_models.get(model)
    if m is None:
        with _llama_lock:
            m = _llama_models.get(model)
            if m is None:
                from llama_cpp import Llama  # imported lazily

                path = _resolve_gguf_path(model)
                log.info("Loading GGUF LLM '%s' (n_ctx=%d)", model, LLM_N_CTX)
                m = Llama(model_path=path, n_ctx=LLM_N_CTX, verbose=False)
                _llama_models[model] = m
                log.info("GGUF LLM '%s' loaded", model)
    return m


# --- Request/response models ---

class TranscribeRequest(BaseModel):
    file_path: str
    language: Optional[str] = None  # ISO code to force; None = auto-detect
    model: Optional[str] = None     # whisper model override; None = configured default


class Segment(BaseModel):
    start: float
    end: float
    text: str


class LlmSpec(BaseModel):
    backend: str = "local"          # 'local' (bundled GGUF) | 'remote' (OpenAI-compatible)
    model: Optional[str] = None     # GGUF filename, or remote model id
    url: Optional[str] = None       # remote base URL (e.g. http://ollama:11434/v1)
    api_key: Optional[str] = None


class DetectAdsRequest(BaseModel):
    segments: list[Segment]
    language: Optional[str] = None
    llm: LlmSpec = LlmSpec()


class PullRequest(BaseModel):
    kind: str                        # 'whisper' | 'gguf' | 'ollama'
    model: str                       # whisper size, GGUF filename, or ollama tag
    repo: Optional[str] = None       # HF repo id (for 'gguf')
    filename: Optional[str] = None   # file within the HF repo (for 'gguf')
    url: Optional[str] = None        # remote Ollama base URL (for 'ollama')


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
        "models_dir": MODELS_DIR,
        "whisper_loaded": [f"{n}" for (n, _, _) in _whisper_models.keys()],
        "llm_loaded": list(_llama_models.keys()),
    }


@app.post("/transcribe")
def transcribe(req: TranscribeRequest, x_ai_token: Optional[str] = Header(default=None)):
    """Stream transcription progress as newline-delimited JSON (NDJSON).

    Whisper decodes lazily as the segment generator is consumed, so we emit a
    `{"type":"progress","progress":<0..1>}` line after each segment and a final
    `{"type":"result", ...}` line with the full transcript.
    """
    _check_auth(x_ai_token)
    path = _resolve_media_path(req.file_path)
    model_name = req.model or MODEL_NAME

    def stream():
        model = _get_whisper(model_name, DEVICE, COMPUTE_TYPE)
        started = time.time()
        log.info("Transcribing %s (model=%s)", path, model_name)
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

            log.info("Transcribed %s in %.1fs (%d segments, lang=%s)",
                     path, time.time() - started, len(segments), info.language)
            yield json.dumps({
                "type": "result",
                "language": info.language or (req.language or "unknown"),
                "text": " ".join(text_parts).strip(),
                "segments": segments,
                "model": model_name,
                "duration": round(total, 3),
            }) + "\n"
        except Exception:  # surface mid-stream failures to the caller (detail stays in logs)
            log.exception("Transcription failed for %s", path)
            yield json.dumps({"type": "error", "error": "transcription failed"}) + "\n"

    return StreamingResponse(stream(), media_type="application/x-ndjson")


# --- Ad detection (#790) ---------------------------------------------------------

AD_SYSTEM_PROMPT = (
    "You are an expert at detecting advertisements and sponsor reads in podcast "
    "transcripts. Ads include host-read sponsor spots, promo codes, 'this episode is "
    "brought to you by', dynamically inserted ads, and calls to visit a sponsor's site. "
    "Normal show content, listener mail, and self-promotion of the show's own membership "
    "are NOT ads unless they are a paid sponsor read. Be precise about boundaries."
)


def _chunk_segments(segments: list[dict], max_chars: int = 6000, overlap: int = 6):
    """Split numbered segments into windows bounded by character budget, with a small
    segment overlap so an ad straddling a window boundary is still seen whole."""
    windows = []
    i = 0
    n = len(segments)
    while i < n:
        chars = 0
        j = i
        while j < n and chars < max_chars:
            chars += len(segments[j]["text"]) + 12  # +12 for the index/timestamp prefix
            j += 1
        windows.append((i, j))
        if j >= n:
            break
        i = max(i + 1, j - overlap)
    return windows


def _extract_json(text: str):
    """Best-effort parse of an LLM response into a dict/list, tolerating prose wrappers."""
    text = text.strip()
    try:
        return json.loads(text)
    except Exception:
        pass
    m = re.search(r"\{.*\}", text, re.DOTALL) or re.search(r"\[.*\]", text, re.DOTALL)
    if m:
        try:
            return json.loads(m.group(0))
        except Exception:
            return None
    return None


def _post_with_retry(url: str, headers: dict, payload: dict) -> dict:
    """POST JSON, retrying on 429/5xx with exponential backoff (honouring Retry-After).
    Returns the parsed JSON response, or raises HTTPException on exhaustion."""
    last_status = None
    for attempt in range(LLM_MAX_RETRIES):
        resp = requests.post(url, headers=headers, json=payload, timeout=300)
        if resp.status_code == 429 or resp.status_code >= 500:
            last_status = f"{resp.status_code} {resp.reason}: {resp.text[:200]}"
            if attempt == LLM_MAX_RETRIES - 1:
                break
            retry_after = resp.headers.get("Retry-After", "")
            wait = float(retry_after) if retry_after.replace(".", "", 1).isdigit() else 2.0 ** attempt
            log.warning("LLM endpoint %s; retrying in %.1fs (attempt %d/%d)",
                        last_status, wait, attempt + 1, LLM_MAX_RETRIES)
            time.sleep(min(wait, 30.0))
            continue
        resp.raise_for_status()
        return resp.json()
    raise HTTPException(
        status_code=502,
        detail=f"LLM endpoint rate-limited/unavailable after {LLM_MAX_RETRIES} attempts ({last_status}). "
               f"Check your provider's balance/plan/rate limits or try a lighter model.",
    )


def _llm_chat(spec: LlmSpec, system: str, user: str) -> str:
    """Run a chat completion against the configured backend and return the content."""
    # Remote, OpenAI-compatible (Ollama, LM Studio, OpenAI, z.ai general API, …).
    if spec.backend == "remote":
        if not spec.url:
            raise HTTPException(status_code=400, detail="remote LLM requires a url")
        headers = {"Content-Type": "application/json"}
        if spec.api_key:
            headers["Authorization"] = f"Bearer {spec.api_key}"
        base = spec.url.rstrip("/")
        url = base if base.endswith("/chat/completions") else base + "/chat/completions"
        payload = {
            "model": spec.model or "",
            "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            "temperature": 0.1,
            "max_tokens": LLM_MAX_TOKENS,
        }
        data = _post_with_retry(url, headers, payload)
        return data["choices"][0]["message"]["content"]

    # Anthropic-compatible Messages API (Anthropic, z.ai Coding Plan via /api/anthropic, …).
    if spec.backend == "anthropic":
        if not spec.url:
            raise HTTPException(status_code=400, detail="anthropic LLM requires a url")
        headers = {"Content-Type": "application/json", "anthropic-version": "2023-06-01"}
        if spec.api_key:
            headers["x-api-key"] = spec.api_key
            headers["Authorization"] = f"Bearer {spec.api_key}"  # z.ai accepts either
        base = spec.url.rstrip("/")
        if base.endswith("/messages"):
            url = base
        elif base.endswith("/v1"):
            url = base + "/messages"
        else:
            url = base + "/v1/messages"
        payload = {
            "model": spec.model or "",
            "max_tokens": LLM_MAX_TOKENS,
            "temperature": 0.1,
            "system": system,  # Anthropic takes the system prompt as a top-level field
            "messages": [{"role": "user", "content": user}],
        }
        data = _post_with_retry(url, headers, payload)
        # content is a list of blocks; concatenate the text ones.
        return "".join(b.get("text", "") for b in data.get("content", []) if b.get("type") == "text")

    model = spec.model or LLM_MODEL_DEFAULT
    if not model:
        raise HTTPException(status_code=400, detail="no local LLM model configured")
    llama = _get_llama(model)
    out = llama.create_chat_completion(
        messages=[{"role": "system", "content": system}, {"role": "user", "content": user}],
        temperature=0.1,
        max_tokens=LLM_MAX_TOKENS,
    )
    return out["choices"][0]["message"]["content"]


def _detect_window(spec: LlmSpec, segments: list[dict], lo: int, hi: int):
    """Ask the LLM which segments in [lo, hi) are ads. Returns list of (start,end) secs."""
    lines = []
    for idx in range(lo, hi):
        s = segments[idx]
        lines.append(f"{idx}: [{s['start']:.0f}-{s['end']:.0f}] {s['text']}")
    user = (
        "Below are numbered podcast transcript segments as `index: [start-end] text`.\n"
        "Identify contiguous runs of segments that are advertisements or sponsor reads.\n"
        'Respond with ONLY JSON: {"ads": [{"start_index": N, "end_index": M}]} where each '
        "object is one contiguous ad spanning segment indices N..M inclusive. If there are "
        'no ads, respond {"ads": []}.\n\n' + "\n".join(lines)
    )
    raw = _llm_chat(spec, AD_SYSTEM_PROMPT, user)
    parsed = _extract_json(raw)
    spans = []
    ads = parsed.get("ads") if isinstance(parsed, dict) else parsed
    if isinstance(ads, list):
        for a in ads:
            try:
                si = int(a["start_index"])
                ei = int(a["end_index"])
            except (KeyError, TypeError, ValueError):
                continue
            si, ei = max(lo, min(si, ei)), min(hi - 1, max(si, ei))
            if lo <= si <= ei < hi:
                spans.append((segments[si]["start"], segments[ei]["end"]))
    return spans


def _merge_spans(spans: list, gap: float = 5.0):
    """Sort and merge overlapping / near-adjacent (within `gap` seconds) time ranges."""
    if not spans:
        return []
    spans = sorted(spans)
    merged = [list(spans[0])]
    for start, end in spans[1:]:
        if start <= merged[-1][1] + gap:
            merged[-1][1] = max(merged[-1][1], end)
        else:
            merged.append([start, end])
    return [{"start": round(s, 3), "end": round(e, 3)} for s, e in merged]


@app.post("/detect_ads")
def detect_ads(req: DetectAdsRequest, x_ai_token: Optional[str] = Header(default=None)):
    """Detect ad/sponsor segments in an already-generated transcript, streaming NDJSON."""
    _check_auth(x_ai_token)
    segments = [{"start": s.start, "end": s.end, "text": s.text} for s in req.segments]

    def stream():
        started = time.time()
        try:
            if not segments:
                yield json.dumps({"type": "result", "segments": []}) + "\n"
                return
            # Remote endpoints have large context windows, so use bigger chunks — far fewer
            # requests, which is gentler on provider rate limits. Local GGUFs keep small chunks
            # to fit their context.
            max_chars = 16000 if req.llm.backend in ("remote", "anthropic") else 6000
            windows = _chunk_segments(segments, max_chars=max_chars)
            all_spans = []
            for wi, (lo, hi) in enumerate(windows):
                all_spans.extend(_detect_window(req.llm, segments, lo, hi))
                progress = (wi + 1) / len(windows)
                yield json.dumps({"type": "progress", "progress": round(progress, 4)}) + "\n"
            merged = _merge_spans(all_spans)
            log.info("Ad detection: %d ad span(s) over %d segments in %.1fs",
                     len(merged), len(segments), time.time() - started)
            yield json.dumps({"type": "result", "segments": merged}) + "\n"
        except HTTPException as he:
            yield json.dumps({"type": "error", "error": he.detail}) + "\n"
        except Exception:  # detail stays in logs, not the client response
            log.exception("Ad detection failed")
            yield json.dumps({"type": "error", "error": "ad detection failed"}) + "\n"

    return StreamingResponse(stream(), media_type="application/x-ndjson")


# --- Model management ------------------------------------------------------------

def _list_local_ggufs():
    return sorted(os.path.basename(p) for p in glob.glob(os.path.join(MODELS_DIR, "**", "*.gguf"), recursive=True))


def _remote_models(url: str):
    base = url.rstrip("/")
    # Try Ollama first (/api/tags), then OpenAI-compatible (/models or /v1/models).
    try:
        r = requests.get(base.rsplit("/v1", 1)[0] + "/api/tags", timeout=10)
        if r.ok:
            return [m["name"] for m in r.json().get("models", [])]
    except Exception:
        pass
    for suffix in ("/models", "/v1/models"):
        try:
            r = requests.get(base + suffix, timeout=10)
            if r.ok:
                return [m["id"] for m in r.json().get("data", [])]
        except Exception:
            continue
    return []


@app.get("/models")
def list_models(remote_url: Optional[str] = None, x_ai_token: Optional[str] = Header(default=None)):
    _check_auth(x_ai_token)
    usage = shutil.disk_usage(MODELS_DIR) if os.path.isdir(MODELS_DIR) else None
    return {
        "whisper": KNOWN_WHISPER_MODELS,
        "llm_local": _list_local_ggufs(),
        "llm_remote": _remote_models(remote_url) if remote_url else [],
        "models_dir": MODELS_DIR,
        "disk": ({"total": usage.total, "used": usage.used, "free": usage.free} if usage else None),
    }


@app.post("/models/pull")
def pull_model(req: PullRequest, x_ai_token: Optional[str] = Header(default=None)):
    """Download a model, streaming NDJSON progress. GGUF and Ollama pulls report real
    byte-level progress; whisper pulls emit start + completion (faster-whisper gives no hook)."""
    _check_auth(x_ai_token)

    def stream():
        try:
            if req.kind == "ollama":
                if not req.url:
                    raise HTTPException(status_code=400, detail="ollama pull requires a url")
                base = req.url.rstrip("/").rsplit("/v1", 1)[0]
                with requests.post(base + "/api/pull", json={"name": req.model}, stream=True, timeout=3600) as r:
                    r.raise_for_status()
                    for line in r.iter_lines():
                        if not line:
                            continue
                        try:
                            d = json.loads(line)
                        except Exception:
                            continue
                        total, completed = d.get("total"), d.get("completed")
                        prog = round(completed / total, 4) if total and completed else 0.0
                        yield json.dumps({"type": "progress", "progress": prog, "status": d.get("status", "")}) + "\n"
                yield json.dumps({"type": "result", "model": req.model}) + "\n"

            elif req.kind == "gguf":
                if not req.repo or not req.filename:
                    raise HTTPException(status_code=400, detail="gguf pull requires repo and filename")
                from huggingface_hub import hf_hub_url

                # Stream the file ourselves so we can report real byte-level progress and write it
                # directly to MODELS_DIR/<basename> (where _get_llama / _list_local_ggufs look).
                url = hf_hub_url(repo_id=req.repo, filename=req.filename)
                dest = os.path.join(MODELS_DIR, os.path.basename(req.filename))
                tmp = dest + ".part"
                os.makedirs(MODELS_DIR, exist_ok=True)
                headers = {}
                token = os.getenv("HF_TOKEN") or os.getenv("HUGGING_FACE_HUB_TOKEN")
                if token:
                    headers["Authorization"] = f"Bearer {token}"
                yield json.dumps({"type": "progress", "progress": 0.0, "status": "downloading"}) + "\n"
                with requests.get(url, stream=True, timeout=3600, headers=headers) as r:
                    r.raise_for_status()
                    total = int(r.headers.get("content-length", 0))
                    downloaded = 0
                    last_pct = -1
                    with open(tmp, "wb") as f:
                        for chunk in r.iter_content(chunk_size=1024 * 1024):
                            if not chunk:
                                continue
                            f.write(chunk)
                            downloaded += len(chunk)
                            if total:
                                pct = int(downloaded * 100 / total)
                                if pct != last_pct:  # throttle to one line per whole percent
                                    last_pct = pct
                                    yield json.dumps({"type": "progress", "progress": round(downloaded / total, 4)}) + "\n"
                os.replace(tmp, dest)
                yield json.dumps({"type": "result", "model": os.path.basename(dest), "path": dest}) + "\n"

            elif req.kind == "whisper":
                from faster_whisper import WhisperModel

                yield json.dumps({"type": "progress", "progress": 0.0, "status": "downloading"}) + "\n"
                WhisperModel(req.model, device="cpu", compute_type="int8", download_root=MODELS_DIR)
                yield json.dumps({"type": "result", "model": req.model}) + "\n"

            else:
                raise HTTPException(status_code=400, detail=f"unknown pull kind: {req.kind}")

        except HTTPException as he:
            yield json.dumps({"type": "error", "error": he.detail}) + "\n"
        except Exception:  # detail stays in logs, not the client response
            log.exception("Model pull failed (%s %s)", req.kind, req.model)
            yield json.dumps({"type": "error", "error": "model pull failed"}) + "\n"

    return StreamingResponse(stream(), media_type="application/x-ndjson")
