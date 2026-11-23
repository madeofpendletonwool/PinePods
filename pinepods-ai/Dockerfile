# Optional PinePods AI sidecar (transcription #726 + ad detection #790).
# Build/run only if you want the AI-based features.
FROM python:3.14-slim

# ffmpeg: required by faster-whisper to decode audio.
# build-essential + cmake: required to build llama-cpp-python (local LLM backend) from source.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg curl build-essential cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Set automatically by BuildKit (amd64 / arm64).
ARG TARGETARCH
# "cuda" adds the CUDA libraries faster-whisper needs for WHISPER_DEVICE=cuda (amd64 only).
ARG GPU=""

COPY requirements.txt .
# llama.cpp builds for the build machine's CPU by default, and a binary built on a CI runner with
# AVX-512 crashes with "Illegal instruction" on CPUs without it. Build for a portable baseline
# instead: AVX2 on x86-64, plain ARMv8 on arm64.
RUN if [ "$TARGETARCH" = "amd64" ]; then \
        export CMAKE_ARGS="-DGGML_NATIVE=OFF -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON -DGGML_BMI2=ON -DGGML_AVX512=OFF"; \
    else \
        export CMAKE_ARGS="-DGGML_NATIVE=OFF"; \
    fi \
    && pip install --no-cache-dir -r requirements.txt \
    && if [ "$GPU" = "cuda" ]; then \
        pip install --no-cache-dir "nvidia-cublas-cu12==12.*" "nvidia-cudnn-cu12==9.*"; \
    fi

# Where pip puts the CUDA libraries in the cuda variant (absent, and harmless, otherwise).
ENV LD_LIBRARY_PATH=/usr/local/lib/python3.14/site-packages/nvidia/cublas/lib:/usr/local/lib/python3.14/site-packages/nvidia/cudnn/lib

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
