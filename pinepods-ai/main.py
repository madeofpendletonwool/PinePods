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

import gc
import glob
import json
import logging
import os
import re
import shutil
import threading
import time
from contextlib import contextmanager
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
# Free a model's memory (RAM or GPU) once it has sat unused this many seconds; 0 keeps models loaded.
IDLE_UNLOAD_SECS = int(os.getenv("PINEPODS_AI_IDLE_UNLOAD", "300"))
# Transcriptions run at the same time; more wait their turn.
MAX_TRANSCRIBE_JOBS = max(1, int(os.getenv("PINEPODS_AI_MAX_TRANSCRIBE_JOBS", "1")))

# Whisper model sizes faster-whisper can pull on demand (surfaced in /models).
KNOWN_WHISPER_MODELS = [
    "tiny", "tiny.en", "base", "base.en", "small", "small.en",
    "medium", "medium.en", "large-v1", "large-v2", "large-v3", "distil-large-v3",
]

app = FastAPI(title="PinePods AI", version="0.2.0")


class _ModelCache:
    """Models loaded on first use and kept, keyed by name, so several whisper sizes / GGUFs can be
    held at once. Tracks who is using each one so idle models can be freed without pulling one out
    from under a running job."""

    def __init__(self, kind: str, load, close=None):
        self._kind = kind
        self._load = load
        self._close = close
        self._lock = threading.Lock()
        self._models: dict = {}
        self._users: dict = {}
        self._last_used: dict = {}

    @contextmanager
    def use(self, key):
        with self._lock:
            model = self._models.get(key)
            if model is None:
                model = self._load(key)
                self._models[key] = model
            self._users[key] = self._users.get(key, 0) + 1
        try:
            yield model
        finally:
            with self._lock:
                self._users[key] -= 1
                self._last_used[key] = time.time()

    def unload_idle(self, idle_secs: float):
        now = time.time()
        with self._lock:
            idle = [k for k in self._models
                    if self._users.get(k, 0) == 0 and now - self._last_used.get(k, now) >= idle_secs]
            for key in idle:
                model = self._models.pop(key)
                if self._close:
                    try:
                        self._close(model)
                    except Exception:
                        log.exception("Closing %s model %s failed", self._kind, key)
                del model
                log.info("Unloaded idle %s model %s", self._kind, key)
        if idle:
            gc.collect()

    def loaded(self) -> list:
        # No lock: a load can hold it for minutes, and /health must answer meanwhile.
        return list(self._models)


def _load_whisper(key):
    name, device, compute_type = key
    from faster_whisper import WhisperModel  # imported lazily

    log.info("Loading Whisper model '%s' (device=%s, compute_type=%s)", name, device, compute_type)
    model = WhisperModel(name, device=device, compute_type=compute_type, download_root=MODELS_DIR)
    log.info("Whisper model '%s' loaded", name)
    return model


def _resolve_gguf_path(model: str) -> str:
    """Resolve a local GGUF name to an absolute path confined to MODELS_DIR."""
    candidate = model if os.path.isabs(model) else os.path.join(MODELS_DIR, model)
    real = os.path.realpath(candidate)
    if not (real == MODELS_DIR or real.startswith(MODELS_DIR + os.sep)):
        raise HTTPException(status_code=400, detail="model path outside allowed models directory")
    if not os.path.isfile(real):
        raise HTTPException(status_code=404, detail=f"local model not found: {model}")
    return real


def _load_llama(model: str):
    from llama_cpp import Llama  # imported lazily

    path = _resolve_gguf_path(model)
    log.info("Loading GGUF LLM '%s' (n_ctx=%d)", model, LLM_N_CTX)
    llm = Llama(model_path=path, n_ctx=LLM_N_CTX, verbose=False)
    log.info("GGUF LLM '%s' loaded", model)
    return llm


_whisper_models = _ModelCache("whisper", _load_whisper)
_llama_models = _ModelCache("llm", _load_llama, close=lambda m: m.close())

# Heavy jobs share the machine one (or MAX_TRANSCRIBE_JOBS) at a time. llama.cpp in particular
# isn't safe to run from two threads at once: overlapping ad detections crash the process.
_transcribe_slots = threading.Semaphore(MAX_TRANSCRIBE_JOBS)
_llm_lock = threading.Lock()


def _queued_line() -> str:
    return json.dumps({"type": "progress", "progress": 0.0, "status": "queued"}) + "\n"


def _idle_unloader():
    while True:
        time.sleep(30)
        _whisper_models.unload_idle(IDLE_UNLOAD_SECS)
        _llama_models.unload_idle(IDLE_UNLOAD_SECS)


if IDLE_UNLOAD_SECS > 0:
    threading.Thread(target=_idle_unloader, name="idle-unloader", daemon=True).start()


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


_INVISIBLE = "\u200b\u200c\u200d\u2060\ufeff"


def _clean_name(value: Optional[str]) -> Optional[str]:
    """Drop surrounding whitespace and zero-width characters copied along with a name."""
    if value is None:
        return None
    return value.strip().strip(_INVISIBLE).strip() or None


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


# Async so it runs on the event loop: queued jobs can tie up the worker threads.
@app.get("/health")
async def health():
    return {
        "status": "ok",
        "model": MODEL_NAME,
        "device": DEVICE,
        "compute_type": COMPUTE_TYPE,
        "models_dir": MODELS_DIR,
        "whisper_loaded": [n for (n, _, _) in _whisper_models.loaded()],
        "llm_loaded": _llama_models.loaded(),
        "idle_unload_secs": IDLE_UNLOAD_SECS,
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
        if not _transcribe_slots.acquire(blocking=False):
            yield _queued_line()
            _transcribe_slots.acquire()
        try:
            with _whisper_models.use((model_name, DEVICE, COMPUTE_TYPE)) as model:
                yield from _transcribe(model, model_name, path, req.language)
        except Exception as e:  # model failed to load
            log.exception("Loading whisper model %s failed", model_name)
            yield json.dumps({"type": "error", "error": f"transcription failed: {type(e).__name__}: {e}"}) + "\n"
        finally:
            _transcribe_slots.release()

    return StreamingResponse(stream(), media_type="application/x-ndjson")


def _transcribe(model, model_name: str, path: str, language: Optional[str]):
    started = time.time()
    log.info("Transcribing %s (model=%s)", path, model_name)
    try:
        segments_iter, info = model.transcribe(path, language=language, beam_size=BEAM_SIZE)
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
            "language": info.language or (language or "unknown"),
            "text": " ".join(text_parts).strip(),
            "segments": segments,
            "model": model_name,
            "duration": round(total, 3),
        }) + "\n"
    except Exception as e:  # surface mid-stream failures to the caller (full traceback in logs)
        log.exception("Transcription failed for %s", path)
        yield json.dumps({"type": "error", "error": f"transcription failed: {type(e).__name__}: {e}"}) + "\n"


# --- Ad detection (#790) ---------------------------------------------------------

AD_SYSTEM_PROMPT = (
    "You are an expert at detecting advertisements and sponsor reads in podcast "
    "transcripts. Ads include host-read sponsor spots, promo codes, 'this episode is "
    "brought to you by', dynamically inserted ads, promos for other podcasts or shows, "
    "and calls to visit a sponsor's site. Episodes often open with one or more ads before "
    "the show itself starts, so the first segments can be ads with no lead-in. Normal show "
    "content, listener mail, and self-promotion of the show's own membership are NOT ads "
    "unless they are a paid sponsor read. Be precise about boundaries."
)


def _chunk_segments(segments: list[dict], max_chars: int = 6000, overlap: int = 10):
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
    with _llama_models.use(model) as llama:
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

    # Local models run one detection at a time (see _llm_lock); remote endpoints don't need to.
    local = req.llm.backend not in ("remote", "anthropic")

    def stream():
        if local and not _llm_lock.acquire(blocking=False):
            yield _queued_line()
            _llm_lock.acquire()
        try:
            yield from _detect(req, segments)
        finally:
            if local:
                _llm_lock.release()

    return StreamingResponse(stream(), media_type="application/x-ndjson")


def _detect(req: DetectAdsRequest, segments: list[dict]):
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
    except Exception as e:  # full traceback in logs; summary to the client
        log.exception("Ad detection failed")
        yield json.dumps({"type": "error", "error": f"ad detection failed: {type(e).__name__}: {e}"}) + "\n"


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
    req.model = _clean_name(req.model) or ""
    req.repo = _clean_name(req.repo)
    req.filename = _clean_name(req.filename)
    req.url = _clean_name(req.url)

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
                # directly to MODELS_DIR/<basename> (where _load_llama / _list_local_ggufs look).
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
        except Exception as e:  # full traceback in logs; summary to the client
            log.exception("Model pull failed (%s %s)", req.kind, req.model)
            yield json.dumps({"type": "error", "error": f"model pull failed: {type(e).__name__}: {e}"}) + "\n"

    return StreamingResponse(stream(), media_type="application/x-ndjson")
