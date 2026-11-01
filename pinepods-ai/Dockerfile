# Optional PinePods AI sidecar (transcription #726 + ad detection #790).
# Build/run only if you want the AI-based features.
FROM ghcr.io/astral-sh/uv:0.13.6 AS uv

FROM python:3.14-slim

# ffmpeg: required by faster-whisper to decode audio.
# build-essential + cmake: required to build llama-cpp-python (local LLM backend) from source.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg curl build-essential cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY .python-version pyproject.toml uv.lock ./
RUN --mount=from=uv,source=/uv,target=/bin/uv \
    uv sync --locked --no-cache
ENV PATH="/app/.venv/bin:$PATH"

COPY main.py .

# Where the Whisper model weights are cached inside the container. Mount a volume
# here to avoid re-downloading the model on every container recreate.
ENV HF_HOME=/models \
    PINEPODS_AI_PORT=8100

EXPOSE 8100

HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 \
    CMD curl -fsS http://localhost:8100/health || exit 1

# Single worker: the Whisper model is large and held in-process; scale with
# replicas rather than workers if needed.
CMD ["sh", "-c", "uvicorn main:app --host 0.0.0.0 --port ${PINEPODS_AI_PORT}"]
