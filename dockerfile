# Builder stage for compiling the Yew application
FROM rust:alpine AS builder
# Install build dependencies
RUN apk update && apk upgrade && \
    apk add --no-cache musl-dev libffi-dev zlib-dev jpeg-dev curl
# Install trunk from GitHub releases (musl binary, arch-aware) and wasm target
RUN ARCH=$(uname -m) && \
    curl -sSL "https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-${ARCH}-unknown-linux-musl.tar.gz" \
    | tar -xz -C /usr/local/bin && \
    rustup target add wasm32-unknown-unknown
# Copy Cargo.lock early so we can read the wasm-bindgen version before the full build
COPY ./web/Cargo.lock /app/Cargo.lock
# Pre-populate trunk's wasm-bindgen cache with the musl binary matching Cargo.lock.
# Trunk downloads glibc binaries by default which don't run on Alpine, so we grab
# the musl variant ourselves. Version is read dynamically so it tracks Cargo.lock.
RUN WASM_BINDGEN_VERSION=$(grep -A2 'name = "wasm-bindgen"' /app/Cargo.lock | grep '^version' | head -1 | sed 's/version = "\(.*\)"/\1/') && \
    ARCH=$(uname -m) && \
    mkdir -p /tmp/wb-dl /root/.cache/trunk/wasm-bindgen-${WASM_BINDGEN_VERSION} && \
    curl -sSL "https://github.com/rustwasm/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-${ARCH}-unknown-linux-musl.tar.gz" \
    | tar -xz -C /tmp/wb-dl && \
    find /tmp/wb-dl -name "wasm-bindgen" ! -name "*test*" -type f \
    | xargs -I{} cp {} /root/.cache/trunk/wasm-bindgen-${WASM_BINDGEN_VERSION}/wasm-bindgen && \
    chmod +x /root/.cache/trunk/wasm-bindgen-${WASM_BINDGEN_VERSION}/wasm-bindgen && \
    rm -rf /tmp/wb-dl
# Add application files to the builder stage
COPY ./web/Cargo.toml ./web/dev-info.md ./web/index.html ./web/tailwind.config.js ./web/Trunk.toml /app/
COPY ./web/src /app/src
COPY ./web/static /app/static
WORKDIR /app
# Build the Yew application in release mode
RUN RUSTFLAGS="--cfg=web_sys_unstable_apis --cfg getrandom_backend=\"wasm_js\"" trunk build --features server_build --release

# Go builder stage for the gpodder API
FROM golang:alpine AS go-builder
WORKDIR /gpodder-api

# Install build dependencies
RUN apk add --no-cache git

# Copy go module files first for better layer caching
COPY ./gpodder-api/go.mod ./gpodder-api/go.sum ./
RUN go mod download

# Copy the rest of the source code
COPY ./gpodder-api/cmd ./cmd
COPY ./gpodder-api/config ./config
COPY ./gpodder-api/internal ./internal

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o gpodder-api ./cmd/server/

# Python builder stage for database setup
FROM python:3.11-alpine AS python-builder
WORKDIR /build

# Install build dependencies for PyInstaller and MariaDB connector
RUN apk add --no-cache gcc musl-dev libffi-dev openssl-dev mariadb-connector-c-dev

# Copy Python source files
COPY ./database_functions ./database_functions
COPY ./startup/setup_database_new.py ./startup/setup_database_new.py
COPY ./requirements.txt ./requirements.txt

# Install Python dependencies including PyInstaller
RUN pip install --no-cache-dir -r requirements.txt pyinstaller

# Build standalone database setup binary
RUN pyinstaller --onefile \
    --name pinepods-db-setup \
    --hidden-import psycopg \
    --hidden-import mysql.connector \
    --hidden-import cryptography \
    --hidden-import cryptography.fernet \
    --hidden-import passlib \
    --hidden-import passlib.hash \
    --hidden-import passlib.hash.argon2 \
    --hidden-import argon2 \
    --hidden-import argon2.exceptions \
    --hidden-import argon2.profiles \
    --hidden-import argon2._password_hasher \
    --add-data "database_functions:database_functions" \
    --console \
    startup/setup_database_new.py

# Rust API builder stage
FROM rust:alpine AS rust-api-builder
WORKDIR /rust-api

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

# Copy Rust API files
COPY ./rust-api/Cargo.toml ./rust-api/Cargo.lock ./
COPY ./rust-api/src ./src

# Set environment for static linking
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include

# Build the Rust API
RUN cargo build --release && strip target/release/pinepods-api

# Final stage for setting up runtime environment
FROM alpine
# Metadata
LABEL maintainer="Collin Pendleton <collinp@collinpendleton.com>"
# Install runtime dependencies
# su-exec: drop privileges to PUID:PGID at startup. shadow: usermod/groupmod to remap the runtime user's IDs.
RUN apk add --no-cache tzdata nginx openssl bash mariadb-client postgresql-client curl ffmpeg wget jq mariadb-connector-c-dev su-exec shadow

# Create a fixed runtime user/group (default 911); startup.sh remaps these to PUID/PGID at runtime
RUN addgroup -g 911 pinepods && \
    adduser -D -H -u 911 -G pinepods -h /pinepods pinepods


# Download and install latest yt-dlp — pick the arch-specific musl binary (no Python needed)
RUN ARCH=$(uname -m) && \
    LATEST=$(curl -s https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest | jq -r .tag_name) && \
    case "$ARCH" in \
        x86_64)  YTDLP="yt-dlp_musllinux" ;; \
        aarch64) YTDLP="yt-dlp_musllinux_aarch64" ;; \
        armv7l)  YTDLP="yt-dlp_linux_armv7l" ;; \
        *)       YTDLP="yt-dlp_musllinux" ;; \
    esac && \
    wget -O /usr/local/bin/yt-dlp "https://github.com/yt-dlp/yt-dlp/releases/download/${LATEST}/${YTDLP}" && \
    chmod +x /usr/local/bin/yt-dlp

# Download and install Horust (x86_64)
RUN wget -O /tmp/horust.tar.gz "https://github.com/FedericoPonzi/Horust/releases/download/v0.1.13/horust-x86_64-unknown-linux-musl.tar.gz" && \
    cd /tmp && tar -xzf horust.tar.gz && \
    mv horust /usr/local/bin/ && \
    chmod +x /usr/local/bin/horust && \
    rm -f /tmp/horust.tar.gz

ENV TZ=UTC
# Copy compiled database setup binary (replaces Python dependency)
COPY --from=python-builder /build/dist/pinepods-db-setup /usr/local/bin/
# Copy built files from the builder stage to the Nginx serving directory
COPY --from=builder /app/dist /var/www/html/
# Copy translation files for the Rust API to access
COPY ./web/src/translations /var/www/html/static/translations
# Copy Go API binary from the go-builder stage
COPY --from=go-builder /gpodder-api/gpodder-api /usr/local/bin/
# Copy Rust API binary from the rust-api-builder stage
COPY --from=rust-api-builder /rust-api/target/release/pinepods-api /usr/local/bin/
# Move to the root directory to execute the startup script
WORKDIR /
# Copy startup scripts
COPY startup/startup.sh /startup.sh
RUN chmod +x /startup.sh
# Copy Pinepods runtime files
RUN mkdir -p /pinepods
RUN mkdir -p /var/log/pinepods/ && mkdir -p /etc/horust/services/
COPY startup/ /pinepods/startup/
# Legacy cron scripts removed - background tasks now handled by internal Rust scheduler
COPY clients/ /pinepods/clients/
COPY database_functions/ /pinepods/database_functions/
RUN chmod +x /pinepods/startup/startup.sh
ENV APP_ROOT=/pinepods
# Define the build argument
ARG PINEPODS_VERSION
# Write the Pinepods version to the current_version file
RUN echo "${PINEPODS_VERSION}" > /pinepods/current_version
# OCI image metadata (shown by registries and `docker inspect`)
LABEL org.opencontainers.image.title="PinePods" \
      org.opencontainers.image.description="Self-hosted podcast management server and player" \
      org.opencontainers.image.version="${PINEPODS_VERSION}" \
      org.opencontainers.image.url="https://pinepods.online" \
      org.opencontainers.image.documentation="https://www.pinepods.online/docs/Introduction" \
      org.opencontainers.image.source="https://github.com/madeofpendletonwool/PinePods" \
      org.opencontainers.image.vendor="Gooseberry Development" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
# Configure Nginx
COPY startup/nginx.conf /etc/nginx/nginx.conf

# Copy script to start gpodder API
COPY ./gpodder-api/start-gpodder.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/start-gpodder.sh

RUN cp /usr/share/zoneinfo/UTC /etc/localtime && \
    echo "UTC" > /etc/timezone

# Expose ports (nginx web UI + gpodder API)
EXPOSE 8040 8042

# Container health: nginx proxies /api -> Rust API, which verifies DB connectivity.
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -fsS http://localhost:8040/api/health || exit 1

# Start everything using the startup script
ENTRYPOINT ["bash", "/startup.sh"]
