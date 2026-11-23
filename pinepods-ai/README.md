# PinePods AI (optional)

A small, **optional**, stateless sidecar that gives PinePods AI-powered features.
It is modeled on Immich's `immich-machine-learning` container: a separate service
the main PinePods API talks to over HTTP. If it isn't running, PinePods simply
hides/disables the AI features — nothing else breaks.

It provides **speech-to-text transcription** (GitHub #726) using
[faster-whisper](https://github.com/SYSTRAN/faster-whisper) and **ad detection** (#790)
with a local GGUF model or a remote LLM endpoint. It holds no database
access and stores nothing persistent (aside from cached model weights).

## How it fits in

```
pinepods (rust-api)  --HTTP-->  pinepods-ai  (this service)
        |                              |
        +--- both mount the same downloads dir (this one read-only) ---+
```

The API sends a **file path** (under the shared downloads mount) and receives the
transcript text plus segment-level timestamps. Point the API at this service with
the `PINEPODS_AI_URL` env var (e.g. `http://pinepods-ai:8100`).

## Dynamic ads

Many podcasts insert different ads each time an episode is fetched, so ad times only
fit the copy they were detected on. The API prefers a server download when it
transcribes, records which copy it used (a fingerprint that ignores ID3 tags, plus
its length), links later server downloads of the episode to that copy, and redoes
the transcript when a download turns out to be a different copy. Players skip ads
only on the matching server download or a stream of the same length. For accurate
skipping, turn on server auto-download for podcasts with auto ad detection; the
"delete downloads when finished" setting keeps that from filling the disk.

## Endpoints

- `GET /health` — readiness + loaded model info.
- `POST /transcribe` — body `{ "file_path": "/opt/pinepods/downloads/.../ep.mp3", "language": null }`;
  returns `{ language, text, segments: [{start, end, text}], model, duration }`.

## Configuration (env)

Models are chosen in PinePods under Settings → AI and sent with each request. The
variables below describe the hardware and limits.

| Var | Default | Notes |
|-----|---------|-------|
| `WHISPER_DEVICE` | `cpu` | `cuda` with the `-cuda` image on an NVIDIA GPU |
| `WHISPER_COMPUTE_TYPE` | `int8` | e.g. `int8`, `float16` (GPU) |
| `WHISPER_BEAM_SIZE` | `5` | |
| `PINEPODS_AI_IDLE_UNLOAD` | `300` | free a model's RAM/VRAM after this many idle seconds; `0` keeps models loaded |
| `PINEPODS_AI_MAX_TRANSCRIBE_JOBS` | `1` | transcriptions run at once; the rest queue. Local LLM ad detection always runs one at a time |
| `AI_LLM_N_CTX` | `8192` | local LLM context window |
| `AI_LLM_MAX_TOKENS` | `1024` | |
| `PINEPODS_AI_PORT` | `8100` | |
| `PINEPODS_AI_MEDIA_BASE` | `/opt/pinepods/downloads` | transcription is confined to this dir |
| `PINEPODS_AI_TOKEN` | *(unset)* | if set, callers must send it as `X-AI-Token` |

`WHISPER_MODEL` and `AI_LLM_MODEL` only apply to direct API calls that name no model;
PinePods always names one.

## Images

- `latest` / `<version>`: CPU, amd64 + arm64. Local LLM ad detection needs AVX2 on x86-64
  (most Intel/AMD CPUs since 2013–2015, but not Atom or Celeron/Pentium J and N models);
  arm64 targets plain ARMv8. Transcription and remote LLM backends work on any CPU.
- `latest-cuda` / `<version>-cuda`: amd64, adds the CUDA 12 cuBLAS and cuDNN 9 libraries
  faster-whisper needs for `WHISPER_DEVICE=cuda`. Local LLM ad detection still runs on the CPU.
  Build it yourself with `docker build --build-arg GPU=cuda`.

## Run

```bash
docker build -t pinepods-ai ./pinepods-ai
docker run --rm -p 8100:8100 \
  -v /home/user/pinepods/downloads:/opt/pinepods/downloads:ro \
  -v pinepods-ai-models:/models \
  pinepods-ai
```

Or enable the commented-out `pinepods-ai` service in your
`deployment/docker/compose-files/docker-compose-*/docker-compose.yml` and set
`PINEPODS_AI_URL: http://pinepods-ai:8100` on the `pinepods` service.
